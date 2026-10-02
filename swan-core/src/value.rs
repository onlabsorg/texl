use std::rc::Rc;
use std::cell::RefCell;
use std::collections::HashMap;
use std::cmp::Ordering;
use std::fmt;
use crate::ast::Node;





// ----------------------------------------------------------------------------
//  Scope  (evaluation environment — NOT a first-class Swan value)
//
//  A `Scope` is the lexical environment used during evaluation. It forms a
//  parent-chained singly-linked list of binding tables, one per lexical block.
//  It is reference-counted so that closures can cheaply capture a snapshot of
//  the current environment.
//
//  This type is intentionally separate from `Value::Namespace`. A Scope is an
//  internal evaluator concept; a Namespace is a first-class Swan value.
// ----------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Scope(pub Rc<RefCell<ScopeData>>);

#[derive(Clone)]
pub struct ScopeData {
    pub parent: Option<Scope>,
    pub bindings: HashMap<String, Value>,
}

impl Scope {
    /// Create a top-level scope with no parent.
    pub fn new() -> Self {
        Self(Rc::new(RefCell::new(ScopeData {
            parent: None,
            bindings: HashMap::new(),
        })))
    }

    /// Create a child scope that inherits from a parent.
    pub fn with_parent(parent: Scope) -> Self {
        Self(Rc::new(RefCell::new(ScopeData {
            parent: Some(parent),
            bindings: HashMap::new(),
        })))
    }

    /// Creates a new child scope using `self` as the parent environment.
    pub fn new_child(&self) -> Self {
        Self::with_parent(self.clone())
    }

    /// Recursively looks up an identifier up the scope chain.
    ///
    /// The borrow on `self` is released *before* recursing into the parent so
    /// that there is no live `RefCell` borrow held across the recursive call.
    /// This prevents a runtime panic in the (unlikely but possible) event that
    /// the same `Scope` node appears more than once in the chain.
    pub fn get(&self, key: &str) -> Value {
        // Release the borrow before any recursive call.
        let (found, parent) = {
            let inner = self.0.borrow();
            let found = inner.bindings.get(key).cloned();
            let parent = inner.parent.clone();
            (found, parent)
        };

        found.unwrap_or_else(|| {
            parent
                .map(|p| p.get(key))
                .unwrap_or(Value::Nothing)
        })
    }

    /// Traverses from the root ancestor scope down to the current scope,
    /// returning the first occurrence of `key` found along the child chain.
    pub fn lift(&self, key: &str) -> Value {
        // Build a vector of scopes starting from the root down to `self`
        let mut chain = Vec::new();
        let mut current = Some(self.clone());

        while let Some(scope) = current {
            chain.push(scope.clone());
            let inner = scope.0.borrow();
            current = inner.parent.clone();
        }

        // Search top-down (root is at the back of `chain`)
        for scope in chain.iter().rev() {
            let inner = scope.0.borrow();
            if let Some(val) = inner.bindings.get(key) {
                return val.clone();
            }
        }

        Value::Nothing
    }

    /// Inserts or updates a binding in the current local scope only.
    pub fn set(&self, key: impl Into<String>, val: Value) {
        self.0.borrow_mut().bindings.insert(key.into(), val);
    }
}

impl PartialEq for ScopeData {

    fn eq(&self, other: &Self) -> bool {

        // Compare bindings by value
        if self.bindings != other.bindings {
            return false;
        }

        // Check if both parents exist or both are None
        match (&self.parent, &other.parent) {
            (None, None) => true,
            (Some(p1), Some(p2)) => Rc::ptr_eq(&p1.0, &p2.0), // Same parent pointer instance
            _ => false,
        }
    }
}

impl fmt::Debug for ScopeData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Print only the binding names: values may be Funcs whose closures
        // point back to this very scope, which would recurse forever.
        let mut keys: Vec<&String> = self.bindings.keys().collect();
        keys.sort();
        f.debug_struct("ScopeData")
            .field("bindings", &keys)
            .field("has_parent", &self.parent.is_some())
            .finish()
    }
}

impl PartialEq for Scope {
    fn eq(&self, other: &Self) -> bool {
        // Optional performance shortcut: check if both Scopes share the exact same ScopeData instance
        if Rc::ptr_eq(&self.0, &other.0) {
            return true;
        }

        // Compare the inner ScopeData fields
        self.0.borrow().eq(&other.0.borrow())
    }
}

impl Eq for Scope {}





