use std::rc::Rc;
use std::cell::RefCell;
use std::collections::HashMap;
use std::cmp::Ordering;
use std::fmt;
use crate::ast::Node;

mod scope;
pub use scope::Scope;

mod undef;
pub use undef::Undef;

mod func;
pub use func::BuiltinFn;



// ----------------------------------------------------------------------------
//  Value  (every first-class Swan value)
// ----------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum Value {

    /// Empty tuple
    Nothing,

    /// Boolean value
    Bool(bool),

    /// Number
    Numb(f64),

    /// Strings
    Text(Rc<str>),

    /// An ordered, integer-indexed sequence — `[a, b, c]`.
    List(Rc<Vec<Value>>),

    /// A first-class, flat key→value map — `{ x: 1, y: 2 }`.
    ///
    /// This wraps a *flat* `HashMap`; it does NOT have a parent chain.
    /// Scope chaining is the responsibility of `Scope`, not `Value::Namespace`.
    /// This makes namespace equality (deep key/value comparison) and merge
    /// (`{a:1} + {b:2}  →  {a:1, b:2}`) straightforward.
    Namespace(Scope),

    /// A user-defined function.
    ///
    /// `closure` captures the lexical `Scope` at definition time so that the
    /// function body can look up free variables correctly when called later.
    Func {
        params:  Box<Value>,
        body:    Box<Node>,
        closure: Scope,
    },

    /// A native Rust function callable from Swan.
    Builtin(BuiltinFn),

    /// Undefined value
    ///
    /// Swan expression do not throw exceptions; they return Undefined
    /// values instead.
    Undefined(Undef),

    /// The Swan product type — a heterogeneous sequence of items produced by
    /// the comma operator `a, b, c`.  A Tuple of zero items is `Nothing`.
    /// A Tuple of exactly one item normalises to that item.
    Tuple(Rc<Vec<Value>>),

    /// Name: identifiers at the left side of an assignment or
    /// at the left side of a function operation
    Name(Rc<str>),
}


impl Value {

    /// Helper to construct a unary operation Undefined value.
    #[inline]
    pub fn undef_unary(op: &'static str, val: Value, pos: usize) -> Value {
        Value::Undefined(Undef::UnaryOperation {
            op,
            val: Box::new(val),
            pos
        })
    }

    /// Helper to construct a binary operation Undefined value.
    #[inline]
    pub fn undef_binary(op: &'static str, lhv: Value, rhv: Value, pos: usize) -> Value {
        Value::Undefined(Undef::BinaryOperation {
            op,
            lhv: Box::new(lhv),
            rhv: Box::new(rhv),
            pos
        })
    }
    
    /// Helper to construct and normalize a tuple
    #[inline]
    pub fn tuple(items: Vec<Value>) -> Value {
        match items.len() {
            0 => Value::Nothing,
            _ => Value::Tuple(Rc::new(items)).normalize(),
        }
    }
}



// ----------------------------------------------------------------------------
//  Implement equality and order for Value
// ----------------------------------------------------------------------------

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Bool(a),      Value::Bool(b))      => a == b,
            (Value::Numb(a),      Value::Numb(b))      => a == b,
            (Value::Text(a),      Value::Text(b))      => a == b,
            (Value::List(a),      Value::List(b))      => a == b,
            (Value::Tuple(a),     Value::Tuple(b))     => a == b,
            (Value::Namespace(a), Value::Namespace(b)) => a == b,
            (Value::Undefined(a), Value::Undefined(b)) => a == b,
            (Value::Nothing,      Value::Nothing)      => true,
            // Every other pair is never equal
            _ => false,
        }
    }
}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (Value::Bool(a),  Value::Bool(b))  => a.partial_cmp(b),
            (Value::Numb(a),  Value::Numb(b))  => a.partial_cmp(b),
            (Value::Text(a),  Value::Text(b))  => a.partial_cmp(b),
            (Value::List(a),  Value::List(b))  => compare_iterators(a.iter(), b.iter()),
            (Value::Tuple(a), Value::Tuple(b)) => compare_iterators(a.iter(), b.iter()),
            // Nothing is equal to Nothing and less that anything else.
            (Value::Nothing,  Value::Nothing)  => Some(Ordering::Equal),
            (Value::Nothing,  _)               => Some(Ordering::Less),
            (_,  Value::Nothing)               => Some(Ordering::Greater),
            
            // Func and Undef are never equal to anything (including themselves).
            _ => None,
        }
    }
}

