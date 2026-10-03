
//! The `list` built-in namespace.
//!
//! Every function here is wrapped with [`map_1`] or [`map_curried`], so it is
//! automatically mapped element-wise over tuple arguments, and `Nothing`
//! arguments yield `Nothing`. The plain Rust functions below therefore only
//! need to handle a single (non-tuple) argument (or pair of arguments).
//! Arguments of the wrong type produce `Value::Undefined`.

use std::rc::Rc;
use crate::value::{Scope, Value};
use super::helpers::*;



// ----------------------------------------------------------------------------
//  `list` built-in namespace
// ----------------------------------------------------------------------------

/// Builds the `list` namespace as a child scope of `parent`.
pub fn create_namespace(parent: &Scope) -> Value {
    let list_scope = parent.new_child();

    // Unary functions: list -> value
    list_scope.set("size"   , map_1("list.size"   , list_size));
    list_scope.set("enum"   , map_1("list.enum"   , list_enum));
    list_scope.set("reverse", map_1("list.reverse", list_reverse));

    // Curried functions: (item | count) -> list -> value
    list_scope.set("find"   , map_curried("list.find"   , list_find));
    list_scope.set("rfind"  , map_curried("list.rfind"  , list_rfind));
    list_scope.set("head"   , map_curried("list.head"   , list_head));
    list_scope.set("tail"   , map_curried("list.tail"   , list_tail));

    Value::Namespace(list_scope)
}

/// Returns the number of elements in a list. Undefined for non-lists.
fn list_size(val: Value) -> Value {
    match val {
        Value::List(l) => Value::Numb(l.len() as f64),
        other => 
            Value::undef_unary("list.size", other, 0),
    }
}

/// Expands a list into a tuple of its elements (an empty list gives `Nothing`, a one-element list gives that element). Undefined for non-lists.
fn list_enum(val: Value) -> Value {
    match val {
        Value::List(l) => normalize_tuple(l.as_ref().clone()),
        other => Value::undef_unary("list.enum", other, 0),
    }
}

/// Returns a new list with the elements in reverse order. Undefined for non-lists.
fn list_reverse(val: Value) -> Value {
    match val {
        Value::List(l) => {
            let mut rev = l.as_ref().clone();
            rev.reverse();
            Value::List(Rc::new(rev))
        }
        other => Value::undef_unary("list.reverse", other, 0),
    }
}

/// Returns the index of the first element equal to `item`, or `-1` if absent.
/// Undefined if `list` is not a list.
fn list_find(item: Value, list: Value) -> Value {
    match list {
        Value::List(l) => Value::Numb(
            l.iter().position(|x| x == &item).map_or(-1.0, |i| i as f64)
        ),
        other => Value::undef_unary("list.find", other, 0),
    }
}

/// Returns the index of the last element equal to `item`, or `-1` if absent.
/// Undefined if `list` is not a list.
fn list_rfind(item: Value, list: Value) -> Value {
    match list {
        Value::List(l) => Value::Numb(
            l.iter().rposition(|x| x == &item).map_or(-1.0, |i| i as f64)
        ),
        other => Value::undef_unary("list.rfind", other, 0),
    }
}

/// Resolves a signed element count `n` against a list of length `len`.
///
/// - `n >= 0`: the count is clamped to `len`.
/// - `n < 0`: counts back from the end, i.e. `len + n`, clamped to `0`.
///
/// Fractional counts are truncated toward zero (and `NaN` becomes `0`).
fn resolve_count(n: f64, len: usize) -> usize {
    let len = len as i64;
    let n = n as i64;
    if n >= 0 {
        n.min(len) as usize
    } else {
        (len + n).max(0) as usize
    }
}

/// Returns the first `n` elements of a list as a new list.
/// A negative `n` drops the last `|n|` elements instead.
/// Undefined unless `n` is a number and `list` is a list.
fn list_head(n: Value, list: Value) -> Value {
    match (&n, list) {
        (Value::Numb(n), Value::List(l)) => {
            let end = resolve_count(*n, l.len());
            Value::List(Rc::new(l[..end].to_vec()))
        }
        (_, other) => Value::undef_unary("list.head", other, 0),
    }
}