// ----------------------------------------------------------------------------
//  Undefined
// ----------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum Undef {

    /// The result of a unary operation that is not defined for the operand type.
    UnaryOperation {
        /// Operation name (always a `&'static str` for built-in ops).
        /// If dunder-method dispatch (`__neg__`, etc.) is added later and
        /// user-defined error labels are needed, change this to `Rc<str>`.
        op : &'static str,
        val: Box<Value>,
        pos: usize,
    },

    /// The result of a binary operation that is not defined for the operand types.
    BinaryOperation {
        op : &'static str,
        lhv: Box<Value>,
        rhv: Box<Value>,
        pos: usize,
    },

    /// The result of a grounp (...), [...] or {...}, wihtout closing parenthesig
    Termination {
        expected: char,
        pos: usize,
    },

    /// The result of an undefined token
    Token {
        token: String,
        pos: usize,
    },

    /// Used when a name is expected but another value is found
    Name {
        pos: usize
    },

    /// A generic undefined value used mostly for user-defined undefined values
    Value(Box<Value>),
}

impl PartialEq for Undef {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Undef::UnaryOperation{op:op1, val:val1, ..}, 
             Undef::UnaryOperation{op:op2, val:val2, ..}) => {
                op1 == op2 && val1 == val2
            }
            (Undef::BinaryOperation{op:op1, lhv:lhv1, rhv:rhv1, ..}, 
             Undef::BinaryOperation{op:op2, lhv:lhv2, rhv:rhv2, ..}) => {
                op1 == op2 && lhv1 == lhv2 && rhv1 == rhv2
            }
            (Undef::Termination{expected:exp1, ..}, 
             Undef::Termination{expected:exp2, ..}) => {
                exp1 == exp2
            }
            (Undef::Token{token:token1, ..}, 
             Undef::Token{token:token2, ..}) => {
                token1 == token2
            }
            (Undef::Name{..}, 
             Undef::Name{..}) => {
                true
            }
            (Undef::Value(val1), 
             Undef::Value(val2)) => {
                val1 == val2
            }
            _ => false,
        }
    }
}





// ----------------------------------------------------------------------------
//  BuiltinFn  (a native Rust function callable from Swan)
// ----------------------------------------------------------------------------

/// A native Rust function that can be called from Swan code.
///
/// # Why a newtype instead of a bare `Rc<dyn Fn>`?
///
/// 1. **Debug**: `dyn Fn` is not `Debug`; storing a `name` gives meaningful
///    output in error messages and test output.
/// 2. **Metadata**: the `name` field will be available when building
///    `Undefined` error values ("function `math.sin` called on a Bool").
/// 3. **Ergonomics**: `BuiltinFn::new("name", |arg| ...)` is self-documenting.
///
/// # Argument convention
///
/// The function receives the evaluated right-hand side as a single `Value`.
/// Multi-argument calls arrive as `Value::Tuple`.  The function is responsible
/// for destructuring the tuple when it expects more than one argument.
///
/// # Return value
///
/// The function must never panic.  Return `Value::Undef(...)` for any input
/// that is invalid.
#[derive(Clone)]
pub struct BuiltinFn {
    /// Human-readable name, used in Debug output and error messages.
    pub name: &'static str,
    func: Rc<dyn Fn(Value) -> Value>,
}

impl BuiltinFn {
    /// Create a new built-in function from any `Fn(Value) -> Value`.
    ///
    /// Both plain function pointers (`fn(Value) -> Value`) and closures are
    /// accepted because both coerce to `dyn Fn(Value) -> Value`.
    pub fn new(name: &'static str, f: impl Fn(Value) -> Value + 'static) -> Self {
        Self { name, func: Rc::new(f) }
    }

    /// Invoke the built-in with the given argument.
    #[inline]
    pub fn call(&self, arg: Value) -> Value {
        (self.func)(arg)
    }
}

impl std::fmt::Debug for BuiltinFn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<builtin {}>", self.name)
    }
}



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
mod scope_tests {
    use super::*;

    #[test]
    fn test_scope_new_and_set_get() {
        let scope = Scope::new();

        // Getting an unbound key returns Nothing
        assert_eq!(scope.get("a"), Value::Nothing);

        // Setting a binding and retrieving it
        scope.set("a", Value::Numb(42.0));
        assert_eq!(scope.get("a"), Value::Numb(42.0));
    }

    #[test]
    fn test_scope_with_parent() {
        let parent = Scope::new();
        parent.set("x", Value::Numb(10.0));

        let child = Scope::with_parent(parent);
        
        // Child can look up variables from parent
        assert_eq!(child.get("x"), Value::Numb(10.0));

        // Unbound key in both child and parent resolves to Nothing
        assert_eq!(child.get("y"), Value::Nothing);
    }