/// Helper function to perform lexicographical comparison between item iterators
fn compare_iterators<'a>(
    mut it1: impl Iterator<Item = &'a Value>,
    mut it2: impl Iterator<Item = &'a Value>,
) -> Option<Ordering> {
    loop {
        match (it1.next(), it2.next()) {
            (None, None)    => return Some(Ordering::Equal),
            (None, Some(_)) => return Some(Ordering::Less),    // Shorter tuple/list is Less
            (Some(_), None) => return Some(Ordering::Greater), // Longer tuple/list is Greater
            (Some(a), Some(b)) => match a.partial_cmp(b) {
                Some(Ordering::Equal) => continue,
                non_eq => return non_eq,
            },
        }
    }
}



// ----------------------------------------------------------------------------
//  ValueIter  (generic "tuple item" iterator over any Value)
//
//  Every Value in Swan is conceptually a Tuple: `Nothing` is the empty tuple,
//  any non-Tuple Item behaves like a 1-element tuple, and `Value::Tuple`
//  behaves like the sequence of its elements. `ValueIter` implements this
//  view uniformly, mirroring `Term.items()` in the JS reference.
//
//  Nested tuples are flattened (defensively — well-formed values should never
//  contain a Tuple element after normalisation, but the iterator handles it
//  anyway) and `Nothing` items are skipped, so `(1, (), 2)` yields `1, 2`,
//  not `1, Nothing, 2`.
// ----------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ValueIter {
    /// Stack of (tuple, next_index) frames; the last entry is the tuple
    /// currently being consumed. Nested tuples push a new frame; exhausted
    /// frames are popped.
    stack: Vec<(Rc<Vec<Value>>, usize)>,
}

impl ValueIter {
    fn new(value: Value) -> Self {
        match value {
            Value::Nothing  => ValueIter { stack: Vec::new() },
            Value::Tuple(v) => ValueIter { stack: vec![(v, 0)] },
            other           => ValueIter { stack: vec![(Rc::new(vec![other]), 0)] },
        }
    }
}

impl Iterator for ValueIter {
    type Item = Value;

    fn next(&mut self) -> Option<Value> {
        loop {
            let (items, idx) = self.stack.last_mut()?;

            if *idx >= items.len() {
                self.stack.pop();
                continue;
            }

            let item = items[*idx].clone();
            *idx += 1;

            match item {
                Value::Nothing  => continue,          // skip Nothing items
                Value::Tuple(v) => self.stack.push((v, 0)),  // flatten
                other           => return Some(other),
            }
        }
    }
}

impl std::iter::FusedIterator for ValueIter {}



// ----------------------------------------------------------------------------
//  Pairs  (generic "tuple item pair" iterator over two Values)
//
//  Yields corresponding pairs of items from two values (each treated as a
//  tuple), mirroring `Term.iterPairs(other)` in the JS reference.
//
//  When the two tuples have different lengths, the missing items from the
//  shorter tuple are replaced with `Value::Nothing`.
// ----------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Pairs {
    iter1: ValueIter,
    iter2: ValueIter,
}

impl Pairs {
    /// Creates a new `Pairs` iterator from two values.
    pub fn new(first: &Value, second: &Value) -> Self {
        Self {
            iter1: first.items(),
            iter2: second.items(),
        }
    }
}

impl Iterator for Pairs {
    type Item = (Value, Value);

    fn next(&mut self) -> Option<Self::Item> {
        let item1 = self.iter1.next();
        let item2 = self.iter2.next();

        match (item1, item2) {
            (None, None) => None,
            (Some(v1), Some(v2)) => Some((v1, v2)),
            (Some(v1), None) => Some((v1, Value::Nothing)),
            (None, Some(v2)) => Some((Value::Nothing, v2)),
        }
    }
}

impl std::iter::FusedIterator for Pairs {}



// ----------------------------------------------------------------------------
//  Value tuple methods
// ----------------------------------------------------------------------------

impl Value {
    /// Returns an iterator over this value's items, treating it as a tuple.
    ///
    /// - `Nothing` yields no items.
    /// - `Tuple` yields its items (recursively flattened, `Nothing` skipped).
    /// - Any other value yields itself as the single item.
    pub fn items(&self) -> ValueIter {
        ValueIter::new(self.clone())
    }

    /// Returns an iterator yielding corresponding pairs of items between
    /// `self` and `other`, treating each value as a tuple.
    ///
    /// If the two values have different lengths, the missing items from the
    /// shorter tuple are replaced with [`Value::Nothing`].
    pub fn pair(&self, other: &Value) -> Pairs {
        Pairs::new(self, other)
    }