/// Returns the list without its first `n` elements.
/// A negative `n` keeps only the last `|n|` elements instead.
/// Undefined unless `n` is a number and `list` is a list.
fn list_tail(n: Value, list: Value) -> Value {
    match (&n, list) {
        (Value::Numb(n), Value::List(l)) => {
            let start = resolve_count(*n, l.len());
            Value::List(Rc::new(l[start..].to_vec()))
        }
        (_, other) => Value::undef_unary("list.tail", other, 0),
    }
}



// ----------------------------------------------------------------------------
//  UNIT TESTS
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evaluator::eval;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    /// Creates a scope containing the `list` namespace and the `TRUE`/`FALSE` constants, evaluated in a child scope like real user code.
    fn root_scope() -> Scope {
        let scope = Scope::new();
        scope.set("list" , create_namespace(&scope));
        scope.set("TRUE" , Value::Bool(true));
        scope.set("FALSE", Value::Bool(false));
        scope.new_child()
    }

    /// Parses and evaluates `src` in a fresh test scope.
    fn run(src: &str) -> Value {
        let scope = root_scope();
        let mut parser = Parser::new(Lexer::new(src));
        let ast = parser.parse();
        eval(&ast, &scope)
    }

    #[test]
    fn list_size() {

        // List
        assert_eq!(run("list.size [1,2,3]"), Value::Numb(3.0));
        assert_eq!(run("list.size []     "), Value::Numb(0.0));

        // Nothing (also when paired with a non-list, since Nothing short-circuits)
        assert_eq!(run("list.size()"), Value::Nothing);

        // Tuple
        assert_eq!(
            run("list.size([1,2,3], [4,5,6,7,8])"), 
            Value::Tuple(Rc::new(vec![
                Value::Numb(3.0),
                Value::Numb(5.0),
            ]))
        );

        // Others
        assert!(matches!(run("list.size 123"), Value::Undefined(_)));
    }

    #[test]
    fn list_enum() {

        // List
        assert_eq!(
            run("list.enum [1,2,3]"),
            Value::Tuple(Rc::new(vec![
                Value::Numb(1.0),
                Value::Numb(2.0),
                Value::Numb(3.0),
            ]))
        );

        // Nothing (also when paired with a non-list, since Nothing short-circuits)
        assert_eq!(run("list.enum()"), Value::Nothing);

        // Tuple
        assert_eq!(
            run("list.enum([1,2,3], [4,5])"), 
            Value::Tuple(Rc::new(vec![
                Value::Numb(1.0),
                Value::Numb(2.0),
                Value::Numb(3.0),
                Value::Numb(4.0),
                Value::Numb(5.0),
            ]))
        );

        // Others
        assert!(matches!(run("list.enum 123"), Value::Undefined(_)));
    }

    #[test]
    fn list_reverse() {
        
        // List
        assert_eq!(
            run("list.reverse[3,2,1]"),
            Value::List(Rc::new(vec![
                Value::Numb(1.0),
                Value::Numb(2.0),
                Value::Numb(3.0),
            ]))
        );
        
        // Nothing (also when paired with a non-list, since Nothing short-circuits)
        assert_eq!(run("list.reverse()"), Value::Nothing);

        // Tuple
        assert_eq!(
            run("list.reverse([1,2], [3,4])"),
            Value::Tuple(Rc::new(vec![
                Value::List(Rc::new(vec![Value::Numb(2.0), Value::Numb(1.0)])),
                Value::List(Rc::new(vec![Value::Numb(4.0), Value::Numb(3.0)])),
            ]))
        );

        // Others
        assert!(matches!(run("list.reverse 123"), Value::Undefined(_)));
    }

    #[test]
    fn list_find() {

        // List
        assert_eq!(run("list.find 2 [1,2,3,4,1,2,3,4]"), Value::Numb(1.0));
        assert_eq!(run("list.find 7 [1,2,3,4,1,2,3,4]"), Value::Numb(-1.0));
        assert_eq!(run("list.find 1 []"), Value::Numb(-1.0));

        // Nothing (also when paired with a non-list, since Nothing short-circuits)
        assert_eq!(run("list.find ()"), Value::Nothing);
        assert_eq!(run("list.find () \"abc\""), Value::Nothing);
        assert_eq!(run("list.find () ()"), Value::Nothing);

        // Tuple - List: each item/count is applied to the same list
        assert_eq!(
            run("list.find (2,4) [1,2,3,4,5,1,2,3,4,5]"), 
            Value::Tuple(Rc::new(vec![
                Value::Numb(1.0),
                Value::Numb(3.0),
            ]))
        );

        // Tuple - Tuple: items/counts are paired with lists; results are flattened in order
        assert_eq!(
            run("list.find (2,4) ([1,2,3,4,5,1,2,3,4,5], [2,4,6,2,4,6])"), 
            Value::Tuple(Rc::new(vec![
                Value::Numb(1.0),
                Value::Numb(0.0),
                Value::Numb(3.0),
                Value::Numb(1.0),
            ]))
        );

        // Others
        assert!(matches!(run("list.find 123 123"), Value::Undefined(_)));
    }

    #[test]
    fn list_rfind() {

        // List
        assert_eq!(run("list.rfind 2 [1,2,3,4,1,2,3,4]"), Value::Numb(5.0));
        assert_eq!(run("list.rfind 7 [1,2,3,4,1,2,3,4]"), Value::Numb(-1.0));
        assert_eq!(run("list.rfind 1 []"), Value::Numb(-1.0));

        // Nothing (also when paired with a non-list, since Nothing short-circuits)
        assert_eq!(run("list.rfind ()"), Value::Nothing);
        assert_eq!(run("list.rfind () \"abc\""), Value::Nothing);
        assert_eq!(run("list.rfind () ()"), Value::Nothing);

        // Tuple - List: each item/count is applied to the same list
        assert_eq!(
            run("list.rfind (2,4) [1,2,3,4,5,1,2,3,4,5]"), 
            Value::Tuple(Rc::new(vec![
                Value::Numb(6.0),
                Value::Numb(8.0),
            ]))
        );

        // Tuple - Tuple: items/counts are paired with lists; results are flattened in order
        assert_eq!(
            run("list.rfind (2,4) ([1,2,3,4,5,1,2,3,4,5], [2,4,6,2,4,6])"), 
            Value::Tuple(Rc::new(vec![
                Value::Numb(6.0),
                Value::Numb(3.0),
                Value::Numb(8.0),
                Value::Numb(4.0),
            ]))
        );

        // Others
        assert!(matches!(run("list.rfind 123 123"), Value::Undefined(_)));
    }

    #[test]
    fn list_head() {

        // Numb - List
        assert_eq!(
            run("list.head 3 [1,2,3,4,5,6]"), 
            Value::List(Rc::new(vec![
                Value::Numb(1.0),
                Value::Numb(2.0),
                Value::Numb(3.0),
            ]))
        );
        assert_eq!(
            run("list.head 0 [1,2,3,4,5,6]"), 
            Value::List(Rc::new(vec![]))
        );
        assert_eq!(
            run("list.head(-2) [1,2,3,4,5,6]"), 
            Value::List(Rc::new(vec![
                Value::Numb(1.0),
                Value::Numb(2.0),
                Value::Numb(3.0),
                Value::Numb(4.0),
            ]))
        );
        // Counts beyond the list length are clamped
        assert_eq!(
            run("list.head 10 [1,2,3]"),
            Value::List(Rc::new(vec![Value::Numb(1.0), Value::Numb(2.0), Value::Numb(3.0)]))
        );
        assert_eq!(run("list.head(-10) [1,2,3]"), Value::List(Rc::new(vec![])));

        // Nothing (also when paired with a non-list, since Nothing short-circuits)
        assert_eq!(run("list.head ()"), Value::Nothing);
        assert_eq!(run("list.head () [1,2,3]"), Value::Nothing);
        assert_eq!(run("list.head () ()"), Value::Nothing);

        // Tuple - List: each item/count is applied to the same list
        assert_eq!(
            run("list.head (2, 3) [1,2,3,4,5,6]"), 
            Value::Tuple(Rc::new(vec![
                Value::List(Rc::new(vec![
                    Value::Numb(1.0),
                    Value::Numb(2.0),
                ])),
                Value::List(Rc::new(vec![
                    Value::Numb(1.0),
                    Value::Numb(2.0),
                    Value::Numb(3.0),
                ])),
            ]))
        );

        // Tuple - Tuple: items/counts are paired with lists; results are flattened in order
        assert_eq!(
            run("list.head (2, 3) ([1,2,3,4,5], [6,7,8,9])"), 
            Value::Tuple(Rc::new(vec![
                Value::List(Rc::new(vec![
                    Value::Numb(1.0),
                    Value::Numb(2.0),
                ])),
                Value::List(Rc::new(vec![
                    Value::Numb(6.0),
                    Value::Numb(7.0),
                ])),
                Value::List(Rc::new(vec![
                    Value::Numb(1.0),
                    Value::Numb(2.0),
                    Value::Numb(3.0),
                ])),
                Value::List(Rc::new(vec![
                    Value::Numb(6.0),
                    Value::Numb(7.0),
                    Value::Numb(8.0),
                ])),
            ]))
        );

        // Others
        assert!(matches!(run("list.head 123 456"), Value::Undefined(_)));
        assert!(matches!(run("list.head TRUE 123"), Value::Undefined(_)));
    }

    #[test]
    fn list_tail() {

        // Numb - List
        assert_eq!(
            run("list.tail 4 [1,2,3,4,5,6]"), 
            Value::List(Rc::new(vec![
                Value::Numb(5.0),
                Value::Numb(6.0),
            ]))
        );
        assert_eq!(
            run("list.tail 0 [1,2,3,4,5,6]"), 
            Value::List(Rc::new(vec![
                Value::Numb(1.0),
                Value::Numb(2.0),
                Value::Numb(3.0),
                Value::Numb(4.0),
                Value::Numb(5.0),
                Value::Numb(6.0),
            ]))
        );
        assert_eq!(
            run("list.tail(-2) [1,2,3,4,5,6]"), 
            Value::List(Rc::new(vec![
                Value::Numb(5.0),
                Value::Numb(6.0),
            ]))
        );
        // Counts beyond the list length are clamped
        assert_eq!(run("list.tail 10 [1,2,3]"), Value::List(Rc::new(vec![])));
        assert_eq!(
            run("list.tail(-10) [1,2,3]"),
            Value::List(Rc::new(vec![Value::Numb(1.0), Value::Numb(2.0), Value::Numb(3.0)]))
        );

        // Nothing (also when paired with a non-list, since Nothing short-circuits)
        assert_eq!(run("list.tail ()"), Value::Nothing);
        assert_eq!(run("list.tail () [1,2,3]"), Value::Nothing);
        assert_eq!(run("list.tail () ()"), Value::Nothing);

        // Tuple - List: each item/count is applied to the same list
        assert_eq!(
            run("list.tail (5, 4) [1,2,3,4,5,6]"), 
            Value::Tuple(Rc::new(vec![
                Value::List(Rc::new(vec![
                    Value::Numb(6.0),
                ])),
                Value::List(Rc::new(vec![
                    Value::Numb(5.0),
                    Value::Numb(6.0),
                ])),
            ]))
        );

        // Tuple - Tuple: items/counts are paired with lists; results are flattened in order
        assert_eq!(
            run("list.tail (3, 2) ([1,2,3,4,5], [6,7,8,9,10])"), 
            Value::Tuple(Rc::new(vec![
                Value::List(Rc::new(vec![
                    Value::Numb(4.0),
                    Value::Numb(5.0),
                ])),
                Value::List(Rc::new(vec![
                    Value::Numb(9.0),
                    Value::Numb(10.0),
                ])),
                Value::List(Rc::new(vec![
                    Value::Numb(3.0),
                    Value::Numb(4.0),
                    Value::Numb(5.0),
                ])),
                Value::List(Rc::new(vec![
                    Value::Numb(8.0),
                    Value::Numb(9.0),
                    Value::Numb(10.0),
                ])),
            ]))
        );

        // Others
        assert!(matches!(run("list.tail 123 456"), Value::Undefined(_)));
        assert!(matches!(run("list.tail TRUE 123"), Value::Undefined(_)));
    }
}