    #[test]
    fn test_scope_new_child() {
        let parent = Scope::new();
        parent.set("greeting", Value::Text("hello".into()));

        let child = parent.new_child();

        // Child inherits parent binding
        assert_eq!(child.get("greeting"), Value::Text("hello".into()));

        // Local set in child does not pollute parent
        child.set("local_var", Value::Bool(true));
        assert_eq!(child.get("local_var"), Value::Bool(true));
        assert_eq!(parent.get("local_var"), Value::Nothing);
    }

    #[test]
    fn test_scope_shadowing() {
        let parent = Scope::new();
        parent.set("x", Value::Numb(1.0));

        let child = parent.new_child();
        // Shadow 'x' in the child scope
        child.set("x", Value::Numb(2.0));

        // Child sees shadowed value, parent retains original value
        assert_eq!(child.get("x"), Value::Numb(2.0));
        assert_eq!(parent.get("x"), Value::Numb(1.0));
    }

    #[test]
    fn test_parent_mutation_visible_to_child() {
        let scope1 = Scope::new();
        let scope2 = scope1.new_child();

        // Modify parent AFTER child creation
        scope1.set("x", Value::Numb(10.0));

        // Child reflects post-creation modifications in parent
        assert_eq!(scope2.get("x"), Value::Numb(10.0));
    }

    #[test]
    fn test_deep_scope_chain() {
        let grand_parent = Scope::new();
        grand_parent.set("a", Value::Numb(1.0));

        let parent = grand_parent.new_child();
        parent.set("b", Value::Numb(2.0));

        let child = parent.new_child();
        child.set("c", Value::Numb(3.0));

        // Child can resolve up the entire chain
        assert_eq!(child.get("a"), Value::Numb(1.0));
        assert_eq!(child.get("b"), Value::Numb(2.0));
        assert_eq!(child.get("c"), Value::Numb(3.0));

        // Parent cannot see child bindings
        assert_eq!(parent.get("c"), Value::Nothing);
        assert_eq!(grand_parent.get("b"), Value::Nothing);
    }

    #[test]
    fn test_scope_equality_same_instance() {
        let scope = Scope::new();
        scope.set("a", Value::Numb(1.0));

        // A scope is equal to its clone (pointer identity shortcut)
        let clone = scope.clone();
        assert_eq!(scope, clone);
    }

    #[test]
    fn test_scope_equality_top_level() {
        let scope1 = Scope::new();
        scope1.set("a", Value::Numb(1.0));

        let scope2 = Scope::new();
        scope2.set("a", Value::Numb(1.0));

        // Both are top-level (parent: None) with equal bindings
        assert_eq!(scope1, scope2);

        // Modifying one breaks equality
        scope2.set("b", Value::Numb(2.0));
        assert_ne!(scope1, scope2);
    }

    #[test]
    fn test_scope_equality_shared_parent_instance() {
        let parent = Scope::new();
        parent.set("shared", Value::Bool(true));

        let child1 = parent.new_child();
        child1.set("x", Value::Numb(10.0));

        let child2 = parent.new_child();
        child2.set("x", Value::Numb(10.0));

        // Both children share the EXACT same parent reference and have equal bindings
        assert_eq!(child1, child2);
    }

    #[test]
    fn test_scope_equality_different_parent_instances() {
        // Parent 1 and Parent 2 have identical bindings, but are DIFFERENT memory instances
        let parent1 = Scope::new();
        parent1.set("shared", Value::Bool(true));

        let parent2 = Scope::new();
        parent2.set("shared", Value::Bool(true));

        let child1 = parent1.new_child();
        child1.set("x", Value::Numb(10.0));

        let child2 = parent2.new_child();
        child2.set("x", Value::Numb(10.0));

        // Must NOT be equal because parents are different instances (Rc::ptr_eq fails)
        assert_ne!(child1, child2);
    }

    #[test]
    fn test_scope_lift() {
        let root = Scope::new();
        root.set("x", Value::Numb(1.0));
        root.set("y", Value::Text("root".into()));

        let parent = root.new_child();
        parent.set("x", Value::Numb(2.0));
        parent.set("z", Value::Bool(true));

        let child = parent.new_child();
        child.set("x", Value::Numb(3.0));

        // 1. `x` is defined at root, parent, and child. 
        //    `lift` should return the root's value (1.0), unlike `get` which returns 3.0.
        assert_eq!(child.lift("x"), Value::Numb(1.0));
        assert_eq!(child.get("x"), Value::Numb(3.0));

        // 2. `y` is only defined at root.
        assert_eq!(child.lift("y"), Value::Text("root".into()));

        // 3. `z` is defined in parent and child chain, but not root.
        //    `lift` finds the top-most occurrence down the chain (the parent's value).
        assert_eq!(child.lift("z"), Value::Bool(true));

        // 4. Looking up a non-existent binding returns `Value::Nothing`.
        assert_eq!(child.lift("unbound"), Value::Nothing);
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