    pub fn normalize(&self) -> Self {
        if matches!(self, Value::Tuple(_)) {
            let flat_items: Vec<_> = self.items().collect();
            match flat_items.len() {
                0 => Value::Nothing,
                1 => flat_items[0].clone(),
                _ => Value::Tuple(Rc::new(flat_items)),
            }
        } else {
            self.clone()
        }
    }

    /// Maps a function `f: Value -> Value` over this `Value`:
    /// - `Value::Nothing` -> `Value::Nothing`
    /// - `Value::Tuple`   -> `Value::Tuple` containing `f(v1), f(v2), ..., f(vn)`
    /// - Any other value  -> `f(self)`
    pub fn map(&self, f: impl Fn(Value) -> Value + 'static) -> Value {
        match self {
            Value::Nothing => Value::Nothing,
            Value::Tuple(items) => {
                let mapped_items: Vec<Value> = items
                    .iter()
                    .map(|item| f(item.clone()))
                    .collect();
                Value::Tuple(Rc::new(mapped_items)).normalize()
            }
            _ => f(self.clone()),
        }
    }
}



// ----------------------------------------------------------------------------
//  Value type casting
// ----------------------------------------------------------------------------

impl From<Value> for bool {
    fn from(val: Value) -> Self {
        match val {
            Value::Nothing      => false,
            Value::Bool(b)      => b,
            Value::Numb(n)      => n != 0.0,
            Value::Text(t)      => !t.is_empty(),
            Value::List(v)      => !v.is_empty(),
            Value::Func{..}     => true,
            Value::Builtin(_)   => true,
            Value::Undefined(_) => false,
            Value::Namespace(s) => s.0.borrow().bindings.len() > 0,
            Value::Tuple(_) => {
                // Returns true if AT LEAST ONE item in the tuple converts to true.
                // If ALL items map to false, `.any()` returns false.
                val.items().any(|item| bool::from(item))
            }
            // For remaining types like Namespace, Func, Builtin, Name
            _ => panic!("Can't convert value to boolean: {:?}", val),
        }
    }
}



// ============================================================================
//  Tests
// ============================================================================

#[cfg(test)]
mod tuple_tests {
    use super::*;

    fn tuple(items: Vec<Value>) -> Value {
        Value::Tuple(Rc::new(items))
    }

    #[test]
    fn test_pairs_same_length() {
        let t1 = tuple(vec![Value::Numb(1.0), Value::Numb(2.0), Value::Numb(3.0)]);
        let t2 = tuple(vec![Value::Numb(10.0), Value::Numb(20.0), Value::Numb(30.0)]);

        let pairs: Vec<(Value, Value)> = t1.pair(&t2).collect();
        assert_eq!(pairs.len(), 3);
        assert_eq!(pairs[0], (Value::Numb(1.0), Value::Numb(10.0)));
        assert_eq!(pairs[1], (Value::Numb(2.0), Value::Numb(20.0)));
        assert_eq!(pairs[2], (Value::Numb(3.0), Value::Numb(30.0)));
    }

    #[test]
    fn test_pairs_first_longer() {
        let t1 = tuple(vec![Value::Numb(1.0), Value::Numb(2.0), Value::Numb(3.0)]);
        let t2 = tuple(vec![Value::Numb(10.0), Value::Numb(20.0)]);

        let pairs: Vec<(Value, Value)> = t1.pair(&t2).collect();
        assert_eq!(pairs.len(), 3);
        assert_eq!(pairs[0], (Value::Numb(1.0), Value::Numb(10.0)));
        assert_eq!(pairs[1], (Value::Numb(2.0), Value::Numb(20.0)));
        assert_eq!(pairs[2], (Value::Numb(3.0), Value::Nothing));
    }

    #[test]
    fn test_pairs_second_longer() {
        let t1 = tuple(vec![Value::Numb(1.0), Value::Numb(2.0)]);
        let t2 = tuple(vec![Value::Numb(10.0), Value::Numb(20.0), Value::Numb(30.0)]);

        let pairs: Vec<(Value, Value)> = t1.pair(&t2).collect();
        assert_eq!(pairs.len(), 3);
        assert_eq!(pairs[0], (Value::Numb(1.0), Value::Numb(10.0)));
        assert_eq!(pairs[1], (Value::Numb(2.0), Value::Numb(20.0)));
        assert_eq!(pairs[2], (Value::Nothing, Value::Numb(30.0)));
    }

    #[test]
    fn test_pairs_both_empty() {
        let t1 = Value::Nothing;
        let t2 = Value::Nothing;

        let pairs: Vec<(Value, Value)> = t1.pair(&t2).collect();
        assert!(pairs.is_empty());
    }

    #[test]
    fn test_pairs_first_empty() {
        let t1 = Value::Nothing;
        let t2 = tuple(vec![Value::Numb(10.0), Value::Numb(20.0)]);

        let pairs: Vec<(Value, Value)> = t1.pair(&t2).collect();
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0], (Value::Nothing, Value::Numb(10.0)));
        assert_eq!(pairs[1], (Value::Nothing, Value::Numb(20.0)));
    }

    #[test]
    fn test_pairs_second_empty() {
        let t1 = tuple(vec![Value::Numb(1.0), Value::Numb(2.0)]);
        let t2 = Value::Nothing;

        let pairs: Vec<(Value, Value)> = t1.pair(&t2).collect();
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0], (Value::Numb(1.0), Value::Nothing));
        assert_eq!(pairs[1], (Value::Numb(2.0), Value::Nothing));
    }

    #[test]
    fn test_pairs_scalars() {
        let v1 = Value::Numb(42.0);
        let v2 = Value::Text("swan".into());

        let pairs: Vec<(Value, Value)> = v1.pair(&v2).collect();
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0], (Value::Numb(42.0), Value::Text("swan".into())));
    }

    #[test]
    fn test_pairs_nested_tuples_flattened() {
        let inner = tuple(vec![Value::Numb(2.0), Value::Numb(3.0)]);
        let t1 = tuple(vec![Value::Numb(1.0), inner]);
        let t2 = tuple(vec![Value::Numb(10.0), Value::Numb(20.0), Value::Numb(30.0)]);

        let pairs: Vec<(Value, Value)> = t1.pair(&t2).collect();
        assert_eq!(pairs.len(), 3);
        assert_eq!(pairs[0], (Value::Numb(1.0), Value::Numb(10.0)));
        assert_eq!(pairs[1], (Value::Numb(2.0), Value::Numb(20.0)));
        assert_eq!(pairs[2], (Value::Numb(3.0), Value::Numb(30.0)));
    }

    #[test]
    fn test_pairs_skips_nothing_in_tuples() {
        let t1 = tuple(vec![Value::Numb(1.0), Value::Nothing, Value::Numb(2.0)]);
        let t2 = tuple(vec![Value::Numb(10.0), Value::Numb(20.0)]);

        let pairs: Vec<(Value, Value)> = t1.pair(&t2).collect();
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0], (Value::Numb(1.0), Value::Numb(10.0)));
        assert_eq!(pairs[1], (Value::Numb(2.0), Value::Numb(20.0)));
    }

    #[test]
    fn test_pairs_fused_iterator() {
        let t1 = Value::Nothing;
        let t2 = Value::Nothing;

        let mut pairs = t1.pair(&t2);
        assert_eq!(pairs.next(), None);
        assert_eq!(pairs.next(), None);
        assert_eq!(pairs.next(), None);
    }
}



#[cfg(test)]
mod boolean_conversion_tests {
    use super::*;

    fn tuple(items: Vec<Value>) -> Value {
        Value::Tuple(Rc::new(items))
    }

    #[test]
    fn test_primitives_to_bool() {
        assert!(!bool::from(Value::Nothing));
        assert!(!bool::from(Value::Bool(false)));
        assert!(bool::from(Value::Bool(true)));
        assert!(!bool::from(Value::Numb(0.0)));
        assert!(bool::from(Value::Numb(1.5)));
        assert!(!bool::from(Value::Text("".into())));
        assert!(bool::from(Value::Text("hello".into())));
        assert!(!bool::from(Value::List(Rc::new(vec![]))));
        assert!(bool::from(Value::List(Rc::new(vec![Value::Numb(1.0)]))));
    }

    #[test]
    fn test_tuple_all_false_maps_to_false() {
        // A tuple where all items map to false
        let all_false_tuple = tuple(vec![
            Value::Bool(false),
            Value::Numb(0.0),
            Value::Text("".into()),
            Value::Nothing,
        ]);

        assert!(!bool::from(all_false_tuple));
    }

    #[test]
    fn test_tuple_at_least_one_true_maps_to_true() {
        // A tuple where at least one item maps to true
        let one_true_tuple = tuple(vec![
            Value::Bool(false),
            Value::Numb(0.0),
            Value::Text("truthy".into()),
            Value::Nothing,
        ]);

        assert!(bool::from(one_true_tuple));
    }

    #[test]
    fn test_nested_tuple_all_false_maps_to_false() {
        // Nested tuples evaluate elements recursively via items()
        let inner_false = tuple(vec![Value::Numb(0.0), Value::Bool(false)]);
        let nested_all_false = tuple(vec![Value::Nothing, inner_false]);

        assert!(!bool::from(nested_all_false));
    }
}
