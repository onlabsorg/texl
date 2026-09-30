This file is a merged representation of the entire codebase, combined into a single document by Repomix.

# File Summary

## Purpose
This file contains a packed representation of the entire repository's contents.
It is designed to be easily consumable by AI systems for analysis, code review,
or other automated processes.

## File Format
The content is organized as follows:
1. This summary section
2. Repository information
3. Directory structure
4. Repository files (if enabled)
5. Multiple file entries, each consisting of:
  a. A header with the file path (## File: path/to/file)
  b. The full contents of the file in a code block

## Usage Guidelines
- This file should be treated as read-only. Any changes should be made to the
  original repository files, not this packed version.
- When processing this file, use the file path to distinguish
  between different files in the repository.
- Be aware that this file may contain sensitive information. Handle it with
  the same level of security as you would the original repository.

## Notes
- Some files may have been excluded based on .gitignore rules and Repomix's configuration
- Binary files are not included in this packed representation. Please refer to the Repository Structure section for a complete list of file paths, including binary files
- Files matching patterns in .gitignore are excluded
- Files matching default ignore patterns are excluded
- Files are sorted by Git change count (files with more changes are at the bottom)

# Directory Structure
````
src/
  builtins/
    bool_.rs
    helpers.rs
    list.rs
    math.rs
    nspace.rs
    numb.rs
    text.rs
    time.rs
    undef.rs
  ast.rs
  builtins.rs
  eval.rs
  lexer.rs
  lib.rs
  parser.rs
  token.rs
  value.rs
Cargo.toml
README.md
````

# Files

## File: src/builtins/bool_.rs
````rust
//! The `bool` built-in namespace.
//!
//! Provides:
//! - `bool.TRUE` / `bool.FALSE`: the boolean constants.
//! - `bool.from`: converts a value to a `Bool`, element-wise over tuples.
//! - `bool.any`:  `true` if at least one item of its argument is truthy.
//! - `bool.all`:  `true` if every item of its argument is truthy.
//!
//! Truthiness itself is defined by `impl From<Value> for bool`.
use crate::value::{BuiltinFn, Scope, Value};
use super::helpers::*;



// ----------------------------------------------------------------------------
//  `bool` built-in namespace
// ----------------------------------------------------------------------------

/// Builds the `bool` namespace as a child of `parent`.
///
/// `from` is wrapped with `map_1`, so it maps over tuples and returns a tuple
/// of booleans. `any` and `all` are plain `BuiltinFn`s because they reduce
/// their whole argument to a single `Bool`.
pub fn create_namespace(parent: &Scope) -> Value {
    let bool_scope = parent.new_child();
    bool_scope.set("TRUE" , Value::Bool(true));
    bool_scope.set("FALSE", Value::Bool(false));
    bool_scope.set("from" , map_1("bool.from", bool_from));
    bool_scope.set("any"  , Value::Builtin(BuiltinFn::new("bool.any", bool_any)));
    bool_scope.set("all"  , Value::Builtin(BuiltinFn::new("bool.all", bool_all)));
    Value::Namespace(bool_scope)
}

/// `bool.from x`: converts a single value to `Bool` using its truthiness.
/// Tuple and `Nothing` handling is done by the `map_1` wrapper.
fn bool_from(val: Value) -> Value {
    Value::Bool(bool::from(val))
}

/// `bool.any xs`: `true` if at least one item of `xs` is truthy.
/// `Nothing` has no items, so `bool.any()` is `false`.
fn bool_any(val: Value) -> Value {
    Value::Bool(val.items().any(bool::from))
}

/// `bool.all xs`: `true` if every item of `xs` is truthy.
/// `Nothing` has no items, so `bool.all()` is vacuously `true`.
fn bool_all(val: Value) -> Value {
    Value::Bool(val.items().all(bool::from))
}



// ----------------------------------------------------------------------------
//  Unit tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;
    use crate::eval::eval;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    /// Minimal root scope: the `bool` namespace plus the global boolean
    /// constants. Returns a child scope, mirroring how user code is evaluated.
    fn root_scope() -> Scope {
        let scope = Scope::new();
        scope.set("bool" , create_namespace(&scope));
        scope.set("TRUE" , Value::Bool(true));
        scope.set("FALSE", Value::Bool(false));
        scope.new_child()
    }

    /// Lexes, parses and evaluates `src` in a fresh test scope.
    fn run(src: &str) -> Value {
        let scope = root_scope();
        let mut parser = Parser::new(Lexer::new(src));
        let ast = parser.parse();
        eval(&ast, &scope)
    }

    #[test]
    fn test_constants() {
        assert_eq!(run("bool.TRUE"),  Value::Bool(true));
        assert_eq!(run("bool.FALSE"), Value::Bool(false));
    }

    #[test]
    fn test_from() {

        // Bool
        assert_eq!(run("bool.from(TRUE)"),  Value::Bool(true));

        // Numb
        assert_eq!(run("bool.from 1"),      Value::Bool(true));
        assert_eq!(run("bool.from 0"),      Value::Bool(false));

        // Text
        assert_eq!(run("bool.from \"abc\""),Value::Bool(true));
        assert_eq!(run("bool.from \"\""),   Value::Bool(false));

        // List
        assert_eq!(run("bool.from [1,2,3]"),Value::Bool(true));
        assert_eq!(run("bool.from []"),     Value::Bool(false));

        // Namespace
        assert_eq!(run("bool.from {a:1}"),  Value::Bool(true));
        assert_eq!(run("bool.from {}"),     Value::Bool(false));
        
        // Func
        assert_eq!(run("bool.from(x->2*x)"),Value::Bool(true));
        assert_eq!(run("bool.from(bool.from)"), Value::Bool(true));

        // Nothing
        assert_eq!(run("bool.from()"),      Value::Nothing);

        // Tuple: converted element-wise, not reduced
        assert_eq!(
            run("bool.from(1, [])"), 
            Value::Tuple(Rc::new(vec![
                Value::Bool(true),
                Value::Bool(false),
            ])),
        );
    }

    #[test]
    fn test_any() {

        // Bool
        assert_eq!(run("bool.any(TRUE)"), Value::Bool(true));

        // Numb
        assert_eq!(run("bool.any 1"),     Value::Bool(true));
        assert_eq!(run("bool.any 0"),     Value::Bool(false));

        // Text
        assert_eq!(run("bool.any \"abc\""), Value::Bool(true));
        assert_eq!(run("bool.any \"\""),   Value::Bool(false));

        // List
        assert_eq!(run("bool.any [1,2,3]"), Value::Bool(true));
        assert_eq!(run("bool.any []"),      Value::Bool(false));

        // Namespace
        assert_eq!(run("bool.any {a:1}"),  Value::Bool(true));
        assert_eq!(run("bool.any {}"),     Value::Bool(false));
        
        // Func
        assert_eq!(run("bool.any(x->2*x)"), Value::Bool(true));
        assert_eq!(run("bool.any(bool.any)"), Value::Bool(true));

        // Nothing: no items, so no truthy item
        assert_eq!(run("bool.any()"), Value::Bool(false));

        // Tuple: items are inspected individually
        assert_eq!(run("bool.any(1,0,0,1)"), Value::Bool(true));
        assert_eq!(run("bool.any(0,0,0,0)"), Value::Bool(false));
        assert_eq!(run("bool.any(0,0,1)"), Value::Bool(true));
    }

    #[test]
    fn test_all() {

        // Bool
        assert_eq!(run("bool.all(TRUE)"), Value::Bool(true));

        // Numb
        assert_eq!(run("bool.all 1"),     Value::Bool(true));
        assert_eq!(run("bool.all 0"),     Value::Bool(false));

        // Text
        assert_eq!(run("bool.all \"abc\""), Value::Bool(true));
        assert_eq!(run("bool.all \"\""),   Value::Bool(false));

        // List
        assert_eq!(run("bool.all [1,2,3]"), Value::Bool(true));
        assert_eq!(run("bool.all []"),      Value::Bool(false));

        // Namespace
        assert_eq!(run("bool.all {a:1}"),  Value::Bool(true));
        assert_eq!(run("bool.all {}"),     Value::Bool(false));
        
        // Func
        assert_eq!(run("bool.all(x->2*x)"), Value::Bool(true));
        assert_eq!(run("bool.all(bool.all)"), Value::Bool(true));

        // Nothing: no items, so vacuously true
        assert_eq!(run("bool.all()"), Value::Bool(true));

        // Tuple: items are inspected individually
        assert_eq!(run("bool.all(1,0,0,1)"), Value::Bool(false));
        assert_eq!(run("bool.all(0,0,0,0)"), Value::Bool(false));
        assert_eq!(run("bool.all(1,1,1,1)"), Value::Bool(true));
        assert_eq!(run("bool.all(1,1,0)"), Value::Bool(false));
    }
}
````

## File: src/builtins/helpers.rs
````rust
use std::rc::Rc;
use crate::value::{BuiltinFn, Value};



// ----------------------------------------------------------------------------
//  Macros
// ----------------------------------------------------------------------------

#[macro_export]
macro_rules! undef_unary {
    ($op:expr, $val:expr) => {
        Value::Undefined(Undef::UnaryOperation {
            op: $op,
            val: Box::new($val),
            pos: 0,
        })
    };
}

pub(crate) use undef_unary;






// ----------------------------------------------------------------------------
//  PRIVATE HELPERS
// ----------------------------------------------------------------------------

/// Normalizes a sequence of items:
/// - 0 items -> `Value::Nothing`
/// - 1 item  -> `items[0]`
/// - >=2     -> `Value::Tuple(Rc::new(items))`
pub fn normalize_tuple(items: Vec<Value>) -> Value {
    match items.len() {
        0 => Value::Nothing,
        _ => Value::Tuple(Rc::new(items)).normalize(),
    }
}

/// Wraps a single-argument built-in function `fn(Value) -> Value` so that
/// if applied to a `Tuple`, it maps over each element and returns a `Tuple`.
pub fn map_1(name: &'static str, f: impl Fn(Value) -> Value + 'static + Clone) -> Value {
    Value::Builtin(BuiltinFn::new(name, move |arg| arg.map(f.clone())))
}

/// Wraps a curried 2-argument built-in function so that if given tuples,
/// it pairs the tuple elements using `arg1.pair(&arg2)` and returns a mapped `Tuple`.
pub fn map_curried(
    name: &'static str,
    f: impl Fn(Value, Value) -> Value + 'static + Copy,
) -> Value {
    map_1(name, move |arg1| {
        map_1(name, move |arg2| f(arg1.clone(), arg2))
    })
}
````

## File: src/builtins/list.rs
````rust
//! The `list` built-in namespace.
//!
//! Every function here is wrapped with [`map_1`] or [`map_curried`], so it is
//! automatically mapped element-wise over tuple arguments, and `Nothing`
//! arguments yield `Nothing`. The plain Rust functions below therefore only
//! need to handle a single (non-tuple) argument (or pair of arguments).
//! Arguments of the wrong type produce `Value::Undefined`.

use std::rc::Rc;
use crate::value::{Scope, Undef, Value};
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
        other => undef_unary!("list.size", other),
    }
}

/// Expands a list into a tuple of its elements (an empty list gives `Nothing`, a one-element list gives that element). Undefined for non-lists.
fn list_enum(val: Value) -> Value {
    match val {
        Value::List(l) => normalize_tuple(l.as_ref().clone()),
        other => undef_unary!("list.enum", other),
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
        other => undef_unary!("list.reverse", other),
    }
}

/// Returns the index of the first element equal to `item`, or `-1` if absent.
/// Undefined if `list` is not a list.
fn list_find(item: Value, list: Value) -> Value {
    match list {
        Value::List(l) => Value::Numb(
            l.iter().position(|x| x == &item).map_or(-1.0, |i| i as f64)
        ),
        other => undef_unary!("list.find", other),
    }
}

/// Returns the index of the last element equal to `item`, or `-1` if absent.
/// Undefined if `list` is not a list.
fn list_rfind(item: Value, list: Value) -> Value {
    match list {
        Value::List(l) => Value::Numb(
            l.iter().rposition(|x| x == &item).map_or(-1.0, |i| i as f64)
        ),
        other => undef_unary!("list.rfind", other),
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
        (_, other) => undef_unary!("list.head", other),
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
        (_, other) => undef_unary!("list.tail", other),
    }
}



// ----------------------------------------------------------------------------
//  UNIT TESTS
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::eval;
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
````

## File: src/builtins/math.rs
````rust
use crate::value::{BuiltinFn, Scope, Undef, Value};

use super::helpers::*;



// ----------------------------------------------------------------------------
//  `math` built-in namespace
// ----------------------------------------------------------------------------

/// Constructs and populates the `math` built-in namespace.
///
/// Returns a [`Value::Namespace`] scope containing trigonometric, logarithmic,
/// exponential, rounding, and utility functions, alongside fundamental constants.
///
/// # Curried Two-Argument Functions
/// Functions expecting two arguments (such as `pow`, `log`, `atan2`, `hypot`) are
/// implemented as curried higher-order functions $f(x) \to g(y)$. Calling $f(x)$
/// returns a callable closure expecting $y$.
///
/// # Example
/// ```text
/// pow10 = math.pow 10;
/// pow10 2  # returns 1024.0 (2 ^ 10)
/// ```
pub fn create_namespace(parent: &Scope) -> Value {
    let math_scope = parent.new_child();

    // --- Single-argument math functions (f64 -> f64) ---
    math_scope.set("sqrt",  builtin_f64_1("math.sqrt",  f64::sqrt));
    math_scope.set("abs",   builtin_f64_1("math.abs",   f64::abs));
    math_scope.set("floor", builtin_f64_1("math.floor", f64::floor));
    math_scope.set("ceil",  builtin_f64_1("math.ceil",  f64::ceil));
    math_scope.set("round", builtin_f64_1("math.round", f64::round));
    math_scope.set("trunc", builtin_f64_1("math.trunc", f64::trunc));

    // Trigonometric functions
    math_scope.set("sin",   builtin_f64_1("math.sin",   f64::sin));
    math_scope.set("cos",   builtin_f64_1("math.cos",   f64::cos));
    math_scope.set("tan",   builtin_f64_1("math.tan",   f64::tan));
    math_scope.set("asin",  builtin_f64_1("math.asin",  f64::asin));
    math_scope.set("acos",  builtin_f64_1("math.acos",  f64::acos));
    math_scope.set("atan",  builtin_f64_1("math.atan",  f64::atan));

    // Hyperbolic functions
    math_scope.set("sinh",  builtin_f64_1("math.sinh",  f64::sinh));
    math_scope.set("cosh",  builtin_f64_1("math.cosh",  f64::cosh));
    math_scope.set("tanh",  builtin_f64_1("math.tanh",  f64::tanh));
    math_scope.set("asinh", builtin_f64_1("math.asinh", f64::asinh));
    math_scope.set("acosh", builtin_f64_1("math.acosh", f64::acosh));
    math_scope.set("atanh", builtin_f64_1("math.atanh", f64::atanh));

    // Exponentials and Logarithms
    math_scope.set("exp",   builtin_f64_1("math.exp",   f64::exp));
    math_scope.set("ln",    builtin_f64_1("math.ln",    f64::ln));
    math_scope.set("log2",  builtin_f64_1("math.log2",  f64::log2));
    math_scope.set("log10", builtin_f64_1("math.log10", f64::log10));

    // --- Curried two-argument math functions (f64 -> f64 -> f64) ---
    // `pow`:  pow(exp) -> g(base) => base.powf(exp)
    math_scope.set("pow",   builtin_f64_curried("math.pow",   |exp, base| base.powf(exp)));
    // `log`:  log(base) -> g(x) => x.log(base)
    math_scope.set("log",   builtin_f64_curried("math.log",   |base, x| x.log(base)));
    // `atan2`: atan2(x) -> g(y) => y.atan2(x)
    math_scope.set("atan2", builtin_f64_curried("math.atan2", |x, y| y.atan2(x)));
    // `hypot`: hypot(x) -> g(y) => x.hypot(y)
    math_scope.set("hypot", builtin_f64_curried("math.hypot", |x, y| x.hypot(y)));

    // --- Variadic / Multi-argument math functions ---
    math_scope.set("max", Value::Builtin(BuiltinFn::new("math.max", math_max)));
    math_scope.set("min", Value::Builtin(BuiltinFn::new("math.min", math_min)));

    // --- Mathematical constants ---
    math_scope.set("PI",       Value::Numb(std::f64::consts::PI));
    math_scope.set("E",        Value::Numb(std::f64::consts::E));
    math_scope.set("INF",      Value::Numb(f64::INFINITY));
    math_scope.set("INFINITY", Value::Numb(f64::INFINITY));

    Value::Namespace(math_scope)
}

/// Evaluates `math.max` over a single scalar or a tuple of numeric values.
///
/// Iterates over all values yielded by `arg.items()`. Returns the maximum number.
///
/// # Errors
/// Returns `Value::Undefined` if:
/// - Any element in `arg` is not a `Value::Numb`.
/// - The input tuple/argument yields zero elements.
fn math_max(arg: Value) -> Value {
    let mut count = 0;
    let mut max_val = f64::NEG_INFINITY;

    for item in arg.items() {
        match item {
            Value::Numb(n) => {
                if n > max_val {
                    max_val = n;
                }
                count += 1;
            }
            _ => return undef_unary!("math.max", arg),
        }
    }

    if count == 0 {
        undef_unary!("math.max", arg)
    } else {
        Value::Numb(max_val)
    }
}

/// Evaluates `math.min` over a single scalar or a tuple of numeric values.
///
/// Iterates over all values yielded by `arg.items()`. Returns the minimum number.
///
/// # Errors
/// Returns `Value::Undefined` if:
/// - Any element in `arg` is not a `Value::Numb`.
/// - The input tuple/argument yields zero elements.
fn math_min(arg: Value) -> Value {
    let mut count = 0;
    let mut min_val = f64::INFINITY;

    for item in arg.items() {
        match item {
            Value::Numb(n) => {
                if n < min_val {
                    min_val = n;
                }
                count += 1;
            }
            _ => return undef_unary!("math.min", arg),
        }
    }

    if count == 0 {
        undef_unary!("math.min", arg)
    } else {
        Value::Numb(min_val)
    }
}



// ----------------------------------------------------------------------------
//  PRIVATE HELPERS FOR MATH BUILT-INS
// ----------------------------------------------------------------------------

/// Wraps a standard single-argument float function (`f64 -> f64`) into a [`Value::Builtin`].
///
/// - Accepts a single [`Value::Numb`] operand.
/// - Propagates `NaN` results (e.g., `math.sqrt(-1.0)`) or invalid argument types as `Value::Undefined`.
fn builtin_f64_1(name: &'static str, f: fn(f64) -> f64) -> Value {
    map_1(name, move |arg| {
        match arg {
            Value::Numb(x) => {
                let result = f(x);
                // Propagate NaN (e.g. sqrt(-1)) as Undefined.
                if result.is_nan() {
                    undef_unary!(name, Value::Numb(x))
                } else {
                    Value::Numb(result)
                }
            }
            other => undef_unary!(name, other),
        }
    })
}

/// Wraps a curried float function (`f64 -> f64 -> f64`) into a nested [`Value::Builtin`].
///
/// Calling `f(x)` produces a second function `g(y)` that computes `result(x, y)`.
/// Returns `Value::Undefined` if either argument is non-numeric or if the result evaluates to `NaN`.
fn builtin_f64_curried(name: &'static str, f: fn(f64, f64) -> f64) -> Value {
    map_curried(name, move |arg1, arg2| {

        match (&arg1, arg2) {
            (Value::Numb(a), Value::Numb(b)) => {
                let result = f(*a, b);
                if result.is_nan() {
                    undef_unary!(name, Value::Numb(b))
                } else {
                    Value::Numb(result)
                }
            }
            (_, other) => undef_unary!(name, other),
        }
    })
}



// ----------------------------------------------------------------------------
//  UNIT TESTS
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;
    use crate::eval::eval;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn root_scope() -> Scope {
        let scope = Scope::new();
        scope.set("math" , create_namespace(&scope));
        scope.set("TRUE" , Value::Bool(true));
        scope.set("FALSE", Value::Bool(false));
        scope.new_child()
    }

    fn run(src: &str) -> Value {
        let scope = root_scope();
        let mut parser = Parser::new(Lexer::new(src));
        let ast = parser.parse();
        eval(&ast, &scope)
    }


    // ---------------------------------------------------------------------
    //  Single-argument functions (builtin_f64_1)
    // ---------------------------------------------------------------------

    #[test]
    fn test_sqrt() {

        // Numbers
        assert_eq!(run("math.sqrt 9"), Value::Numb(3.0));
        assert_eq!(run("math.sqrt 4"), Value::Numb(2.0));

        // Undefined
        assert_eq!(run("math.sqrt(-1)")    , undef_unary!("math.sqrt", Value::Numb(-1.0)));
        assert_eq!(run("math.sqrt TRUE")   , undef_unary!("math.sqrt", Value::Bool(true)));
        assert_eq!(run("math.sqrt \"abc\""), undef_unary!("math.sqrt", Value::Text("abc".into())));

        // Nothing
        assert_eq!(run("math.sqrt()"), Value::Nothing);

        // Tuples
        assert_eq!(run("math.sqrt(9,4)"), Value::Tuple(Rc::new(vec![
            Value::Numb(3.0),
            Value::Numb(2.0),
        ])));
    }

    #[test]
    fn test_abs() {

        // Numbers
        assert_eq!(run("math.abs(-5)"), Value::Numb(5.0));
        assert_eq!(run("math.abs 3"),   Value::Numb(3.0));

        // Undefined
        assert_eq!(run("math.abs TRUE")   , undef_unary!("math.abs", Value::Bool(true)));
        assert_eq!(run("math.abs \"abc\""), undef_unary!("math.abs", Value::Text("abc".into())));

        // Nothing
        assert_eq!(run("math.abs()"), Value::Nothing);

        // Tuple
        assert_eq!(run("math.abs(-5,3)"), Value::Tuple(Rc::new(vec![
            Value::Numb(5.0),
            Value::Numb(3.0),
        ])));
    }

    #[test]
    fn test_floor() {
        // Numb
        assert_eq!(run("math.floor 3.7"), Value::Numb(3.0));
        assert_eq!(run("math.floor(-3.2)"), Value::Numb(-4.0));

        // Undefined (type error)
        assert!(matches!(run(r#"math.floor "3.7""#), Value::Undefined(_)));

        // Nothing
        assert!(matches!(run("math.floor()"), Value::Nothing));

        // Tuple
        assert_eq!(
            run("math.floor(1.2, 3.8)"),
            Value::Tuple(Rc::new(vec![Value::Numb(1.0), Value::Numb(3.0)]))
        );
    }

    #[test]
    fn test_ceil() {
        // Numb
        assert_eq!(run("math.ceil 3.2"), Value::Numb(4.0));
        assert_eq!(run("math.ceil(-3.7)"), Value::Numb(-3.0));

        // Undefined (type error)
        assert!(matches!(run("math.ceil [1, 2]"), Value::Undefined(_)));

        // Nothing
        assert!(matches!(run("math.ceil()"), Value::Nothing));

        // Tuple
        assert_eq!(
            run("math.ceil(1.2, 3.8)"),
            Value::Tuple(Rc::new(vec![Value::Numb(2.0), Value::Numb(4.0)]))
        );
    }

    #[test]
    fn test_round() {
        // Numb
        assert_eq!(run("math.round 3.5"), Value::Numb(4.0));
        assert_eq!(run("math.round 3.2"), Value::Numb(3.0));

        // Undefined (type error)
        assert!(matches!(run("math.round {x: 1}"), Value::Undefined(_)));

        // Nothing
        assert!(matches!(run("math.round()"), Value::Nothing));

        // Tuple
        assert_eq!(
            run("math.round(1.2, 3.8)"),
            Value::Tuple(Rc::new(vec![Value::Numb(1.0), Value::Numb(4.0)]))
        );
    }

    #[test]
    fn test_trunc() {
        // Numb
        assert_eq!(run("math.trunc 3.7"), Value::Numb(3.0));
        assert_eq!(run("math.trunc(-3.7)"), Value::Numb(-3.0));

        // Undefined (type error)
        assert!(matches!(run("math.trunc(bool.TRUE)"), Value::Undefined(_)));

        // Nothing
        assert!(matches!(run("math.trunc()"), Value::Nothing));

        // Tuple
        assert_eq!(
            run("math.trunc(1.9, -2.9)"),
            Value::Tuple(Rc::new(vec![Value::Numb(1.0), Value::Numb(-2.0)]))
        );
    }

    #[test]
    fn test_sin() {
        // Numb
        assert_eq!(run("math.sin 0"), Value::Numb(0.0));

        // Undefined (type error)
        assert!(matches!(run(r#"math.sin "zero""#), Value::Undefined(_)));

        // Nothing
        assert!(matches!(run("math.sin()"), Value::Nothing));

        // Tuple
        assert_eq!(
            run("math.sin(0, 0)"),
            Value::Tuple(Rc::new(vec![Value::Numb(0.0), Value::Numb(0.0)]))
        );
    }

    #[test]
    fn test_cos() {
        // Numb
        assert_eq!(run("math.cos 0"), Value::Numb(1.0));

        // Undefined (type error)
        assert!(matches!(run(r#"math.cos "zero""#), Value::Undefined(_)));

        // Nothing
        assert!(matches!(run("math.cos()"), Value::Nothing));

        // Tuple
        assert_eq!(
            run("math.cos(0, 0)"),
            Value::Tuple(Rc::new(vec![Value::Numb(1.0), Value::Numb(1.0)]))
        );
    }

    #[test]
    fn test_tan() {
        // Numb
        assert_eq!(run("math.tan 0"), Value::Numb(0.0));

        // Undefined (type error)
        assert!(matches!(run(r#"math.tan "zero""#), Value::Undefined(_)));

        // Nothing
        assert!(matches!(run("math.tan()"), Value::Nothing));

        // Tuple
        assert_eq!(
            run("math.tan(0, 0)"),
            Value::Tuple(Rc::new(vec![Value::Numb(0.0), Value::Numb(0.0)]))
        );
    }

    #[test]
    fn test_asin() {
        // Numb
        assert_eq!(run("math.asin 0"), Value::Numb(0.0));

        // Undefined (out of domain / type error)
        assert!(matches!(run("math.asin 2"), Value::Undefined(_)));
        assert!(matches!(run(r#"math.asin "0""#), Value::Undefined(_)));

        // Nothing
        assert!(matches!(run("math.asin()"), Value::Nothing));

        // Tuple
        assert_eq!(
            run("math.asin(0, 0)"),
            Value::Tuple(Rc::new(vec![Value::Numb(0.0), Value::Numb(0.0)]))
        );
    }

    #[test]
    fn test_acos() {
        // Numb
        assert_eq!(run("math.acos 1"), Value::Numb(0.0));

        // Undefined (out of domain / type error)
        assert!(matches!(run("math.acos 2"), Value::Undefined(_)));
        assert!(matches!(run(r#"math.acos "1""#), Value::Undefined(_)));

        // Nothing
        assert!(matches!(run("math.acos()"), Value::Nothing));

        // Tuple
        assert_eq!(
            run("math.acos(1, 1)"),
            Value::Tuple(Rc::new(vec![Value::Numb(0.0), Value::Numb(0.0)]))
        );
    }

    #[test]
    fn test_atan() {
        // Numb
        assert_eq!(run("math.atan 0"), Value::Numb(0.0));

        // Undefined (type error)
        assert!(matches!(run(r#"math.atan "0""#), Value::Undefined(_)));

        // Nothing
        assert!(matches!(run("math.atan()"), Value::Nothing));

        // Tuple
        assert_eq!(
            run("math.atan(0, 0)"),
            Value::Tuple(Rc::new(vec![Value::Numb(0.0), Value::Numb(0.0)]))
        );
    }

    #[test]
    fn test_sinh() {
        // Numb
        assert_eq!(run("math.sinh 0"), Value::Numb(0.0));

        // Undefined (type error)
        assert!(matches!(run(r#"math.sinh "0""#), Value::Undefined(_)));

        // Nothing
        assert!(matches!(run("math.sinh()"), Value::Nothing));

        // Tuple
        assert_eq!(
            run("math.sinh(0, 0)"),
            Value::Tuple(Rc::new(vec![Value::Numb(0.0), Value::Numb(0.0)]))
        );
    }

    #[test]
    fn test_cosh() {
        // Numb
        assert_eq!(run("math.cosh 0"), Value::Numb(1.0));

        // Undefined (type error)
        assert!(matches!(run(r#"math.cosh "0""#), Value::Undefined(_)));

        // Nothing
        assert!(matches!(run("math.cosh()"), Value::Nothing));

        // Tuple
        assert_eq!(
            run("math.cosh(0, 0)"),
            Value::Tuple(Rc::new(vec![Value::Numb(1.0), Value::Numb(1.0)]))
        );
    }

    #[test]
    fn test_tanh() {
        // Numb
        assert_eq!(run("math.tanh 0"), Value::Numb(0.0));

        // Undefined (type error)
        assert!(matches!(run(r#"math.tanh "0""#), Value::Undefined(_)));

        // Nothing
        assert!(matches!(run("math.tanh()"), Value::Nothing));

        // Tuple
        assert_eq!(
            run("math.tanh(0, 0)"),
            Value::Tuple(Rc::new(vec![Value::Numb(0.0), Value::Numb(0.0)]))
        );
    }

    #[test]
    fn test_asinh() {
        // Numb
        assert_eq!(run("math.asinh 0"), Value::Numb(0.0));

        // Undefined (type error)
        assert!(matches!(run(r#"math.asinh "0""#), Value::Undefined(_)));

        // Nothing
        assert!(matches!(run("math.asinh()"), Value::Nothing));

        // Tuple
        assert_eq!(
            run("math.asinh(0, 0)"),
            Value::Tuple(Rc::new(vec![Value::Numb(0.0), Value::Numb(0.0)]))
        );
    }

    #[test]
    fn test_acosh() {
        // Numb
        assert_eq!(run("math.acosh 1"), Value::Numb(0.0));

        // Undefined (out of domain / type error)
        assert!(matches!(run("math.acosh 0.5"), Value::Undefined(_)));
        assert!(matches!(run(r#"math.acosh "1""#), Value::Undefined(_)));

        // Nothing
        assert!(matches!(run("math.acosh()"), Value::Nothing));

        // Tuple
        assert_eq!(
            run("math.acosh(1, 1)"),
            Value::Tuple(Rc::new(vec![Value::Numb(0.0), Value::Numb(0.0)]))
        );
    }

    #[test]
    fn test_atanh() {
        // Numb
        assert_eq!(run("math.atanh 0"), Value::Numb(0.0));

        // Undefined (out of domain / type error)
        assert!(matches!(run("math.atanh 2"), Value::Undefined(_)));
        assert!(matches!(run(r#"math.atanh "0""#), Value::Undefined(_)));

        // Nothing
        assert!(matches!(run("math.atanh()"), Value::Nothing));

        // Tuple
        assert_eq!(
            run("math.atanh(0, 0)"),
            Value::Tuple(Rc::new(vec![Value::Numb(0.0), Value::Numb(0.0)]))
        );
    }

    #[test]
    fn test_exp() {
        // Numb
        assert_eq!(run("math.exp 0"), Value::Numb(1.0));

        // Undefined (type error)
        assert!(matches!(run(r#"math.exp "0""#), Value::Undefined(_)));

        // Nothing
        assert!(matches!(run("math.exp()"), Value::Nothing));

        // Tuple
        assert_eq!(
            run("math.exp(0, 0)"),
            Value::Tuple(Rc::new(vec![Value::Numb(1.0), Value::Numb(1.0)]))
        );
    }

    #[test]
    fn test_ln() {
        // Numb
        assert_eq!(run("math.ln(math.E)"), Value::Numb(1.0));

        // Undefined (out of domain / type error)
        assert!(matches!(run("math.ln(-1)"), Value::Undefined(_)));
        assert!(matches!(run(r#"math.ln "e""#), Value::Undefined(_)));

        // Nothing
        assert!(matches!(run("math.ln()"), Value::Nothing));

        // Tuple
        assert_eq!(
            run("math.ln(math.E, math.E)"),
            Value::Tuple(Rc::new(vec![Value::Numb(1.0), Value::Numb(1.0)]))
        );
    }

    #[test]
    fn test_log2() {
        // Numb
        assert_eq!(run("math.log2 8"), Value::Numb(3.0));

        // Undefined (out of domain / type error)
        assert!(matches!(run("math.log2(-1)"), Value::Undefined(_)));
        assert!(matches!(run(r#"math.log2 "8""#), Value::Undefined(_)));

        // Nothing
        assert!(matches!(run("math.log2()"), Value::Nothing));

        // Tuple
        assert_eq!(
            run("math.log2(8, 16)"),
            Value::Tuple(Rc::new(vec![Value::Numb(3.0), Value::Numb(4.0)]))
        );
    }

    #[test]
    fn test_log10() {
        // Numb
        assert_eq!(run("math.log10 100"), Value::Numb(2.0));

        // Undefined (out of domain / type error)
        assert!(matches!(run("math.log10(-1)"), Value::Undefined(_)));
        assert!(matches!(run(r#"math.log10 "100""#), Value::Undefined(_)));

        // Nothing
        assert!(matches!(run("math.log10()"), Value::Nothing));

        // Tuple
        assert_eq!(
            run("math.log10(100, 1000)"),
            Value::Tuple(Rc::new(vec![Value::Numb(2.0), Value::Numb(3.0)]))
        );
    }


    // ---------------------------------------------------------------------
    //  Curried higher-order functions (builtin_f64_curried)
    // ---------------------------------------------------------------------

    #[test]
    fn test_pow() {

        // Numbers
        assert_eq!(run("pow10 = math.pow 10, pow10 2"), Value::Numb(1024.0));
        assert_eq!(run("sq = math.pow 2, sq 5"),        Value::Numb(25.0));
        assert_eq!(run("math.pow 3 2"),                 Value::Numb(8.0));

        // Undefined
        assert_eq!(run("math.pow TRUE 3")   , undef_unary!("math.pow", Value::Numb(3.0)));
        assert_eq!(run("math.pow \"abc\" 3"), undef_unary!("math.pow", Value::Numb(3.0)));

        // Nothing
        assert_eq!(run("math.pow()"), Value::Nothing);

        // Tuples
        assert_eq!(run("math.pow(3, 4) 2"), Value::Tuple(Rc::new(vec![
            Value::Numb(8.0),
            Value::Numb(16.0),
        ])));
        assert_eq!(run("math.pow(3, 4) (2, 5)"), Value::Tuple(Rc::new(vec![
            Value::Numb(  8.0), 
            Value::Numb(125.0),
            Value::Numb( 16.0), 
            Value::Numb(625.0),
        ])));
    }

    #[test]
    fn test_log() {

        // Numbers
        assert_eq!(run("log10 = math.log 10, log10 100"), Value::Numb(2.0));
        assert_eq!(run("math.log 2 8"),                   Value::Numb(3.0));

        // Undefined
        assert_eq!(run("math.log TRUE 3")   , undef_unary!("math.log", Value::Numb(3.0)));
        assert_eq!(run("math.log \"abc\" 3"), undef_unary!("math.log", Value::Numb(3.0)));

        // Nothing
        assert_eq!(run("math.log()"), Value::Nothing);

        // Tuples
        assert_eq!(run("math.log(10, 100) 10000"), Value::Tuple(Rc::new(vec![
            Value::Numb(4.0),
            Value::Numb(2.0),
        ])));
        assert_eq!(run("math.log(10, 100) (100, 10000)"), Value::Tuple(Rc::new(vec![
            Value::Numb(2.0), 
            Value::Numb(4.0),
            Value::Numb(1.0), 
            Value::Numb(2.0),
        ])));
    }

    #[test]
    fn test_atan2() {
        // Numb
        assert_eq!(
            run("atan_x = math.atan2 0, atan_x 1"),
            Value::Numb(std::f64::consts::FRAC_PI_2)
        );
        assert_eq!(run("math.atan2 1 0"), Value::Numb(0.0));

        // Undefined (type errors)
        assert!(matches!(run(r#"math.atan2 "0" 1"#), Value::Undefined(_)));
        assert!(matches!(run(r#"math.atan2 0 "1""#), Value::Undefined(_)));

        // Nothing
        assert!(matches!(run("math.atan2 0 ()"), Value::Nothing));
        assert!(matches!(run("math.atan2 () 1"), Value::Nothing));

        // Tuple
        assert_eq!(
            run("math.atan2 1 (0, 0)"),
            Value::Tuple(Rc::new(vec![Value::Numb(0.0), Value::Numb(0.0)]))
        );
    }

    #[test]
    fn test_hypot() {
        // Numb
        assert_eq!(run("h3 = math.hypot 3, h3 4"), Value::Numb(5.0));
        assert_eq!(run("math.hypot 3 4"), Value::Numb(5.0));

        // Undefined (type errors)
        assert!(matches!(run(r#"math.hypot "3" 4"#), Value::Undefined(_)));
        assert!(matches!(run(r#"math.hypot 3 "4""#), Value::Undefined(_)));

        // Nothing
        assert!(matches!(run("math.hypot 3 ()"), Value::Nothing));
        assert!(matches!(run("math.hypot () 4"), Value::Nothing));

        // Tuple
        assert_eq!(
            run("math.hypot 3 (4, 4)"),
            Value::Tuple(Rc::new(vec![Value::Numb(5.0), Value::Numb(5.0)]))
        );
    }


    // ---------------------------------------------------------------------
    //  Multi-argument functions
    // ---------------------------------------------------------------------

    #[test]
    fn test_max() {
        // Numb / Tuple
        assert_eq!(run("math.max(1, 22, 3, 4)"), Value::Numb(22.0));
        assert_eq!(run("math.max 42"), Value::Numb(42.0));
        assert_eq!(run("math.max(-10, -5, -20)"), Value::Numb(-5.0));

        // Undefined (type error / empty)
        assert!(matches!(run(r#"math.max(1, "two", 3)"#), Value::Undefined(_)));

        // Nothing (0 items -> Undefined)
        assert!(matches!(run("math.max()"), Value::Undefined(_)));
    }

    #[test]
    fn test_min() {
        // Numb / Tuple
        assert_eq!(run("math.min(1, 22, 3, 4)"), Value::Numb(1.0));
        assert_eq!(run("math.min 42"), Value::Numb(42.0));
        assert_eq!(run("math.min(-10, -5, -20)"), Value::Numb(-20.0));

        // Undefined (type error / empty)
        assert!(matches!(run(r#"math.min(1, [2], 3)"#), Value::Undefined(_)));

        // Nothing (0 items -> Undefined)
        assert!(matches!(run("math.min()"), Value::Undefined(_)));
    }


    // ---------------------------------------------------------------------
    //  Constants
    // ---------------------------------------------------------------------

    #[test]
    fn test_constants() {
        assert_eq!(run("math.PI"), Value::Numb(std::f64::consts::PI));
        assert_eq!(run("math.E"), Value::Numb(std::f64::consts::E));
        assert_eq!(run("math.INF"), Value::Numb(f64::INFINITY));
        assert_eq!(run("math.INFINITY"), Value::Numb(f64::INFINITY));
    }
}
````

## File: src/builtins/nspace.rs
````rust
//! The built-in `nspace` namespace: functions that inspect namespace values.
//!
//! | Function        | Result                                                          |
//! |-----------------|-----------------------------------------------------------------|
//! | `nspace.enum`   | The sorted names bound *locally* in a namespace, as text        |
//! | `nspace.size`   | The number of bindings defined *locally* in a namespace        |
//! | `nspace.parent` | The enclosing namespace, or `Nothing` if there is none         |
//!
//! All functions are wrapped with `map_1`, so they map over tuples, return
//! `Nothing` for `Nothing`, and return `Undefined` for non-namespace arguments.

use crate::value::{Scope, Undef, Value};
use super::helpers::*;



// ----------------------------------------------------------------------------
//  `nspace` built-in namespace
// ----------------------------------------------------------------------------

/// Builds the `nspace` namespace value.
///
/// The namespace is a child of `parent`, so builtins defined in `parent`
/// remain visible from inside it.
pub fn create_namespace(parent: &Scope) -> Value {
    let nspace_scope = parent.new_child();
    nspace_scope.set("enum"  , map_1("nspace.enum"  , nspace_enum));
    nspace_scope.set("size"  , map_1("nspace.size"  , nspace_size));
    nspace_scope.set("parent", map_1("nspace.parent", nspace_parent));
    Value::Namespace(nspace_scope)
}

/// Returns the names bound locally in a namespace (inherited names are excluded),
/// as text values sorted alphabetically (byte-wise, so uppercase sorts before lowercase).
///
/// - empty namespace -> `Nothing`
/// - one binding     -> a single `Text`
/// - several         -> a `Tuple` of `Text`
/// - non-namespace   -> `Undefined`
fn nspace_enum(val: Value) -> Value {
    match val {
        Value::Namespace(scope) => {
            let mut keys: Vec<String> = scope
                .0
                .borrow()
                .bindings
                .keys()
                .cloned()
                .collect();
            keys.sort();
            normalize_tuple(
                keys.into_iter().map(|k| Value::Text(k.into())).collect()
            )
        }
        other => undef_unary!("nspace.enum", other),
    }
}

/// Returns the number of bindings defined locally in a namespace
/// (bindings inherited from parent scopes are not counted).
/// Non-namespace values yield `Undefined`.
fn nspace_size(val: Value) -> Value {
    match val {
        Value::Namespace(scope) => {
            let count = scope.0.borrow().bindings.len();
            Value::Numb(count as f64)
        }
        other => undef_unary!("nspace.size", other),
    }
}

/// Returns the enclosing namespace of `val`, or `Nothing` if it has no parent
/// (i.e. it is the outermost scope). Non-namespace values yield `Undefined`.
fn nspace_parent(val: Value) -> Value {
    match val {
        Value::Namespace(scope) => {
            // Clone the parent handle so the RefCell borrow is released immediately.
            let parent_scope = scope.0.borrow().parent.clone();
            match parent_scope {
                Some(p) => Value::Namespace(p),
                None => Value::Nothing,
            }
        }
        other => undef_unary!("nspace.parent", other),
    }
}



// ----------------------------------------------------------------------------
//  UNIT TESTS
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;
    use crate::eval::eval;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn root_scope() -> Scope {
        let scope = Scope::new();
        scope.set("nspace", create_namespace(&scope));
        scope.set("TRUE"  , Value::Bool(true));
        scope.set("FALSE" , Value::Bool(false));
        scope.new_child()
    }

    fn run(src: &str) -> Value {
        let scope = root_scope();
        let mut parser = Parser::new(Lexer::new(src));
        let ast = parser.parse();
        eval(&ast, &scope)
    }

    fn is_undefined(v: &Value) -> bool {
        matches!(v, Value::Undefined(_))
    }

    fn text(s: &str) -> Value {
        Value::Text(s.into())
    }

    #[test]
    fn test_size() {
        assert_eq!(run("nspace.size {a: 1, b: 2}"), Value::Numb(2.0));
        assert_eq!(run("nspace.size()"), Value::Nothing);
        assert_eq!(
            run("nspace.size({a:1, b:2}, {c:3, d:4, e:5})"), 
            Value::Tuple(Rc::new(vec![Value::Numb(2.0), Value::Numb(3.0)]))
        );
        assert_eq!(run("nspace.size {}"), Value::Numb(0.0));
        assert!(is_undefined(&run("nspace.size 42")));
        // The builtin namespace holds exactly: enum, parent, size
        assert_eq!(run("nspace.size nspace"), Value::Numb(3.0));
        // Mixed tuple: the non-namespace element becomes Undefined, the rest still map
        match run("nspace.size(1, {a:1})") {
            Value::Tuple(items) => {
                assert!(is_undefined(&items[0]));
                assert_eq!(items[1], Value::Numb(1.0));
            }
            other => panic!("expected tuple, got {:?}", other),
        }
    }

    #[test]
    fn test_enum() {
        assert_eq!(
            run("nspace.enum {a: 1, b: 2}"),
            Value::Tuple(Rc::new(vec![Value::Text("a".into()), Value::Text("b".into())]))
        );
        assert_eq!(run("nspace.enum()"), Value::Nothing);
        assert_eq!(
            run("nspace.enum({a:1, b:2}, {c:3})"),
            Value::Tuple(Rc::new(vec![
                    Value::Text("a".into()), 
                    Value::Text("b".into()),
                    Value::Text("c".into()),
            ]))
        );
        // empty -> Nothing; single key -> plain Text, not a 1-tuple
        assert_eq!(run("nspace.enum {}"), Value::Nothing);
        assert_eq!(run("nspace.enum {a: 1}"), text("a"));
        // keys are sorted regardless of definition order
        assert_eq!(
            run("nspace.enum {b: 1, a: 2}"),
            Value::Tuple(Rc::new(vec![text("a"), text("b")]))
        );
        // non-namespace -> Undefined
        assert!(is_undefined(&run("nspace.enum 42")));
        // only local names are listed, not inherited ones
        assert_eq!(run("nspace.enum {}"), Value::Nothing);
        // the builtin namespace lists its own functions, sorted
        assert_eq!(
            run("nspace.enum nspace"),
            Value::Tuple(Rc::new(vec![text("enum"), text("parent"), text("size")]))
        );
    }

    #[test]
    fn test_parent() {

        // 1. Parent of a child namespace literal { ... } created inside another scope
        assert_eq!(
            run("ns = {a = 1}, child = ns.{b = 2}, nspace.parent child == ns"),
            Value::Bool(true)
        );

        // 2. Parent on non-namespace yields Undefined
        assert!(matches!(run("nspace.parent 42"), Value::Undefined(_)));

        // 3. The parent of the root scope should be null
        assert_eq!(
            run("nspace.parent(nspace.parent(nspace.parent{}))"),
            Value::Nothing
        );
        // nspace.parent nspace is the scope holding nspace/TRUE/FALSE
        // (uppercase names sort before lowercase)
        assert_eq!(
            run("nspace.enum(nspace.parent nspace)"),
            Value::Tuple(Rc::new(vec![text("FALSE"), text("TRUE"), text("nspace")]))
        );
        // Nothing in -> Nothing out
        assert_eq!(run("nspace.parent()"), Value::Nothing);

        // Tuples
        assert_eq!(
            run("(a,b) = nspace.parent(14, nspace.{}), a, b == nspace"), 
            Value::Tuple(Rc::new(vec![
                    undef_unary!("nspace.parent", Value::Numb(14.0)),
                    Value::Bool(true)
            ]))
        );
    }

    #[test]
    fn test_direct_size_and_enum() {
        let scope = Scope::new();
        assert_eq!(nspace_size(Value::Namespace(scope.clone())), Value::Numb(0.0));
        assert_eq!(nspace_enum(Value::Namespace(scope.clone())), Value::Nothing);

        scope.set("y", Value::Numb(1.0));
        scope.set("x", Value::Numb(2.0));
        assert_eq!(nspace_size(Value::Namespace(scope.clone())), Value::Numb(2.0));
        assert_eq!(
            nspace_enum(Value::Namespace(scope)),
            Value::Tuple(Rc::new(vec![text("x"), text("y")]))
        );
    }

    #[test]
    fn test_direct_locality() {
        // Inherited bindings must not be counted or listed
        let parent = Scope::new();
        parent.set("p", Value::Numb(1.0));
        let child = parent.new_child();
        child.set("c", Value::Numb(2.0));
        assert_eq!(nspace_size(Value::Namespace(child.clone())), Value::Numb(1.0));
        assert_eq!(nspace_enum(Value::Namespace(child)), text("c"));
    }

    #[test]
    fn test_direct_parent() {
        let parent = Scope::new();
        let child = parent.new_child();
        assert_eq!(
            nspace_parent(Value::Namespace(child)),
            Value::Namespace(parent.clone())
        );
        // A top-level scope has no parent
        assert_eq!(nspace_parent(Value::Namespace(parent)), Value::Nothing);
    }

    #[test]
    fn test_non_namespace_arguments_are_undefined() {
        for v in [Value::Numb(1.0), Value::Bool(true), text("s")] {
            assert!(is_undefined(&nspace_size(v.clone())));
            assert!(is_undefined(&nspace_enum(v.clone())));
            assert!(is_undefined(&nspace_parent(v)));
        }
    }
}
````

## File: src/builtins/numb.rs
````rust
//! The `numb` built-in namespace.
//!
//! Currently contains a single function:
//!
//! - `numb.from(x)` converts a `Bool` or `Text` (or a `Numb`, unchanged) to a `Numb`.
//!
//! ```text
//! numb.from(TRUE)       -> 1
//! numb.from("42")       -> 42
//! numb.from(TRUE, "42") -> (1, 42)
//! numb.from("abc")      -> undefined
//! ```

// `Undef` is kept in scope because `undef_unary!` may expand to code that refers to it.
use crate::value::{Scope, Undef, Value};
use super::helpers::*;



// ----------------------------------------------------------------------------
//  Namespace construction
// ----------------------------------------------------------------------------

/// Builds the `numb` namespace value.
///
/// The namespace scope is a child of `parent`, so root-scope names stay
/// visible inside it.
pub fn create_namespace(parent: &Scope) -> Value {
    let numb_scope = parent.new_child();
    // Unary function: `map_1` applies it to each item of the argument.
    numb_scope.set("from", map_1("numb.from", numb_from));
    Value::Namespace(numb_scope)
}



// ----------------------------------------------------------------------------
//  numb.from
// ----------------------------------------------------------------------------

/// Converts a single (non-tuple) value to a number.
///
/// - `Numb`  -> returned unchanged.
/// - `Bool`  -> `1.0` for `TRUE`, `0.0` for `FALSE`.
/// - `Text`  -> parsed with Rust's `f64::from_str`. This accepts forms such
///              as `"42"`, `"-3.5"` and `"1e3"`. It does not accept surrounding
///              whitespace, and it also accepts `"inf"` and `"NaN"`.
/// - anything else, or text that fails to parse -> `Undefined`.
///
/// `Nothing` and tuples are handled by `map_1` before this function is
/// called, so `val` is always a single item.
fn numb_from(val: Value) -> Value {
    match &val {
        Value::Numb(n) => Value::Numb(*n),
        Value::Bool(b) => Value::Numb(if *b { 1.0 } else { 0.0 }),
        Value::Text(s) => match s.parse::<f64>() {
            Ok(n)  => Value::Numb(n),
            Err(_) => undef_unary!("numb.from", val), // not a valid number literal
        },
        _ => undef_unary!("numb.from", val),          // unsupported type
    }
}



// ----------------------------------------------------------------------------
//  Unit tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;
    use crate::eval::eval;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    /// Minimal root scope: the `numb` namespace plus the boolean constants.
    /// Returns a child scope, mirroring how user code is evaluated.
    fn root_scope() -> Scope {
        let scope = Scope::new();
        scope.set("numb" , create_namespace(&scope));
        scope.set("TRUE" , Value::Bool(true));
        scope.set("FALSE", Value::Bool(false));
        scope.new_child()
    }

    /// Lexes, parses and evaluates `src` in a fresh test scope.
    fn run(src: &str) -> Value {
        let scope = root_scope();
        let mut parser = Parser::new(Lexer::new(src));
        let ast = parser.parse();
        eval(&ast, &scope)
    }

    #[test]
    fn test_from() {

        // Bool
        assert_eq!(run("numb.from(TRUE)") , Value::Numb(1.0));
        assert_eq!(run("numb.from(FALSE)"), Value::Numb(0.0));

        // Numb
        assert_eq!(run("numb.from(1.5)"), Value::Numb(1.5));

        // Text
        assert_eq!(run(r#"numb.from "42""#)   , Value::Numb(42.0));
        assert_eq!(run(r#"numb.from "-3.5""#) , Value::Numb(-3.5));
        assert_eq!(run(r#"numb.from "1e3""#)  , Value::Numb(1000.0));
        assert!(matches!(run(r#"numb.from " 42""#), Value::Undefined(_)));

        // Invalid Text
        assert!(matches!(run(r#"numb.from "abc""#), Value::Undefined(_)));
        assert!(matches!(run(r#"numb.from """#)   , Value::Undefined(_)));

        // Nothing
        assert_eq!(run("numb.from()"), Value::Nothing);

        // Tuple
        assert_eq!(
            run(r#"numb.from(TRUE, "42")"#),
            Value::Tuple(Rc::new(vec![
                Value::Numb(1.0),
                Value::Numb(42.0),
            ]))
        );

        // Others
        assert!(matches!(run("numb.from []"), Value::Undefined(_)));
    }
}
````

## File: src/builtins/text.rs
````rust
//! The `text` built-in namespace.
//!
//! Every function is applied element-wise over tuples (via `map_1` / `map_curried`),
//! so `Nothing` maps to `Nothing` and tuples map to tuples. Arguments of the wrong
//! type produce `Value::Undefined`. All positions and lengths count Unicode
//! characters, not bytes.

use crate::value::{BuiltinFn, Scope, Undef, Value};
use super::helpers::*;


// ----------------------------------------------------------------------------
//  `text` built-in namespace 
// ----------------------------------------------------------------------------

/// Builds the `text` namespace and returns it as a `Value::Namespace`.
///
/// The namespace scope is a child of `parent`, so root-scope names stay visible inside it.
pub fn create_namespace(parent: &Scope) -> Value {
    let text_scope = parent.new_child();

    // Unary functions: map over each item of the argument.
    text_scope.set("from"     , map_1("text.from"     , text_from));
    text_scope.set("size"     , map_1("text.size"     , text_size));
    text_scope.set("enum"     , map_1("text.enum"     , text_enum));
    text_scope.set("lower"    , map_1("text.lower"    , text_lower));
    text_scope.set("upper"    , map_1("text.upper"    , text_upper));
    text_scope.set("trim"     , map_1("text.trim"     , text_trim));
    text_scope.set("trim_head", map_1("text.trim_head", text_trim_head));
    text_scope.set("trim_tail", map_1("text.trim_tail", text_trim_tail));

    // Curried binary functions: (needle/count/separator) first, then the target text.
    text_scope.set("find"     , map_curried("text.find" , text_find));
    text_scope.set("rfind"    , map_curried("text.rfind", text_rfind));
    text_scope.set("head"     , map_curried("text.head" , text_head));
    text_scope.set("tail"     , map_curried("text.tail" , text_tail));
    text_scope.set("split"    , map_curried("text.split", text_split));
    // `join` uses map_1 (not map_curried) on purpose: the separator is mapped, but the
    // returned function receives the whole tuple of items to join.
    text_scope.set("join"     , map_1("text.join"     , text_join));

    Value::Namespace(text_scope)
}

// ----------------------------------------------------------------------------
//  Conversion / inspection
// ----------------------------------------------------------------------------

/// Converts a scalar to its text form (`TRUE`/`FALSE`, number, `[[List of N]]`, `[[Func]]`, `[[Undefined]]`, `[[Namespace of N]]`). Text passes through unchanged; anything else is Undefined.
fn text_from(val: Value) -> Value {
    let s: String = match &val {
        Value::Bool(b) => {
            if *b { "TRUE".into() } else { "FALSE".into() }
        }
        Value::Numb(n) => {
            n.to_string()
        }
        Value::Text(s) => {
            return Value::Text(s.clone())
        }
        Value::List(l) => {
            format!("[[List of {}]]", l.len())
        }
        Value::Func{..} | Value::Builtin(_) => {
            "[[Func]]".to_string()
        }
        Value::Undefined(_) => {
            "[[Undefined]]".to_string()
        }
        Value::Namespace(scope) => {
            format!("[[Namespace of {}]]", scope.0.borrow().bindings.len())
        }
        // Bool/Numb/Text/etc. handled above; Nothing and Tuple never reach here (handled by map_1).
        _ => return undef_unary!("text.from", val),
    };

    Value::Text(s.into())
}

/// Number of characters in the text.
fn text_size(val: Value) -> Value {
    match val {
        Value::Text(s) => Value::Numb(s.chars().count() as f64),
        other => undef_unary!("text.size", other),
    }
}

/// Splits the text into a tuple of single-character texts.
fn text_enum(val: Value) -> Value {
    match val {
        Value::Text(s) => {
            let chars: Vec<Value> = s.chars().map(|c| Value::Text(c.to_string().into())).collect();
            normalize_tuple(chars)
        }
        other => undef_unary!("text.enum", other),
    }
}

// ----------------------------------------------------------------------------
//  Case and whitespace
// ----------------------------------------------------------------------------

/// Lower-cases the text.
fn text_lower(val: Value) -> Value {
    match val {
        Value::Text(s) => Value::Text(s.to_lowercase().into()),
        other => undef_unary!("text.lower", other),
    }
}

/// Upper-cases the text.
fn text_upper(val: Value) -> Value {
    match val {
        Value::Text(s) => Value::Text(s.to_uppercase().into()),
        other => undef_unary!("text.upper", other),
    }
}

/// Removes whitespace from both ends of the text.
fn text_trim(val: Value) -> Value {
    match val {
        Value::Text(s) => Value::Text(s.trim().into()),
        other => undef_unary!("text.trim", other),
    }
}

/// Removes whitespace from the start of the text.
fn text_trim_head(val: Value) -> Value {
    match val {
        Value::Text(s) => Value::Text(s.trim_start().into()),
        other => undef_unary!("text.trim_head", other),
    }
}

/// Removes whitespace from the end of the text.
fn text_trim_tail(val: Value) -> Value {
    match val {
        Value::Text(s) => Value::Text(s.trim_end().into()),
        other => undef_unary!("text.trim_tail", other),
    }
}

// ----------------------------------------------------------------------------
//  Searching
// ----------------------------------------------------------------------------

/// Converts an optional byte offset within `haystack` into a character index
/// (`Value::Numb`), or `-1` when there was no match.
fn char_position(haystack: &str, byte_pos: Option<usize>) -> Value {
    match byte_pos {
        Some(b) => Value::Numb(haystack[..b].chars().count() as f64),
        None => Value::Numb(-1.0),
    }
}

/// Character index of the first occurrence of `needle` in `haystack`, or -1 if absent. Only `haystack` is reported in the Undefined result if either argument is not Text.
fn text_find(needle: Value, haystack: Value) -> Value {
    match (&needle, haystack) {
        (Value::Text(n), Value::Text(h)) => char_position(&h, h.find(n.as_ref())),
        (_, other) => undef_unary!("text.find", other),
    }
}

/// Character index of the last occurrence of `needle` in `haystack`, or -1 if absent. Only `haystack` is reported in the Undefined result if either argument is not Text.
fn text_rfind(needle: Value, haystack: Value) -> Value {
    match (&needle, haystack) {
        (Value::Text(n), Value::Text(h)) => char_position(&h, h.rfind(n.as_ref())),
        (_, other) => undef_unary!("text.rfind", other),
    }
}

// ----------------------------------------------------------------------------
//  Slicing
// ----------------------------------------------------------------------------

/// Resolves a signed character count `n` against a text of `len` characters
/// into a boundary index in `0..=len`.
/// - `n >= 0`: `n`, clamped to `len` (counted from the start)
/// - `n < 0`:  `len + n`, clamped to `0` (counted back from the end)
fn boundary_index(n: f64, len: usize) -> usize {
    let (len, n) = (len as i64, n as i64);
    if n >= 0 { n.min(len) as usize } else { (len + n).max(0) as usize }
}

/// First `n` characters; a negative `n` drops the last `|n|` characters instead.
fn text_head(n: Value, s: Value) -> Value {
    match (&n, s) {
        (Value::Numb(n_val), Value::Text(s_val)) => {
            let chars: Vec<char> = s_val.chars().collect();
            let end = boundary_index(*n_val, chars.len());
            let result: String = chars[..end].iter().collect();
            Value::Text(result.into())
        }
        (_, other) => undef_unary!("text.head", other),
    }
}

/// Text from character index `n` onward; a negative `n` keeps only the last `|n|` characters.
fn text_tail(n: Value, s: Value) -> Value {
    match (&n, s) {
        (Value::Numb(n_val), Value::Text(s_val)) => {
            let chars: Vec<char> = s_val.chars().collect();
            let start = boundary_index(*n_val, chars.len());
            let result: String = chars[start..].iter().collect();
            Value::Text(result.into())
        }
        (_, other) => undef_unary!("text.tail", other),
    }
}

// ----------------------------------------------------------------------------
//  Split / join
// ----------------------------------------------------------------------------

/// Splits `target` on every occurrence of `sep`, returning a tuple of parts (a single Text if `sep` does not occur).
fn text_split(sep: Value, target: Value) -> Value {
    match (&sep, target) {
        (Value::Text(s1), Value::Text(s2)) => {
            let parts: Vec<Value> = s2
                .split(s1.as_ref())
                .map(|p| Value::Text(p.into()))
                .collect();
            normalize_tuple(parts)
        }
        (_, other) => undef_unary!("text.split", other),
    }
}

/// Curried: `text.join sep items` joins all Text items with `sep`. Returns Undefined if `sep` is not Text or if any item is not Text.
fn text_join(sep: Value) -> Value {
    Value::Builtin(BuiltinFn::new("text.join", move |items| {
        match &sep {
            Value::Text(s1) => {
                // Validate and collect first, so one non-Text item aborts the whole join.
                let mut strings: Vec<String> = Vec::new();
                for item in items.items() {
                    match item {
                        Value::Text(t) => strings.push(t.to_string()),
                        other => return undef_unary!("text.join", other), // `return` exits the whole builtin closure, not just the loop.
                    }
                }
                Value::Text(strings.join(s1.as_ref()).into())
            }
            _ => undef_unary!("text.join", items),
        }
    }))
}


// ----------------------------------------------------------------------------
//  UNIT TESTS
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;
    use crate::eval::eval;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn root_scope() -> Scope {
        let scope = Scope::new();
        scope.set("text", create_namespace(&scope));
        scope.set("TRUE", Value::Bool(true));
        scope.set("FALSE", Value::Bool(false));
        scope.new_child()
    }

    fn run(src: &str) -> Value {
        let scope = root_scope();
        let mut parser = Parser::new(Lexer::new(src));
        let ast = parser.parse();
        eval(&ast, &scope)
    }

    #[test]
    fn text_from() {

        // Number
        assert_eq!(run("text.from 42"), Value::Text("42".into()));

        // Bool
        assert_eq!(run("text.from(TRUE)"), Value::Text("TRUE".into()));
        assert_eq!(run("text.from(FALSE)"), Value::Text("FALSE".into()));

        // Text
        assert_eq!(run("text.from \"abc\""), Value::Text("abc".into()));

        // List
        assert_eq!(run("text.from[10,20,30,40]"), Value::Text("[[List of 4]]".into()));
        
        // Namespace
        assert_eq!(run("text.from{a:1, b:2, c:3}"), Value::Text("[[Namespace of 3]]".into()));

        // Func
        assert_eq!(run("text.from(x -> 2*x)"), Value::Text("[[Func]]".into()));
        assert_eq!(run("text.from(text.from)"), Value::Text("[[Func]]".into()));

        // Undefined
        assert_eq!(run("text.from([] - [])"), Value::Text("[[Undefined]]".into()));

        // Nothing
        assert_eq!(run("text.from()"), Value::Nothing);

        // Tuple
        assert_eq!(
            run("text.from(21, TRUE)"),
            Value::Tuple(Rc::new(vec![
                Value::Text("21".into()),
                Value::Text("TRUE".into()),
            ])),
        );
    }

    #[test]
    fn text_size() {

        // Text
        assert_eq!(run("text.size \"abc\""), Value::Numb(3.0));
        assert_eq!(run("text.size \"\"   "), Value::Numb(0.0));

        // Nothing
        assert_eq!(run("text.size()"), Value::Nothing);

        // Tuple
        assert_eq!(
            run("text.size(\"abc\", \"defgh\")"), 
            Value::Tuple(Rc::new(vec![
                Value::Numb(3.0),
                Value::Numb(5.0),
            ]))
        );

        // Others
        assert!(matches!(run("text.size 123"), Value::Undefined(_)));
    }

    #[test]
    fn text_enum() {

        // Text
        assert_eq!(
            run(r#"text.enum "abc""#),
            Value::Tuple(Rc::new(vec![
                Value::Text("a".into()),
                Value::Text("b".into()),
                Value::Text("c".into()),
            ]))
        );

        // Nothing
        assert_eq!(run("text.enum()"), Value::Nothing);

        // Tuple
        assert_eq!(
            run("text.enum(\"abc\", \"de\")"), 
            Value::Tuple(Rc::new(vec![
                Value::Text("a".into()),
                Value::Text("b".into()),
                Value::Text("c".into()),
                Value::Text("d".into()),
                Value::Text("e".into()),
            ]))
        );

        // Others
        assert!(matches!(run("text.enum 123"), Value::Undefined(_)));
    }

    #[test]
    fn text_lower() {

        // Text
        assert_eq!(run("text.lower \"ABC\""), Value::Text("abc".into()));
        assert_eq!(run("text.lower \"aBc\""), Value::Text("abc".into()));
        assert_eq!(run("text.lower \"abc\""), Value::Text("abc".into()));

        // Nothing
        assert_eq!(run("text.lower()"), Value::Nothing);

        // Tuple
        assert_eq!(
            run("text.lower(\"ABC\", \"DEF\")"), 
            Value::Tuple(Rc::new(vec![
                Value::Text("abc".into()),
                Value::Text("def".into()),
            ]))
        );

        // Others
        assert!(matches!(run("text.lower 123"), Value::Undefined(_)));
    }

    #[test]
    fn text_upper() {

        // Text
        assert_eq!(run("text.upper \"ABC\""), Value::Text("ABC".into()));
        assert_eq!(run("text.upper \"aBc\""), Value::Text("ABC".into()));
        assert_eq!(run("text.upper \"abc\""), Value::Text("ABC".into()));

        // Nothing
        assert_eq!(run("text.upper()"), Value::Nothing);

        // Tuple
        assert_eq!(
            run("text.upper(\"abc\", \"def\")"), 
            Value::Tuple(Rc::new(vec![
                Value::Text("ABC".into()),
                Value::Text("DEF".into()),
            ]))
        );

        // Others
        assert!(matches!(run("text.upper 123"), Value::Undefined(_)));
    }

    #[test]
    fn text_trim() {

        // Text
        assert_eq!(run("text.trim \"  abc  \""), Value::Text("abc".into()));
        assert_eq!(run("text.trim \"    abc\""), Value::Text("abc".into()));
        assert_eq!(run("text.trim \"abc    \""), Value::Text("abc".into()));
        assert_eq!(run("text.trim \"abc\""), Value::Text("abc".into()));

        // Nothing
        assert_eq!(run("text.trim()"), Value::Nothing);

        // Tuple
        assert_eq!(
            run("text.trim(\"  abc  \", \"  def  \")"), 
            Value::Tuple(Rc::new(vec![
                Value::Text("abc".into()),
                Value::Text("def".into()),
            ]))
        );

        // Others
        assert!(matches!(run("text.trim 123"), Value::Undefined(_)));
    }

    #[test]
    fn text_trim_head() {

        // Text
        assert_eq!(run("text.trim_head \"  abc  \""), Value::Text("abc  ".into()));
        assert_eq!(run("text.trim_head \"    abc\""), Value::Text("abc".into()));
        assert_eq!(run("text.trim_head \"abc    \""), Value::Text("abc    ".into()));
        assert_eq!(run("text.trim_head \"abc\""), Value::Text("abc".into()));

        // Nothing
        assert_eq!(run("text.trim_head()"), Value::Nothing);

        // Tuple
        assert_eq!(
            run("text.trim_head(\"  abc  \", \"  def  \")"), 
            Value::Tuple(Rc::new(vec![
                Value::Text("abc  ".into()),
                Value::Text("def  ".into()),
            ]))
        );

        // Others
        assert!(matches!(run("text.trim_head 123"), Value::Undefined(_)));
    }

    #[test]
    fn text_trim_tail() {

        // Text
        assert_eq!(run("text.trim_tail \"  abc  \""), Value::Text("  abc".into()));
        assert_eq!(run("text.trim_tail \"    abc\""), Value::Text("    abc".into()));
        assert_eq!(run("text.trim_tail \"abc    \""), Value::Text("abc".into()));
        assert_eq!(run("text.trim_tail \"abc\""), Value::Text("abc".into()));

        // Nothing
        assert_eq!(run("text.trim_tail()"), Value::Nothing);

        // Tuple
        assert_eq!(
            run("text.trim_tail(\"  abc  \", \"  def  \")"), 
            Value::Tuple(Rc::new(vec![
                Value::Text("  abc".into()),
                Value::Text("  def".into()),
            ]))
        );

        // Others
        assert!(matches!(run("text.trim_tail 123"), Value::Undefined(_)));
    }

    #[test]
    fn text_find() {

        // Text
        assert_eq!(run(r#"text.find "bc" "abcdefabcd""#), Value::Numb(1.0));
        assert_eq!(run(r#"text.find "xyz" "abcdefabcd""#), Value::Numb(-1.0));

        // Nothing
        assert_eq!(run("text.find ()"), Value::Nothing);
        assert_eq!(run("text.find () \"abc\""), Value::Nothing);
        assert_eq!(run("text.find () ()"), Value::Nothing);

        // Tuple - Text
        assert_eq!(
            run("text.find (\"b\", \"d\") \"abcdefabcdef\""), 
            Value::Tuple(Rc::new(vec![
                Value::Numb(1.0),
                Value::Numb(3.0),
            ]))
        );

        // Tuple - Tuple
        assert_eq!(
            run("text.find (\"b\", \"d\") (\"abcdefabcdef\", \"aaaaabcccd\")"), 
            Value::Tuple(Rc::new(vec![
                Value::Numb(1.0),
                Value::Numb(5.0),
                Value::Numb(3.0),
                Value::Numb(9.0),
            ]))
        );

        // Others
        assert!(matches!(run("text.find 123 \"abc\""), Value::Undefined(_)));
        assert!(matches!(run("text.find \"abc\" 123"), Value::Undefined(_)));
    }

    #[test]
    fn text_rfind() {

        // Text - Text
        assert_eq!(run(r#"text.rfind "bc" "abcdefabcd""#), Value::Numb(7.0));
        assert_eq!(run(r#"text.rfind "xyz" "abcdefabcd""#), Value::Numb(-1.0));

        // Nothing
        assert_eq!(run("text.rfind ()"), Value::Nothing);
        assert_eq!(run("text.rfind () \"abc\""), Value::Nothing);
        assert_eq!(run("text.rfind () ()"), Value::Nothing);

        // Tuple - Text
        assert_eq!(
            run("text.rfind (\"b\", \"d\") \"abcdefabcdef\""), 
            Value::Tuple(Rc::new(vec![
                Value::Numb(7.0),
                Value::Numb(9.0),
            ]))
        );

        // Tuple - Tuple
        assert_eq!(
            run("text.rfind (\"b\", \"d\") (\"abcdefabcdef\", \"aaaaabcccd\")"), 
            Value::Tuple(Rc::new(vec![
                Value::Numb(7.0),
                Value::Numb(5.0),
                Value::Numb(9.0),
                Value::Numb(9.0),
            ]))
        );

        // Others
        assert!(matches!(run("text.rfind 123 \"abc\""), Value::Undefined(_)));
        assert!(matches!(run("text.rfind \"abc\" 123"), Value::Undefined(_)));
    }

    #[test]
    fn text_head() {

        // Numb - Text
        assert_eq!(run("text.head 3 \"abcdef\""), Value::Text("abc".into()));
        assert_eq!(run("text.head 0 \"abcdef\""), Value::Text("".into()));
        assert_eq!(run("text.head(-2) \"abcdef\""), Value::Text("abcd".into()));
        assert_eq!(run("text.head 99 \"abc\""), Value::Text("abc".into()));
        assert_eq!(run("text.head(-99) \"abc\""), Value::Text("".into()));

        // Nothing
        assert_eq!(run("text.head ()"), Value::Nothing);
        assert_eq!(run("text.head () \"abc\""), Value::Nothing);
        assert_eq!(run("text.head () ()"), Value::Nothing);

        // Tuple - Text
        assert_eq!(
            run("text.head (2, 3) \"abcdef\""), 
            Value::Tuple(Rc::new(vec![
                Value::Text("ab".into()),
                Value::Text("abc".into()),
            ]))
        );

        // Tuple - Tuple
        assert_eq!(
            run("text.head (2, 3) (\"abcd\", \"efgh\")"), 
            Value::Tuple(Rc::new(vec![
                Value::Text("ab".into()),
                Value::Text("ef".into()),
                Value::Text("abc".into()),
                Value::Text("efg".into()),
            ]))
        );

        // Others
        assert!(matches!(run("text.head 123 456"), Value::Undefined(_)));
        assert!(matches!(run("text.head \"abc\" 123"), Value::Undefined(_)));
    }

    #[test]
    fn text_tail() {

        // Numb - Text
        assert_eq!(run("text.tail 3 \"abcdef\""), Value::Text("def".into()));
        assert_eq!(run("text.tail 0 \"abcdef\""), Value::Text("abcdef".into()));
        assert_eq!(run("text.tail(-2) \"abcdef\""), Value::Text("ef".into()));
        assert_eq!(run("text.tail 99 \"abc\""), Value::Text("".into()));
        assert_eq!(run("text.tail(-99) \"abc\""), Value::Text("abc".into()));

        // Nothing
        assert_eq!(run("text.tail ()"), Value::Nothing);
        assert_eq!(run("text.tail () \"abc\""), Value::Nothing);
        assert_eq!(run("text.tail () ()"), Value::Nothing);

        // Tuple - Text
        assert_eq!(
            run("text.tail (-2, -3) \"abcdef\""), 
            Value::Tuple(Rc::new(vec![
                Value::Text("ef".into()),
                Value::Text("def".into()),
            ]))
        );

        // Tuple - Tuple
        assert_eq!(
            run("text.tail (-2, -3) (\"abcd\", \"efgh\")"), 
            Value::Tuple(Rc::new(vec![
                Value::Text("cd".into()),
                Value::Text("gh".into()),
                Value::Text("bcd".into()),
                Value::Text("fgh".into()),
            ]))
        );

        // Others
        assert!(matches!(run("text.tail 123 456"), Value::Undefined(_)));
        assert!(matches!(run("text.tail \"abc\" 123"), Value::Undefined(_)));
    }

    #[test]
    fn text_split() {

        // Text - Text
        assert_eq!(
            run("text.split \":\" \"aa:bb:cc\""),
            Value::Tuple(Rc::new(vec![
                Value::Text("aa".into()),
                Value::Text("bb".into()),
                Value::Text("cc".into()),
            ]))
        );
        assert_eq!(
            run("text.split \":\" \"aabbcc\""),
            Value::Text("aabbcc".into()),
        );

        // Nothing
        assert_eq!(run("text.split ()"), Value::Nothing);
        assert_eq!(run("text.split () \"abc\""), Value::Nothing);
        assert_eq!(run("text.split () ()"), Value::Nothing);

        // Tuple - Text
        assert_eq!(
            run("text.split (\".\", \":\") \"aa.:bb.:cc\""), 
            Value::Tuple(Rc::new(vec![
                Value::Text("aa".into()),
                Value::Text(":bb".into()),
                Value::Text(":cc".into()),
                Value::Text("aa.".into()),
                Value::Text("bb.".into()),
                Value::Text("cc".into()),
            ]))
        );

        // Tuple - Tuple
        assert_eq!(
            run("text.split (\".\", \":\") (\"aa.:bb.:cc\", \"abc:def\")"), 
            Value::Tuple(Rc::new(vec![
                Value::Text("aa".into()),
                Value::Text(":bb".into()),
                Value::Text(":cc".into()),
                Value::Text("abc:def".into()),
                Value::Text("aa.".into()),
                Value::Text("bb.".into()),
                Value::Text("cc".into()),
                Value::Text("abc".into()),
                Value::Text("def".into()),
            ]))
        );

        // Others
        assert!(matches!(run("text.split 123 456"), Value::Undefined(_)));
        assert!(matches!(run("text.split \"abc\" 123"), Value::Undefined(_)));
    }

    #[test]
    fn text_join() {

        // Text - Tuple
        assert_eq!(run(r#"text.join ":" ("aa", "bb", "cc")"#), Value::Text("aa:bb:cc".into()));
        assert_eq!(run(r#"text.join ":" "abc""#), Value::Text("abc".into()));

        // Nothing
        assert_eq!(run("text.join ()"), Value::Nothing);
        assert_eq!(run("text.join () \"abc\""), Value::Nothing);
        assert_eq!(run("text.join () ()"), Value::Nothing);

        // Tuple - Tuple
        assert_eq!(
            run(r#"text.join (".", ":") ("aa", "bb", "cc")"#), 
            Value::Tuple(Rc::new(vec![
                Value::Text("aa.bb.cc".into()),
                Value::Text("aa:bb:cc".into()),
            ]))
        );

        // Others
        assert!(matches!(run("text.join 123 345"), Value::Undefined(_)));
        assert!(matches!(run("text.join \"abc\" 123"), Value::Undefined(_)));
        assert!(matches!(run(r#"text.join ":" ("aa", 1, "cc")"#), Value::Undefined(_)));
    }
}
````

## File: src/builtins/time.rs
````rust
//! The `time` built-in namespace.
//!
//! Timestamps are `Numb` values holding seconds since the Unix epoch
//! (1970-01-01T00:00:00Z), always interpreted as UTC. Every function except
//! `now` is mapped over tuples via `map_1`: `Nothing` gives `Nothing`, a tuple
//! is mapped element-wise, and an unsupported argument type gives `Undefined`.
//!
//! Namespace members:
//! - `now()`         : current time
//! - `to_date(t)`    : timestamp -> namespace `{year, month, day, hour, minute, second}`
//! - `from_date(d)`  : date namespace -> timestamp (missing fields default to 1970-01-01T00:00:00)
//! - `week_day(t)`   : 0 = Sunday ... 6 = Saturday
//! - `week_number(t)`: ISO-8601 week number
//! - `to_string(t)`  : ISO-8601 text, e.g. `2026-05-28T20:26:40.000Z`
//! - `from_string(s)`: parse ISO-8601 text (date, or date + time)

use crate::value::{BuiltinFn, Scope, Undef, Value};
use super::helpers::*;



// ----------------------------------------------------------------------------
//  `time` built-in namespace
// ----------------------------------------------------------------------------

/// Builds the `time` namespace as a child of `parent`.
pub fn create_namespace(parent: &Scope) -> Value {
    let time_scope = parent.new_child();

    time_scope.set("now", Value::Builtin(BuiltinFn::new("time.now", time_now)));

    time_scope.set("to_date", map_1("time.to_date", time_to_date));
    time_scope.set("from_date", map_1("time.from_date", time_from_date));

    time_scope.set("week_day", map_1("time.week_day", time_week_day));
    time_scope.set("week_number", map_1("time.week_number", time_week_number));

    time_scope.set("to_string", map_1("time.to_string", time_to_string));
    time_scope.set("from_string", map_1("time.from_string", time_from_string));

    Value::Namespace(time_scope)
}



// ----------------------------------------------------------------------------
//  Built-in functions
// ----------------------------------------------------------------------------

/// `time.now()`: current time as epoch seconds (fractional). Any argument is ignored.
/// Returns 0 if the system clock is before the epoch.
fn time_now(_arg: Value) -> Value {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => Value::Numb(d.as_secs_f64()),
        Err(_) => Value::Numb(0.0),
    }
}

/// Returns a fresh namespace with `year, month, day, hour, minute, second`,
/// with `second` possibly fractional.
fn time_to_date(t: Value) -> Value {
    match t {
        Value::Numb(t) => {
            let dt = datetime_from_epoch(t);
            let scope = Scope::new();
            scope.set("year", Value::Numb(dt.year as f64));
            scope.set("month", Value::Numb(dt.month as f64));
            scope.set("day", Value::Numb(dt.day as f64));
            scope.set("hour", Value::Numb(dt.hour as f64));
            scope.set("minute", Value::Numb(dt.minute as f64));
            scope.set("second", Value::Numb(dt.second));
            Value::Namespace(scope)
        }
        other => undef_unary!("time.to_date", other),
    }
}

/// Missing or non-numeric fields default to year 0 / month 1 / day 1 / 00:00:00.
fn time_from_date(date: Value) -> Value {
    match date {
        Value::Namespace(scope) => {
            let y = numb_field(&scope, "year", 0.0) as i64;
            let m = numb_field(&scope, "month", 1.0) as i64;
            let d = numb_field(&scope, "day", 1.0) as i64;
            let h = numb_field(&scope, "hour", 0.0) as i64;
            let min = numb_field(&scope, "minute", 0.0) as i64;
            let s = numb_field(&scope, "second", 0.0);
            Value::Numb(epoch_from_parts(y, m, d, h, min, s))
        }
        other => undef_unary!("time.from_date", other),
    }
}

/// Formats the given timestamp as `YYYY-MM-DDTHH:MM:SS.mmmZ` (UTC, millisecond precision).
fn time_to_string(val: Value) -> Value {
    match val {
        Value::Numb(t) => {
            let dt = datetime_from_epoch(t);
            Value::Text(format!(
                "{:04}-{:02}-{:02}T{:02}:{:02}:{:06.3}Z",
                dt.year, dt.month, dt.day, dt.hour, dt.minute, dt.second
            ).into())
        }
        other => undef_unary!("time.to_string", other),
    }
}

/// Parses an ISO-8601 date/time string. Accepts `YYYY-MM-DD`, optionally followed by
/// `T`, `t` or a space, then `HH:MM` or `HH:MM:SS[.fff]`, with an optional trailing `Z`/`z`.
/// A missing time means midnight. Returns `Undefined` for unparsable text.
fn time_from_string(val: Value) -> Value {
    match val {
        Value::Text(s) => match parse_iso_string(&s) {
            Some(t) => Value::Numb(t),
            None => undef_unary!("time.from_string", Value::Text(s)),
        },
        other => undef_unary!("time.from_string", other),
    }
}

/// `time.week_day(t)`: 0 = Sunday … 6 = Saturday.
fn time_week_day(val: Value) -> Value {
    match val {
        Value::Numb(t) => {
            let days = (t.floor() as i64).div_euclid(86400);
            // 1970-01-01 (day 0) was a Thursday, i.e. weekday 4
            let wd = (days + 4).rem_euclid(7);
            Value::Numb(wd as f64)
        }
        other => undef_unary!("time.week_day", other),
    }
}

/// `time.week_number(t)`: ISO-8601 week number (weeks start Monday; week 1 contains the year's first Thursday).
fn time_week_number(val: Value) -> Value {
    match val {
        Value::Numb(t) => {
            let days = (t.floor() as i64).div_euclid(86400);
            // weekday with Monday = 0 (day 0 was a Thursday, so +3)
            let wday_mon0 = (days + 3).rem_euclid(7);
            // the Thursday of the current ISO week. Its calendar year is the ISO year.
            let thu_days = days + 3 - wday_mon0;
            let (thu_y, _, _) = civil_from_days(thu_days);
            // January 4th is always in ISO week 1. `jan4_thu_days` is the Thursday of that week.
            let jan4_days = days_from_civil(thu_y, 1, 4);
            let jan4_wday_mon0 = (jan4_days + 3).rem_euclid(7);
            let jan4_thu_days = jan4_days + 3 - jan4_wday_mon0;
            // 1 + the number of whole weeks between the two Thursdays
            let wn = 1 + (thu_days - jan4_thu_days) / 7;
            Value::Numb(wn as f64)
        }
        other => undef_unary!("time.week_number", other),
    }
}



// ----------------------------------------------------------------------------
//  Private helpers
// ----------------------------------------------------------------------------

/// A broken-down UTC date and time.
struct DateTime {
    year: i64, month: u32, day: u32,
    hour: i64, minute: i64, second: f64, // `second` includes the fractional part
}

/// Splits epoch seconds into calendar fields (UTC). Works for negative times too.
fn datetime_from_epoch(t: f64) -> DateTime {
    let t_floor = t.floor() as i64;
    let sub_sec = t - (t_floor as f64);
    let days = t_floor.div_euclid(86400);
    let rem_secs = t_floor.rem_euclid(86400);
    let (year, month, day) = civil_from_days(days);
    DateTime {
        year, month, day,
        hour: rem_secs / 3600,
        minute: (rem_secs % 3600) / 60,
        second: (rem_secs % 60) as f64 + sub_sec,
    }
}

/// Combines calendar fields (UTC) into epoch seconds.
fn epoch_from_parts(y: i64, m: i64, d: i64, h: i64, min: i64, sec: f64) -> f64 {
    let days = days_from_civil(y, m, d);
    (days * 86400 + h * 3600 + min * 60) as f64 + sec
}

/// Reads a numeric field from a date namespace, or returns `default`
/// if the field is missing or not a `Numb`.
fn numb_field(scope: &Scope, name: &str, default: f64) -> f64 {
    match scope.get(name) {
        Value::Numb(n) => n,
        _ => default,
    }
}

/// Convert days since 1970-01-01 to (year, month, day) in civil Gregorian calendar.
///
/// This implementation uses Howard Hinnant's `civil_from_days` algorithm,
/// which works for the proleptic Gregorian calendar (i.e., it's valid
/// for dates before 1582 as well, though the results might not match
/// historical records).
///
/// `z`   : days since 0000-03-01
/// `era` : 400-year cycle
/// `doe` : day of era
/// `yoe` : year of era
/// `doy` : day of year counted from March 1
/// `mp`  : month counted from March
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// Convert (year, month, day) to days since 1970-01-01 in civil Gregorian calendar.
///
/// This implementation uses Howard Hinnant's `days_from_civil` algorithm,
/// which works for the proleptic Gregorian calendar.
///
/// `y`   : year (adjusted for March-based year start)
/// `era` : 400-year cycle
/// `yoe` : year of era
/// `doy` : day of year (0-indexed, from March 1st)
/// `doe` : day of era
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Parse an ISO-8601 date/time string into epoch seconds.
///
/// Accepts forms like:
/// - `YYYY-MM-DD` (time defaults to 00:00:00Z)
/// - `YYYY-MM-DDTHH:MM`
/// - `YYYY-MM-DDtHH:MM:SS`
/// - `YYYY-MM-DD HH:MM:SS.fffZ`
/// An optional trailing `Z` or `z` is ignored. Returns `None` on any malformed input,
/// including out-of-range month/day values.
fn parse_iso_string(s: &str) -> Option<f64> {
    let s = s.trim();
    // Split into date and optional time parts
    let parts: Vec<&str> = s.split(|c| c == 'T' || c == 't' || c == ' ').collect();
    if parts.is_empty() {
        return None;
    }

    // Parse date part (YYYY-MM-DD)
    let date_parts: Vec<&str> = parts[0].split('-').collect();
    if date_parts.len() != 3 {
        return None;
    }
    let y: i64 = date_parts[0].parse().ok()?;
    let m: i64 = date_parts[1].parse().ok()?;
    let d: i64 = date_parts[2].parse().ok()?;

    // Validate month and day ranges
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }

    // Parse optional time part (HH:MM:SS[.fff])
    let (h, min, sec) = if parts.len() > 1 {
        let time_str = parts[1].trim_end_matches(|c| c == 'Z' || c == 'z');
        let time_parts: Vec<&str> = time_str.split(':').collect();
        if time_parts.len() < 2 {
            return None;
        }
        let h: i64 = time_parts[0].parse().ok()?;
        let min: i64 = time_parts[1].parse().ok()?;
        let sec: f64 = if time_parts.len() >= 3 {
            time_parts[2].parse().ok()?
        } else {
            0.0
        };
        (h, min, sec)
    } else {
        (0, 0, 0.0)
    };

    Some(epoch_from_parts(y, m, d, h, min, sec))
}



// ----------------------------------------------------------------------------
//  Unit tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;
    use crate::eval::eval;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    /// Minimal root scope: the `time` namespace plus the boolean constants.
    /// Returns a child scope, mirroring how user code is evaluated.
    fn root_scope() -> Scope {
        let scope = Scope::new();
        scope.set("time", create_namespace(&scope));
        scope.set("TRUE", Value::Bool(true));
        scope.set("FALSE", Value::Bool(false));
        scope.new_child()
    }

    /// Lexes, parses and evaluates `src` in a fresh test scope.
    fn run(src: &str) -> Value {
        let scope = root_scope();
        let mut parser = Parser::new(Lexer::new(src));
        let ast = parser.parse();
        eval(&ast, &scope)
    }

    /// `time.now()`: returns a number.
    #[test]
    fn test_now() {
        assert!(matches!(run("time.now()"), Value::Numb(_)));
    }

    /// `time.to_date`: timestamp to date namespace.
    #[test]
    fn test_to_date() {
        // Number
        assert_eq!(
            run("d = time.to_date(1780000000), d.(year, month, day, hour, minute, second)"),
            Value::Tuple(Rc::new(vec![
                Value::Numb(2026.0), // year
                Value::Numb(5.0),    // month
                Value::Numb(28.0),   // day
                Value::Numb(20.0),   // hour (UTC)
                Value::Numb(26.0),   // minute
                Value::Numb(40.0),   // second
            ]))
        );

        // Nothing
        assert_eq!(run("time.to_date()"), Value::Nothing);

        // Tuple
        assert_eq!(
            run("(d1,d2) = time.to_date(1780000000, 1780000009), d1.[minute, second], d2.[minute, second]"),
            Value::Tuple(Rc::new(vec![
                Value::List(Rc::new(vec![Value::Numb(26.0), Value::Numb(40.0)])),
                Value::List(Rc::new(vec![Value::Numb(26.0), Value::Numb(49.0)])),
            ]))
        );

        // Others
        assert!(matches!(run("time.to_date []"), Value::Undefined(_)));
    }

    /// `time.from_date`: date namespace to timestamp.
    #[test]
    fn test_from_date() {
        // Number
        assert_eq!(
            run("time.from_date{year:2026, month:5, day:28, hour:20, minute:26, second:40}"),
            Value::Numb(1780000000.0)
        );

        // Missing fields default to 1970-01-01T00:00:00
        assert_eq!(run("time.from_date{year:1970}"), Value::Numb(0.0));

        // Nothing
        assert_eq!(run("time.from_date()"), Value::Nothing);

        // Tuple
        let d1 = "{year:2026, month:5, day:28, hour:20, minute:26, second:40}";
        let d2 = "{year:2026, month:5, day:28, hour:20, minute:26, second:49}";
        assert_eq!(
            run(&format!("time.from_date({}, {})", d1, d2)),
            Value::Tuple(Rc::new(vec![
                Value::Numb(1780000000.0),
                Value::Numb(1780000009.0),
            ]))
        );

        // Others
        assert!(matches!(run("time.from_date []"), Value::Undefined(_)));
    }

    /// `time.week_day`: returns day of week.
    #[test]
    fn test_week_day() {
        // Number (Sunday = 0 ... Saturday = 6)
        assert_eq!(run("time.week_day(1780000000)"), Value::Numb(4.0)); // Thursday
        assert_eq!(run("time.week_day(1780259200)"), Value::Numb(0.0)); // Sunday
        assert_eq!(run("time.week_day(1780345600)"), Value::Numb(1.0)); // Monday
        assert_eq!(run("time.week_day(0)"), Value::Numb(4.0));          // 1970-01-01, Thursday

        // Nothing
        assert_eq!(run("time.week_day()"), Value::Nothing);

        // Tuple
        assert_eq!(
            run("time.week_day(1780000000, 1780259200, 1780345600)"),
            Value::Tuple(Rc::new(vec![
                Value::Numb(4.0),
                Value::Numb(0.0),
                Value::Numb(1.0),
            ]))
        );

        // Others
        assert!(matches!(run("time.week_day([])"), Value::Undefined(_)));
    }

    /// `time.week_number`: returns ISO week number.
    #[test]
    fn test_week_number() {
        // Number (ISO-8601 week numbers)
        assert_eq!(run("time.week_number(1780000000)"), Value::Numb(22.0)); // Thursday
        assert_eq!(run("time.week_number(1780259200)"), Value::Numb(22.0)); // Sunday, same week
        assert_eq!(run("time.week_number(1780345600)"), Value::Numb(23.0)); // Monday, next week
        assert_eq!(run("time.week_number(0)"), Value::Numb(1.0));          // 1970-01-01 is in ISO week 1

        // Nothing
        assert_eq!(run("time.week_number()"), Value::Nothing);

        // Tuple
        assert_eq!(
            run("time.week_number(1780000000, 1780345600)"),
            Value::Tuple(Rc::new(vec![
                Value::Numb(22.0),
                Value::Numb(23.0),
            ]))
        );

        // Others
        assert!(matches!(run("time.week_number([])"), Value::Undefined(_)));
    }

    /// `time.to_string`: formats timestamp to ISO-8601 string.
    #[test]
    fn test_to_string() {
        // Number
        assert_eq!(
            run("time.to_string(1780000000)"),
            Value::Text("2026-05-28T20:26:40.000Z".into())
        );

        // Number with fractional seconds
        assert_eq!(
            run("time.to_string(1780000000.5)"),
            Value::Text("2026-05-28T20:26:40.500Z".into())
        );
        assert_eq!(run("time.to_string(0)"), Value::Text("1970-01-01T00:00:00.000Z".into()));

        // Nothing
        assert_eq!(run("time.to_string()"), Value::Nothing);

        // Tuple
        assert_eq!(
            run("time.to_string(1780000000, 1780000009)"),
            Value::Tuple(Rc::new(vec![
                Value::Text("2026-05-28T20:26:40.000Z".into()),
                Value::Text("2026-05-28T20:26:49.000Z".into()),
            ]))
        );

        // Others
        assert!(matches!(run("time.to_string([])"), Value::Undefined(_)));
        assert!(matches!(run(r#"time.to_string("abc")"#), Value::Undefined(_)));
    }

    /// `time.from_string`: parses ISO-8601 string to timestamp.
    #[test]
    fn test_from_string() {
        // Text (full date-time)
        assert_eq!(
            run(r#"time.from_string("2026-05-28T20:26:40.000Z")"#),
            Value::Numb(1780000000.0)
        );

        // Text (date only -> midnight UTC)
        assert_eq!(
            run(r#"time.from_string("2026-05-28")"#),
            Value::Numb(1779926400.0)
        );

        // Space as date/time separator, no trailing Z
        assert_eq!(
            run(r#"time.from_string("2026-05-28 20:26:40")"#),
            Value::Numb(1780000000.0)
        );

        // Nothing
        assert_eq!(run("time.from_string()"), Value::Nothing);

        // Tuple
        assert_eq!(
            run(r#"time.from_string("2026-05-28T20:26:40Z", "2026-05-28T20:26:49Z")"#),
            Value::Tuple(Rc::new(vec![
                Value::Numb(1780000000.0),
                Value::Numb(1780000009.0),
            ]))
        );

        // Round trip
        assert_eq!(
            run("time.from_string(time.to_string(1780000000))"),
            Value::Numb(1780000000.0)
        );

        // Others
        assert!(matches!(run("time.from_string([])"), Value::Undefined(_)));
        assert!(matches!(run(r#"time.from_string("not a date")"#), Value::Undefined(_)));
        assert!(matches!(run(r#"time.from_string("2026-13-01")"#), Value::Undefined(_)));
        assert!(matches!(run(r#"time.from_string("2026-05-00")"#), Value::Undefined(_)));
    }
}
````

## File: src/builtins/undef.rs
````rust
//! The `undef` built-in namespace.
//!
//! Provides helpers for explicitly constructing `Value::Undefined` values
//! from user code.
//!
//! | Member       | Description                                              |
//! |--------------|----------------------------------------------------------|
//! | `undef.from` | Wraps any value into `Undefined(Undef::Value(<value>))`. |

use crate::value::{BuiltinFn, Scope, Undef, Value};



// ----------------------------------------------------------------------------
//  `undef` built-in namespace
// ----------------------------------------------------------------------------

/// Builds the `undef` namespace as a child of `parent`.
///
/// The child scope keeps the parent's bindings visible from inside the
/// namespace. It currently contains only `from`.
pub fn create_namespace(parent: &Scope) -> Value {
    let undef_scope = parent.new_child();
    undef_scope.set("from", Value::Builtin(BuiltinFn::new("undef.from", undef_from)));
    Value::Namespace(undef_scope)
}

/// `undef.from X` — wraps `X` into `Value::Undefined(Undef::Value(X))`.
///
/// The argument is wrapped as a whole and is not mapped element-wise, so a
/// tuple argument produces a single `Undefined` that holds the entire tuple.
/// `Nothing` is wrapped too.
fn undef_from(val: Value) -> Value {
    Value::Undefined(Undef::Value(Box::new(val)))
}



// ----------------------------------------------------------------------------
//  UNIT TESTS
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::eval;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn root_scope() -> Scope {
        let scope = Scope::new();
        scope.set("undef", create_namespace(&scope));
        scope.set("TRUE"  , Value::Bool(true));
        scope.set("FALSE" , Value::Bool(false));
        scope.new_child()
    }

    fn run(src: &str) -> Value {
        let scope = root_scope();
        let mut parser = Parser::new(Lexer::new(src));
        let ast = parser.parse();
        eval(&ast, &scope)
    }

    fn undefined(val: Value) -> Value {
        Value::Undefined(Undef::Value(Box::new(val)))
    }

    #[test]
    fn test_undef_from() {
        assert_eq!(
            run("undef.from 10"), 
            Value::Undefined(Undef::Value(Box::new(Value::Numb(10.0))))
        );
        assert_eq!(
            run("undef.from \"abc\""), 
            Value::Undefined(Undef::Value(Box::new(Value::Text("abc".into()))))
        );

        // Undefined from Bool
        assert_eq!(run("undef.from TRUE"),  undefined(Value::Bool(true)));
        assert_eq!(run("undef.from FALSE"), undefined(Value::Bool(false)));

        // Undefined from Nothing
        assert_eq!(undef_from(Value::Nothing), undefined(Value::Nothing));

        // Nested Undefined
        assert_eq!(
            run("undef.from (undef.from 1)"),
            undefined(undefined(Value::Numb(1.0)))
        );
    }
}
````

## File: src/ast.rs
````rust
#[derive(Debug, PartialEq, Clone)]
pub struct Leaf<T> {
    pub pos: usize,
    pub val: T,
}


#[derive(Debug, PartialEq, Clone)]
pub struct UnaryOp {
    pub pos: usize,
    pub opd: Box<Node>,
}


#[derive(Debug, PartialEq, Clone)]
pub struct BinaryOp {
    pub pos: usize,
    pub lho: Box<Node>,
    pub rho: Box<Node>,
}




#[derive(Debug, PartialEq, Clone)]
pub enum Node {

    // Literals / Leaves
    Identifier(Leaf<String>),
    Numb(Leaf<f64>),
    Text(Leaf<String>),
    Nothing(Leaf<()>),
    Unclosed(Leaf<char>),
    UnexpectedToken(Leaf<String>),


    // Grouping 
    List(UnaryOp),          // [ ... ]
    Namespace(UnaryOp),     // { ... }


    // Unary Operations
    AffirmOp(UnaryOp),          // +
    NegateOp(UnaryOp),          // -
    LogicalNegationOp(UnaryOp), // !
    LiftingOp(UnaryOp),         // $


    // Binary Operations :: Core
    FunctionDefinitionOp(BinaryOp), // -> 
    ApplicationOp(BinaryOp),        // no operator
    PairingOp(BinaryOp),            // ,
    MappingOp(BinaryOp),            // =>
    AssignmentOp(BinaryOp),         // =
    LabellingOp(BinaryOp),          // :
    SubcontextingOp(BinaryOp),      // .


    // Binary Operations :: Arithmetic
    PowerOp(BinaryOp),          // ^
    ProductOp(BinaryOp),        // *
    DivisionOp(BinaryOp),       // /
    ModuloOp(BinaryOp),         // %
    SumOp(BinaryOp),            // +
    SubtractionOp(BinaryOp),    // -


    // Binary Operations :: Comparison
    EqualToOp(BinaryOp),                // ==
    NotEqualToOp(BinaryOp),             // !=
    LessThanOp(BinaryOp),               // <
    LessThanOrEqualToOp(BinaryOp),      // <=
    GreaterThanOp(BinaryOp),            // >
    GreaterThanOrEqualToOp(BinaryOp),   // >=


    // Binary Operations :: Logic
    AndOp(BinaryOp),            // &
    OrOp(BinaryOp),             // |


    // Binary Operations :: Conditional
    ConditionalOp(BinaryOp),    // ?
    AlternativeOp(BinaryOp),    // ;


    // Binary Operations :: Function Composition
    CompositionOp(BinaryOp),         // <<
    ReverseCompositionOp(BinaryOp),  // >>
}


impl Node {
    
    pub fn position (&self) -> usize {
        match self {

            Self::Identifier(leaf)      => leaf.pos,
            Self::Numb(leaf)            => leaf.pos,
            Self::Text(leaf)            => leaf.pos,
            Self::Nothing(leaf)         => leaf.pos,
            Self::Unclosed(leaf)        => leaf.pos,
            Self::UnexpectedToken(leaf) => leaf.pos,

            Self::List(unary)      => unary.pos,
            Self::Namespace(unary) => unary.pos,

            Self::AffirmOp(unary)          => unary.pos,
            Self::NegateOp(unary)          => unary.pos,
            Self::LogicalNegationOp(unary) => unary.pos,
            Self::LiftingOp(unary)         => unary.pos,

            Self::FunctionDefinitionOp(binary) => binary.pos,
            Self::ApplicationOp(binary)        => binary.pos,
            Self::PairingOp(binary)            => binary.pos,
            Self::MappingOp(binary)            => binary.pos,
            Self::AssignmentOp(binary)         => binary.pos,
            Self::LabellingOp(binary)          => binary.pos,
            Self::SubcontextingOp(binary)      => binary.pos,

            Self::PowerOp(binary)       => binary.pos,
            Self::ProductOp(binary)     => binary.pos,
            Self::DivisionOp(binary)    => binary.pos,
            Self::ModuloOp(binary)      => binary.pos,
            Self::SumOp(binary)         => binary.pos,
            Self::SubtractionOp(binary) => binary.pos,

            Self::EqualToOp(binary)              => binary.pos,
            Self::NotEqualToOp(binary)           => binary.pos,
            Self::LessThanOp(binary)             => binary.pos,
            Self::LessThanOrEqualToOp(binary)    => binary.pos,
            Self::GreaterThanOp(binary)          => binary.pos,
            Self::GreaterThanOrEqualToOp(binary) => binary.pos,

            Self::AndOp(binary) => binary.pos,
            Self::OrOp(binary)  => binary.pos,

            Self::ConditionalOp(binary) => binary.pos,
            Self::AlternativeOp(binary) => binary.pos,

            Self::CompositionOp(binary)        => binary.pos,
            Self::ReverseCompositionOp(binary) => binary.pos,
        }
    }

}
````

## File: src/builtins.rs
````rust
// src/builtins.rs
//
// Purpose: this module builds the root scope and defines the built-ins that
// live directly in it (`type`, `parse`, `TRUE`, `FALSE`). Every other
// built-in lives in a submodule that exposes `create_namespace(&Scope) -> Value`.
//
// Adding a root-level function:
//   1. Write `fn my_fn(arg: Value) -> Value`.
//   2. Register it in `root_scope()` with `scope.set("my_fn", map_1("my_fn", my_fn))`.
//      `map_1` makes it map over tuples and pass `Nothing` through.
//
// Adding a function to a namespace: put it in the matching submodule (`numb`,
// `text`, `time`, …) and register it in that module's `create_namespace`.
//
// Multi-argument functions: these are curried. Use `map_curried` from `helpers`,
// so `math.pow 10` returns a function that takes the second argument.
//
// Errors: never panic. Return `Undefined` through the `undef_unary!` macro (or
// its binary counterpart in `helpers`), using the function's own name as the
// `op` label.

use crate::value::{BuiltinFn, Scope, Undef, Value};
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::eval::eval;

// Shared helpers and macros (map_1, map_curried, undef_unary!, ...).
mod helpers;
use helpers::*;

// One submodule per built-in namespace; each exposes create_namespace().
mod bool_;
mod numb;
mod text;
mod list;
mod nspace;
mod undef;
mod math;
mod time;



// ----------------------------------------------------------------------------
//  Root scope factory
// ----------------------------------------------------------------------------

/// Build a fresh root `Scope` pre-populated with all built-in bindings.
///
/// Every evaluation session should start from a scope created here (or from a
/// child of one) so that built-in names are always available.
///
/// ```
/// use swan_core::builtins::root_scope;
/// use swan_core::eval::eval;
/// use swan_core::lexer::Lexer;
/// use swan_core::parser::Parser;
///
/// let scope = root_scope();
/// let mut parser = Parser::new(Lexer::new("1+2"));
/// let ast = parser.parse();
/// assert_eq!(eval(&ast, &scope), swan_core::value::Value::Numb(3.0));
/// ```
pub fn root_scope() -> Scope {
    // Built-ins are installed in this base scope.
    let scope = Scope::new();

    // Each namespace is registered under its own name, e.g. `math.sqrt`.
    scope.set("bool"  , bool_::create_namespace(&scope));
    scope.set("numb"  , numb::create_namespace(&scope));
    scope.set("text"  , text::create_namespace(&scope));
    scope.set("list"  , list::create_namespace(&scope));
    scope.set("nspace", nspace::create_namespace(&scope));
    scope.set("undef" , undef::create_namespace(&scope));
    scope.set("math"  , math::create_namespace(&scope));
    scope.set("time"  , time::create_namespace(&scope));

    scope.set("TRUE" , Value::Bool(true));
    scope.set("FALSE", Value::Bool(false));

    scope.set("type" , map_1("type" , type_function));
    scope.set("parse", map_1("parse", parse_function));

    // Returns a fresh empty child scope. User code then defines and shadows
    // names in the child and cannot overwrite the built-ins stored in the parent.
    scope.new_child()
}



// ----------------------------------------------------------------------------
//  built-in functions
// ----------------------------------------------------------------------------

/// Builtin `type`: returns the type name of its argument as a `Text`.
///
/// Wrapped with `map_1`, so `type()` is `Nothing` and `type(a, b)` gives a
/// tuple of type names. Both `Func` and `Builtin` values report `"Func"`.
fn type_function(val: Value) -> Value {
    let n = match val {
        Value::Nothing => "Nothing",
        Value::Bool(_) => "Bool",
        Value::Numb(_) => "Numb",
        Value::Text(_) => "Text",
        Value::List(_) => "List",
        Value::Namespace(_) => "Namespace",
        Value::Func { .. } | Value::Builtin(_) => "Func",
        Value::Undefined(_) => "Undefined",
        Value::Tuple(_) => "Tuple",
        Value::Name(_) => "Name",
    };
    Value::Text(n.into())
}

/// Builtin `parse` function:
///   - takes a `Text` containing source code and parses it immediately.
///   - returns a builtin function named `"eval"`.
///   - that function takes a `Namespace` and evaluates the captured AST in
///     that namespace as its scope.
///   - a non-`Text` argument to `parse` gives `Undefined`, labelled `"parse"`.
///   - a non-`Namespace` argument to the returned function gives `Undefined`,
///     labelled `"eval"`.
fn parse_function(arg: Value) -> Value {
    match arg {
        Value::Text(source) => {
            let mut parser = Parser::new(Lexer::new(&source));
            let ast = parser.parse();

            // The closure takes ownership of the AST.
            Value::Builtin(BuiltinFn::new("eval", move |ns_arg| {
                match ns_arg {
                    Value::Namespace(scope) => eval(&ast, &scope),
                    other => undef_unary!("eval", other),
                }
            }))
        }
        other => undef_unary!("parse", other),
    }
}



// ----------------------------------------------------------------------------
//  Tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    /// Evaluates `src` in a fresh root scope.
    fn run(src: &str) -> Value {
        let scope = root_scope();
        let mut parser = Parser::new(Lexer::new(src));
        let ast = parser.parse();
        eval(&ast, &scope)
    }

    #[test]
    fn test_type() {
        // Item
        assert_eq!(run("type(1 == 1)"), Value::Text("Bool".into()));
        assert_eq!(run("type 10"), Value::Text("Numb".into()));
        assert_eq!(run(r#"type "abc""#), Value::Text("Text".into()));
        assert_eq!(run("type [1, 2]"), Value::Text("List".into()));
        assert_eq!(run("type {a: 1}"), Value::Text("Namespace".into()));
        assert_eq!(run("type(x -> x)"), Value::Text("Func".into()));
        assert_eq!(run("type type"), Value::Text("Func".into()));

        // Nothing
        assert_eq!(run("type()"), Value::Nothing);

        // Tuple
        assert_eq!(
            run(r#"type(10, "abc", [1])"#),
            Value::Tuple(Rc::new(vec![
                Value::Text("Numb".into()),
                Value::Text("Text".into()),
                Value::Text("List".into()),
            ]))
        );
    }

    #[test]
    fn test_parse() {
        // Evaluation with a scope argument
        assert_eq!(
            run(r#"eval_fn = parse "a + b", eval_fn { a = 10, b = 20 }"#),
            Value::Numb(30.0)
        );

        // Returns a function/closure
        assert_eq!(
            run(r#"double = (parse "x -> x * 2") {}, double 5"#),
            Value::Numb(10.0)
        );

        // Nothing
        assert_eq!(run("parse()"), Value::Nothing);

        // Tuple
        assert_eq!(
            run(r#"(f,g) = parse("2*x", "3*x"), f{x:10}, g{x:20}"#),
            Value::Tuple(Rc::new(vec![
                Value::Numb(20.0),
                Value::Numb(60.0),
            ])),
        );

        // Invalid argument type returns Undefined
        assert!(matches!(run("parse 123"), Value::Undefined(_)));
    }
}
````

## File: src/eval.rs
````rust
// src/eval.rs

use std::rc::Rc;
use std::cmp::Ordering;
use crate::ast::Node;
use crate::value::{Scope, Value, Undef, BuiltinFn};





// ----------------------------------------------------------------------------
//  MACROS
// ----------------------------------------------------------------------------

/// Evaluates a binary operation with element-wise tuple broadcasting (§3.5).
///
/// If either operand is a [`Value::Tuple`], pairs the items using [`Value::pair`]
/// and applies `$eval_fn` element-wise. Otherwise, applies `$eval_fn` directly.
///
/// Requires 4 arguments: `($lhv, $rhv, $eval_fn, $pos)`
macro_rules! eval_binary {
    ($eval_fn:path, $lhv:expr, $rhv:expr, $pos:expr) => {{
        let (lhv, rhv) = ($lhv, $rhv);
        if matches!(lhv, Value::Tuple(_)) || matches!(rhv, Value::Tuple(_)) {
            Value::Tuple(Rc::new(
                lhv.pair(&rhv)
                    .map(|(l, r)| $eval_fn(l, r, $pos))
                    .collect(),
            ))
        } else if lhv == Value::Nothing && rhv == Value::Nothing {
            Value::Nothing
        } else {
            $eval_fn(lhv, rhv, $pos)
        }
    }};
}





// ----------------------------------------------------------------------------
//  PUBLIC ENTRY POINT
// ----------------------------------------------------------------------------

/// Evaluates an AST node within a given `Scope` context and returns a `Value`.
///
/// This function is **infallible**: it never panics.  All error conditions are
/// encoded as `Value::Undefined(...)` so that Swan's "errors are values" semantics
/// are preserved throughout. (#14)
pub fn eval(node: &Node, scope: &Scope) -> Value {
    match node {


        // ===== Literals / Leafs =====

        // Numeric literal
        Node::Numb(leaf)       => Value::Numb(leaf.val),

        // String literal
        Node::Text(leaf)       => Value::Text(leaf.val.as_str().into()),

        // Nothing literal: `()` or empty expression
        Node::Nothing(_)       => Value::Nothing,

        // Identifier lookup
        Node::Identifier(leaf) => scope.get(&leaf.val),


        // ===== Grouping =====

        // List creation: `[X]`
        Node::List(unary) => {
            let val = eval(&unary.opd, scope);
            let vec: Vec<_> = val.items().collect();
            Value::List(Rc::new(vec))
        }

        // Namespace creation: `{X}`
        Node::Namespace(unary) => {
            let child_scope = scope.new_child();
            eval(&unary.opd, &child_scope);
            Value::Namespace(child_scope)
        }



        // ===== Unary Operations =====

        // Unary affimr operation: `+X`
        Node::AffirmOp(unary) => {
            let val = eval(&unary.opd, scope);
            eval_affirm(val)          // pos not needed — pure identity
        }

        // Unary negation operation: `-X`
        Node::NegateOp(unary) => {
            let val = eval(&unary.opd, scope);
            if matches!(val, Value::Tuple(_)) {
                Value::Tuple(Rc::new(
                    val.items()
                    .map(|v| eval_negate(v, unary.pos))
                    .collect()
                ))
            } else {
                eval_negate(val, unary.pos)
            }
        }

        // Unary logical negation operation: `!X`
        Node::LogicalNegationOp(unary) => {
            let val = eval(&unary.opd, scope);
            Value::Bool(!bool::from(val))
        }

        // Unary lifting operation: `$X`
        Node::LiftingOp(unary) => {
            let names = eval_names(&unary.opd);
            if matches!(names, Value::Tuple(_)) {
                Value::Tuple(Rc::new(
                    names.items()
                    .map(|n| eval_lifting(n, &scope, unary.pos))
                    .collect()
                ))
            } else {
                eval_lifting(names, &scope, unary.pos)
            }
        }



        // ===== Binary Operations :: Core =====

        // Function definition: `A -> B`
        Node::FunctionDefinitionOp(bin) => {
            let names = eval_names(&bin.lho);
            if matches!(names, Value::Undefined(_)) {
                Value::Undefined(Undef::BinaryOperation{
                    op: "function_definition",
                    lhv: Box::new(names),
                    rhv: Box::new(Value::Nothing),
                    pos: bin.pos,
                })
            } else {
                Value::Func{
                    params: Box::new(names),
                    body: bin.rho.clone(),
                    closure: scope.clone(),
                }
            }
        }

        // `f x` — function application.
        Node::ApplicationOp(bin) => {
            // The left-hand operand must evaluate to a callable (Builtin or Func).
            // The right-hand operand is the argument (possibly a Tuple for multiple
            // arguments). Builtin closures receive the whole argument Value and are
            // responsible for destructuring it if needed.
            let func = eval(&bin.lho, scope);
            let args= eval(&bin.rho, scope);
            eval_apply(func, args, bin.pos)
        }

        // Pairing operation: `X , Y`
        Node::PairingOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            eval_pairing(lhv, rhv)
        }

        // Mapping: `T => F`
        Node::MappingOp(bin) => {
            let args = eval(&bin.lho, scope);
            let func = eval(&bin.rho, scope);
            let vals: Vec<Value> = args.items().map(|arg| eval_apply(func.clone(), arg, bin.pos)).collect();
            match vals.len() {
                0 => Value::Nothing,
                1 => vals[0].clone(),
                _ => Value::Tuple(vals.into()),
            }
        }

        // Assignemt: `name = V`
        Node::AssignmentOp(bin) => {
            let names = eval_names(&bin.lho);
            if matches!(names, Value::Undefined(_)) {
                return Value::Undefined(Undef::BinaryOperation{
                    op: "assignment",
                    lhv: Box::new(names),
                    rhv: Box::new(Value::Nothing),
                    pos: bin.pos,
                });
            }

            let values = eval(&bin.rho, scope);
            eval_assignment(scope, &names, &values);
            Value::Nothing
        }

        // Labelling: `name : V`
        Node::LabellingOp(bin) => {
            let names = eval_names(&bin.lho);
            if matches!(names, Value::Undefined(_)) {
                return Value::Undefined(Undef::BinaryOperation{
                    op: "labelling",
                    lhv: Box::new(names),
                    rhv: Box::new(Value::Nothing),
                    pos: bin.pos,
                });
            }

            let values = eval(&bin.rho, scope);
            eval_assignment(scope, &names, &values);
            values
        }

        // `ns.expression` — namespace sub-contexting
        Node::SubcontextingOp(bin) => {
            // The left-hand side must evaluate to a `Value::Namespace`.
            // The right-hand side must be an `Identifier` node; its string is used
            // as the lookup key, not its evaluated value.
            match eval(&bin.lho, scope) {
                Value::Namespace(ns_scope) => {
                    eval(&bin.rho, &ns_scope)
                }
                lhv => Value::Undefined(Undef::BinaryOperation {
                    op:  "subcontexting",
                    lhv: Box::new(lhv),
                    rhv: Box::new(Value::Nothing),  // rhv can´t be evaluated without a scope
                    pos: bin.pos,
                }),
            }
        }



        // ===== Arithmetic Operations =====

        // Sum: `A + B`
        Node::SumOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            eval_binary!(eval_add, lhv, rhv, bin.pos)
        }

        // Subtraction: `A - B`
        Node::SubtractionOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            eval_binary!(eval_subtraction, lhv, rhv, bin.pos)
        }

        // Product: `A * B`
        Node::ProductOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            eval_binary!(eval_product, lhv, rhv, bin.pos)
        }

        // Division: `A / B`
        Node::DivisionOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            eval_binary!(eval_division, lhv, rhv, bin.pos)
        }

        // Modulo: `A % B`
        Node::ModuloOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            eval_binary!(eval_modulo, lhv, rhv, bin.pos)
        }

        // Exponentiation: `A ^ B`
        Node::PowerOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            eval_binary!(eval_power, lhv, rhv, bin.pos)
        }



        // ===== Comparison Operations =====

        // Equal: `A == B`
        Node::EqualToOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            eval_equal_to(lhv, rhv)
        }

        // Non-equal: `A != B`
        Node::NotEqualToOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            eval_not_equal_to(lhv, rhv)
        }

        // Greater-than: `A > B`
        Node::GreaterThanOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            eval_greater_than(lhv, rhv, bin.pos)
        }

        // Non-greater-than: `A <= B`
        Node::GreaterThanOrEqualToOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            eval_greater_than_or_equal_to(lhv, rhv, bin.pos)
        }

        // Less than: `A < B`
        Node::LessThanOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            eval_less_than(lhv, rhv, bin.pos)
        }

        // Non-less-than: `A >= B`
        Node::LessThanOrEqualToOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            eval_less_than_or_equal_to(lhv, rhv, bin.pos)
        }



        // ===== Logic Operations =====

        // And: `A & B`
        Node::AndOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            if bool::from(lhv.clone()) {
                eval(&bin.rho, scope)
            } else {
                lhv
            }
        }

        // Or: `A | B`
        Node::OrOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            if bool::from(lhv.clone()) {
                lhv
            } else {
                eval(&bin.rho, scope)
            }
        }



        // ===== Conditional and alternative =====

        // Conditional: `A ? B`
        Node::ConditionalOp(bin) => {
            let cnd = bool::from(eval(&bin.lho, scope));
            if cnd {
                eval(&bin.rho, scope)
            } else {
                Value::Nothing
            }
        }

        // Alternative: `A ; B`
        Node::AlternativeOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            match lhv {
                Value::Nothing => eval(&bin.rho, scope),
                _ => lhv
            }
        }



        // ===== Function composition =====

        // Composition: `F << G`
        Node::CompositionOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            eval_binary!(eval_composition, lhv, rhv, bin.pos)
        }

        // Reverse composition: `F >> G`
        Node::ReverseCompositionOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            eval_binary!(eval_composition, rhv, lhv, bin.pos)
        }



        // ===== Syntax Errors =====

        // Missing closing bracket
        Node::Unclosed(leaf) => {
            Value::Undefined(Undef::Termination{
                expected: leaf.val,
                pos: leaf.pos,
            })
        }

        // Undefined token
        Node::UnexpectedToken(leaf) => {
            Value::Undefined(Undef::Token{
                token: leaf.val.clone(),
                pos: leaf.pos
            })
        }
    }
}

/// Evaluates an AST nodes that are supposed to resolve into names
///
/// This functions resolves values or tuple at the left side of an
/// assignment operation or at the left side of a function definition.
/// Those values can be either an identifier or a tuple of identifiers.
fn eval_names(node: &Node) -> Value {
    match node {

        // Identifier lookup
        Node::Identifier(leaf) => Value::Name(leaf.val.as_str().into()),

        // Pairing operation: `X , Y`
        Node::PairingOp(bin) => {
            let name1 = eval_names(&bin.lho);
            let name2 = eval_names(&bin.rho);

            // Short-circuit / propagate if either side failed to produce a valid name
            if matches!(name1, Value::Undefined(_)) {
                return name1;
            }
            if matches!(name2, Value::Undefined(_)) {
                return name2;
            }

            eval_pairing(name1, name2)
        }

        // Any other value will return and undefined value
        node => Value::Undefined(Undef::Name{ pos: node.position() }),
    }
}





// ----------------------------------------------------------------------------
//  UNARY OPERATIONS
// ----------------------------------------------------------------------------

/// `+X` — identity for ALL Swan value types. (#10)
///
/// In JS this is literally `return this.operand.evaluate(context)` with no
/// type guard.  Restricting it to Numb only was a bug.
fn eval_affirm(val: Value) -> Value {
    val
}

/// `-X` — negation.
///
/// Currently only defined for `Numb`. When tuples are supported the operation
/// should be mapped element-wise over all items. (#11)
fn eval_negate(val: Value, pos: usize) -> Value {
    match val {
        Value::Numb(n) => Value::Numb(-n),
        other => Value::Undefined(Undef::UnaryOperation {
            op:  "negation",
            val: Box::new(other),
            pos,
        }),
    }
}

fn eval_lifting(name: Value, scope: &Scope, pos: usize) -> Value {
    match name {
        Value::Name(n) => scope.lift(&n),
        other => Value::Undefined(Undef::UnaryOperation {
            op:  "lifting",
            val: Box::new(other),
            pos,
        }),
    }
}





// ----------------------------------------------------------------------------
//  CORE BINARY OPERATIONS
// ----------------------------------------------------------------------------

/// `func arg` — applies a callable Swan value to an argument.
///
/// Dispatches on the type of `func`:
///
/// - `Value::Builtin(f)` — calls the native Rust function directly.
/// - `Value::Func { ... }` — user-defined function (not yet implemented).
/// - Anything else — returns `Undefined`.
fn eval_apply(func: Value, arg: Value, pos: usize) -> Value {
    func.map(move |f| eval_apply_1(f, arg.clone(), pos))
}

fn eval_apply_1(func: Value, arg: Value, pos: usize) -> Value {
    // // 1. Broadcast over Tuple on the left: applying each function in a tuple to `arg`
    // if matches!(func, Value::Tuple(_)) {
    //     let results: Vec<Value> = func
    //         .items()
    //         .map(|f| eval_apply(f, arg.clone(), pos))
    //         .collect();
    //     return match results.len() {
    //         0 => Value::Nothing,
    //         1 => results[0].clone(),
    //         _ => Value::Tuple(Rc::new(results)),
    //     };
    // }

    // 2. Dispatch single-item application by type
    match func {
        Value::Builtin(ref f) => f.call(arg),

        Value::Func { params, body, closure } => {
            let local_scope = closure.new_child();
            eval_assignment(&local_scope, &params, &arg);
            eval(&body, &local_scope)
        }

        // --- List Application ---
        Value::List(list) => {
            if matches!(arg, Value::Tuple(_)) {
                let items: Vec<Value> = arg
                    .items()
                    .map(|idx| eval_apply_list(&list, idx, pos))
                    .collect();
                match items.len() {
                    0 => Value::Nothing,
                    1 => items[0].clone(),
                    _ => Value::Tuple(Rc::new(items)),
                }
            } else {
                eval_apply_list(&list, arg, pos)
            }
        }

        // --- Namespace Application ---
        Value::Namespace(scope) => {
            if matches!(arg, Value::Tuple(_)) {
                let items: Vec<Value> = arg
                    .items()
                    .map(|key| eval_apply_namespace(&scope, key, pos))
                    .collect();
                match items.len() {
                    0 => Value::Nothing,
                    1 => items[0].clone(),
                    _ => Value::Tuple(Rc::new(items)),
                }
            } else {
                eval_apply_namespace(&scope, arg, pos)
            }
        }

        other => Value::Undefined(Undef::BinaryOperation {
            op: "application",
            lhv: Box::new(other),
            rhv: Box::new(arg),
            pos,
        }),
    }
}

/// Applies a List to a single index argument `n`.
fn eval_apply_list(list: &[Value], arg: Value, pos: usize) -> Value {
    match arg {
        Value::Numb(n) => {
            if n >= 0.0 && n.fract() == 0.0 {
                let idx = n as usize;
                if idx < list.len() {
                    return list[idx].clone();
                }
            }
            // Index is out of bounds or not a whole integer
            Value::Nothing
        }
        // Argument is not a number
        other => Value::Undefined(Undef::BinaryOperation {
            op: "application",
            lhv: Box::new(Value::List(Rc::new(list.to_vec()))),
            rhv: Box::new(other),
            pos,
        }),
    }
}

/// Applies a Namespace to a single key argument `t`.
fn eval_apply_namespace(scope: &Scope, arg: Value, pos: usize) -> Value {
    match arg {
        Value::Text(t) => {
            // Own-only lookup per Swan spec §3.7
            let found = scope.0.borrow().bindings.get(t.as_ref()).cloned();
            found.unwrap_or(Value::Nothing)
        }
        // Argument is not a string
        other => Value::Undefined(Undef::BinaryOperation {
            op: "application",
            lhv: Box::new(Value::Namespace(scope.clone())),
            rhv: Box::new(other),
            pos,
        }),
    }
}

/// `X , Y` - pairing operation
fn eval_pairing(lhv: Value, rhv: Value) -> Value {
    let mut vec: Vec<_> = lhv.items().chain(rhv.items()).collect();
    match vec.len() {
        0 => Value::Nothing,
        1 => vec.pop().unwrap(),
        _ => Value::Tuple(Rc::new(vec)),
    }
}

/// `X = Y` - assignment operation
fn eval_assignment(scope: &Scope, names: &Value, values: &Value) {
    let mut names_iter = names.items();
    let mut values_iter = values.items();
    let mut last_name: Rc<str> = "__uname__".into();
    loop {
        let name = names_iter.next();
        let value = values_iter.next();
        match (name, value) {

            (Some(Value::Name(name)), Some(value)) => {
                scope.set(name.as_ref(), value);
                last_name = name;
            }

            (Some(Value::Name(name)), None) => {
                scope.set(name.as_ref(), Value::Nothing);
                last_name = name;
            }

            (None, Some(value)) => {
                let prev_value = scope.get(last_name.as_ref());
                scope.set(last_name.as_ref(), eval_pairing(prev_value, value));
            }

            (None, None) => {
                break;
            }

            _ => {
                scope.set("__uname__", Value::Nothing);
            }
        }
    }
}





// ----------------------------------------------------------------------------
//  Arithmetic binary operations
// ----------------------------------------------------------------------------

/// `X + Y` — type-homogeneous sum.
///
/// Matches the JS `SumOperation`: both operands must be the *same* Swan type
/// and that type must define a `sum` operation.  Mismatched types always
/// produce `Undefined`. (#12)
///
/// Currently implemented:
///   Numb  + Numb  → number addition
///   Text  + Text  → string concatenation
///   List  + List  → list concatenation
///   Namespace + Namespace → shallow merge (right wins on collision)
///
/// When tuples are supported, the operation should iterate `iterPairs` over
/// the two tuples and apply element-wise, assembling the results into a new
/// Tuple. (#11)
fn eval_add(lhv: Value, rhv: Value, pos: usize) -> Value {
    match (lhv, rhv) {

        (Value::Numb(a), Value::Numb(b)) => Value::Numb(a + b),

        (Value::Bool(a), Value::Bool(b)) => Value::Bool(a || b),

        (Value::Text(a), Value::Text(b)) => {
            // Concatenate two Rc<str> values without going through String twice.
            let mut s = String::with_capacity(a.len() + b.len());
            s.push_str(&a);
            s.push_str(&b);
            Value::Text(s.into())
        }

        (Value::List(a), Value::List(b)) => {
            // Concatenate two Rc<Vec<Value>>: clone items from both sides.
            let mut v = Vec::with_capacity(a.len() + b.len());
            v.extend_from_slice(&a);
            v.extend_from_slice(&b);
            Value::List(Rc::new(v))
        }

        (lhv, rhv) => Value::Undefined(Undef::BinaryOperation {
            op:  "sum",
            lhv: Box::new(lhv),
            rhv: Box::new(rhv),
            pos,
        }),
    }
}

/// `X - Y` — subtraction.
///
/// Currently implemented:
///   Numb  - Numb  → number subtraction
///
/// When tuples are supported, the operation should iterate `iterPairs` over
/// the two tuples and apply element-wise, assembling the results into a new
/// Tuple. (#11)
fn eval_subtraction(lhv: Value, rhv: Value, pos: usize) -> Value {
    match (lhv, rhv) {

        (Value::Numb(a), Value::Numb(b)) => Value::Numb(a - b),

        (lhv, rhv) => Value::Undefined(Undef::BinaryOperation {
            op:  "subtraction",
            lhv: Box::new(lhv),
            rhv: Box::new(rhv),
            pos,
        }),
    }
}

/// `X * Y` — product
///
/// Currently implemented:
///   Numb * Numb  → number product
///
fn eval_product(lhv: Value, rhv: Value, pos: usize) -> Value {
    match (lhv, rhv) {

        (Value::Numb(a), Value::Numb(b)) => Value::Numb(a * b),

        (Value::Bool(a), Value::Bool(b)) => Value::Bool(a && b),

        (lhv, rhv) => Value::Undefined(Undef::BinaryOperation {
            op:  "product",
            lhv: Box::new(lhv),
            rhv: Box::new(rhv),
            pos,
        }),
    }
}

/// `X / Y` — division
///
/// Currently implemented:
///   Numb / Numb  → number division
///
fn eval_division(lhv: Value, rhv: Value, pos: usize) -> Value {
    match (lhv, rhv) {

        (Value::Numb(a), Value::Numb(b)) => Value::Numb(a / b),

        (lhv, rhv) => Value::Undefined(Undef::BinaryOperation {
            op:  "division",
            lhv: Box::new(lhv),
            rhv: Box::new(rhv),
            pos,
        }),
    }
}

/// `X % Y` — modulo
///
/// Currently implemented:
///   Numb % Numb  → number modulo
///
fn eval_modulo(lhv: Value, rhv: Value, pos: usize) -> Value {
    match (lhv, rhv) {

        (Value::Numb(a), Value::Numb(b)) => Value::Numb(a % b),

        (lhv, rhv) => Value::Undefined(Undef::BinaryOperation {
            op:  "modulo",
            lhv: Box::new(lhv),
            rhv: Box::new(rhv),
            pos,
        }),
    }
}

/// `X ^ Y` — power
///
/// Currently implemented:
///   Numb ^ Numb  → number exponentiation
///
fn eval_power(lhv: Value, rhv: Value, pos: usize) -> Value {
    match (lhv, rhv) {

        (Value::Numb(a), Value::Numb(b)) => Value::Numb(a.powf(b)),

        (lhv, rhv) => Value::Undefined(Undef::BinaryOperation {
            op:  "power",
            lhv: Box::new(lhv),
            rhv: Box::new(rhv),
            pos,
        }),
    }
}





// ----------------------------------------------------------------------------
//  Comparison operations
// ----------------------------------------------------------------------------

/// `X == Y` — equal-to operation 
fn eval_equal_to(lhv: Value, rhv: Value) -> Value {
    Value::Bool(lhv == rhv)
}

/// `X != Y` — not-equal-to operation
fn eval_not_equal_to(lhv: Value, rhv: Value) -> Value {
    Value::Bool(lhv != rhv)
}

/// `X > Y` — greater-than operation
fn eval_greater_than(lhv: Value, rhv: Value, pos: usize) -> Value {
    if let Some(cmp) = lhv.partial_cmp(&rhv) {
        Value::Bool(matches!(cmp, Ordering::Greater))
    } else {
        Value::Undefined(Undef::BinaryOperation{
            op:  "order",
            lhv: Box::new(lhv),
            rhv: Box::new(rhv),
            pos: pos,
        })
    }
}

/// `X < Y` — less-than operation
fn eval_less_than(lhv: Value, rhv: Value, pos: usize) -> Value {
    if let Some(cmp) = lhv.partial_cmp(&rhv) {
        Value::Bool(matches!(cmp, Ordering::Less))
    } else {
        Value::Undefined(Undef::BinaryOperation{
            op:  "order",
            lhv: Box::new(lhv),
            rhv: Box::new(rhv),
            pos: pos,
        })
    }
}

/// `X >= Y` — greater-than-or-equal-to operation
fn eval_greater_than_or_equal_to(lhv: Value, rhv: Value, pos: usize) -> Value {

    // >= is evaluated as !<
    match eval_less_than(lhv, rhv, pos) {
        Value::Bool(b) => Value::Bool(!b),
        undef => undef,
    }
}

/// `X <= Y` — less-than-or-equal-to operation
fn eval_less_than_or_equal_to(lhv: Value, rhv: Value, pos: usize) -> Value {

    // <= is evaluated as !>
    match eval_greater_than(lhv, rhv, pos) {
        Value::Bool(b) => Value::Bool(!b),
        undef => undef,
    }
}





// ----------------------------------------------------------------------------
//  Function composition operations
// ----------------------------------------------------------------------------

/// `f << g` - function composition
fn eval_composition(f: Value, g: Value, pos: usize) -> Value {
    Value::Builtin(BuiltinFn::new("composed", move |x:Value| {
        let y = eval_apply(g.clone(), x, pos);    // y = g(x)
        eval_apply(f.clone(), y, pos)             // z = f(y)
    }))
}



// ----------------------------------------------------------------------------
//  Tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::assert_matches;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn evaluate(src: &str) -> Value {
        let mut parser = Parser::new(Lexer::new(src));
        let ast = parser.parse();
        let scope = Scope::new();
        scope.set("TRUE", Value::Bool(true));
        scope.set("FALSE", Value::Bool(false));
        eval(&ast, &scope)
    }

    fn evaluate_bool(src: &str) -> bool {
        match evaluate(src) {
            Value::Bool(b) => b,
            _ => panic!("Boolean epression expected, found {}", src),
        }
    }

    mod literals {
        use super::*;

        #[test]
        fn number() {
            assert_eq!(evaluate("7.2"), Value::Numb(7.2));
        }

        #[test]
        fn string() {
            assert_eq!(evaluate("\"abc\""), Value::Text("abc".into()));
        }

        #[test]
        fn nothing() {

            assert_eq!(evaluate(""), Value::Nothing);
            assert_eq!(evaluate("   "), Value::Nothing);
            assert_eq!(evaluate("# comment\n"), Value::Nothing);
            assert_eq!(evaluate("  # comment\n  "), Value::Nothing);

            assert_eq!(evaluate("()"), Value::Nothing);
            assert_eq!(evaluate("(   )"), Value::Nothing);
            assert_eq!(evaluate("(\n# comment\n)"), Value::Nothing);
            assert_eq!(evaluate("(())"), Value::Nothing);
        }

        #[test]
        fn tuple() {
            assert_eq!(evaluate("7 , 2 , 9"), Value::Tuple(Rc::new(vec![
                        Value::Numb(7.0), 
                        Value::Numb(2.0),
                        Value::Numb(9.0)
            ])));

            assert_eq!(evaluate("7 , (2,9)"), Value::Tuple(Rc::new(vec![
                        Value::Numb(7.0), 
                        Value::Numb(2.0),
                        Value::Numb(9.0)
            ])));

            assert_eq!(evaluate("(7,2) , 9"), Value::Tuple(Rc::new(vec![
                        Value::Numb(7.0), 
                        Value::Numb(2.0),
                        Value::Numb(9.0)
            ])));

            assert_eq!(evaluate("(7,2) , (9,3)"), Value::Tuple(Rc::new(vec![
                        Value::Numb(7.0), 
                        Value::Numb(2.0),
                        Value::Numb(9.0),
                        Value::Numb(3.0)
            ])));

            assert_eq!(evaluate("7 , 2 , () , 9"), Value::Tuple(Rc::new(vec![
                        Value::Numb(7.0), 
                        Value::Numb(2.0),
                        Value::Numb(9.0)
            ])));

            assert_eq!(evaluate("() , ()"), Value::Nothing);
            assert_eq!(evaluate("1 , ()"), Value::Numb(1.0));
            assert_eq!(evaluate("() , 1"), Value::Numb(1.0));
        }
    
        #[test]
        fn list() {
            assert_eq!(evaluate("[7 , 2 , 9]"), Value::List(Rc::new(vec![
                        Value::Numb(7.0), 
                        Value::Numb(2.0),
                        Value::Numb(9.0)
            ])));

            assert_eq!(evaluate("[7 , [2,9]]"), Value::List(Rc::new(vec![
                        Value::Numb(7.0), 
                        Value::List(Rc::new(vec![
                            Value::Numb(2.0),
                            Value::Numb(9.0)
                        ]))
            ])));

            assert_eq!(evaluate("[]"), Value::List(Rc::new(vec![])));
        }

        #[test]
        fn namespace() {
            match evaluate("{x:10, y:20}") {
                Value::Namespace(scope) => {
                    assert_eq!(scope.get("x"), Value::Numb(10.0));
                    assert_eq!(scope.get("y"), Value::Numb(20.0));
                    
                    // Inherited from parent scope
                    assert_eq!(scope.get("TRUE"), Value::Bool(true));
                }
                other => panic!("expected Namespace, got {:?}", other),
            }
        }
    }

    mod core_operations {
        use super::*;

        #[test]
        fn function_definition() {

            assert_matches!(evaluate("(x, y) -> x+y"), Value::Func{params:_, body:_, closure:_});

            // Invalid left-hand names.
            assert_eq!(evaluate("(x, 12) -> 2*x"), Value::Undefined(Undef::BinaryOperation{
                op: "function_definition",
                lhv: Box::new(Value::Undefined(Undef::Name{pos:4})),
                rhv: Box::new(Value::Nothing),
                pos: 8,
            }));
        }

        #[test]
        fn function_application() {
            assert_eq!(evaluate("f = (a) -> 2*a , f 5"), Value::Numb(10.0));
            assert_eq!(evaluate("f = (a,b) -> a+b , f(3,7)"), Value::Numb(10.0));
            assert_eq!(evaluate("f = (a,b,c) -> (a,b)*c , f(2,3,4,5)"), Value::Tuple(Rc::new(vec![
                Value::Numb(8.0),
                Value::Numb(15.0),
            ])));
            assert_eq!(
                evaluate("(x->2*x , x->3*x) 4"), 
                Value::Tuple(Rc::new(vec![Value::Numb(8.0), Value::Numb(12.0)]))
            );
        }

        #[test]
        fn list_application() {
            assert_eq!(evaluate("[10, 20, 30] 1"), Value::Numb(20.0));
            
            // Out-of-bounds returns Value::Nothing
            assert_eq!(evaluate("[10, 20] 5"), Value::Nothing);
            
            // Non-number argument returns Undefined(Undef::BinaryOperation)
            assert!(matches!(
                evaluate(r#"[10, 20] "0""#),
                Value::Undefined(Undef::BinaryOperation { op: "application", .. })
            ));

            // Applying list to a Tuple
            assert_eq!(
                evaluate("[10, 20, 30] (0, 5, 2)"),
                Value::Tuple(Rc::new(vec![Value::Numb(10.0), Value::Nothing, Value::Numb(30.0)]))
            );
            assert_eq!(
                evaluate("([1,2,3] , [10,20,30]) 1"), 
                Value::Tuple(Rc::new(vec![Value::Numb(2.0), Value::Numb(20.0)]))
            );
        }

        #[test]
        fn namespace_application() {
            assert_eq!(evaluate(r#"{x: 10, y: 20} "x""#), Value::Numb(10.0));
            
            // Missing key returns Value::Nothing
            assert_eq!(evaluate(r#"{x: 10} "z""#), Value::Nothing);

            // Non-text key returns Undefined(Undef::BinaryOperation)
            assert!(matches!(
                evaluate("{x: 10} 1"),
                Value::Undefined(Undef::BinaryOperation { op: "application", .. })
            ));

            // Applying namespace to a Tuple
            assert_eq!(
                evaluate(r#"{x: 10, y: 20} ("x", "missing")"#),
                Value::Tuple(Rc::new(vec![Value::Numb(10.0), Value::Nothing]))
            );

            assert_eq!(
                evaluate("({x:1, y:2} , {x:10, y:20}) \"y\""), 
                Value::Tuple(Rc::new(vec![Value::Numb(2.0), Value::Numb(20.0)]))
            );
        }

        #[test]
        fn assignment() {
            // `x = 5` evaluates to Nothing; the binding is only visible through
            // a subsequent lookup. Because `=` binds tighter than `,`, this
            // parses as `(x = 5), x`, letting us observe the stored value.
            assert_eq!(evaluate("x = 5, x"), Value::Numb(5.0));

            // Assignment itself always yields Nothing.
            assert_eq!(evaluate("x = 5"), Value::Nothing);

            // Destructuring assignment: `(a, b) = (1, 2)` binds a=1, b=2.
            assert_eq!(
                evaluate("(a, b) = (1, 2), a, b"),
                Value::Tuple(Rc::new(vec![Value::Numb(1.0), Value::Numb(2.0)]))
            );

            // Re-assigning an existing name overwrites the previous value.
            assert_eq!(evaluate("x = 1, x = 2, x"), Value::Numb(2.0));

            // Invalid left-hand names.
            assert_eq!(evaluate("(x, 12) = (1, 2)"), Value::Undefined(Undef::BinaryOperation{
                op: "assignment",
                lhv: Box::new(Value::Undefined(Undef::Name{pos:4})),
                rhv: Box::new(Value::Nothing),
                pos: 8,
            }));
        }

        #[test]
        fn labelling() {
            // `x : 5` evaluates to the assigned value itself (unlike `=`),
            // *and* binds `x` in scope. Parses as `(x : 5), x`.
            assert_eq!(evaluate("x : 5"), Value::Numb(5.0));

            assert_eq!(
                evaluate("x : 5, x"),
                Value::Tuple(Rc::new(vec![Value::Numb(5.0), Value::Numb(5.0)]))
            );

            // Destructuring labelling behaves like destructuring assignment,
            // but also yields the assigned values as the LabellingOp's result.
            assert_eq!(
                evaluate("(a, b) : (1, 2)"),
                Value::Tuple(Rc::new(vec![Value::Numb(1.0), Value::Numb(2.0)]))
            );

            // Invalid left-hand names.
            assert_eq!(evaluate("(x, 12) : (1, 2)"), Value::Undefined(Undef::BinaryOperation{
                op: "labelling",
                lhv: Box::new(Value::Undefined(Undef::Name{pos:4})),
                rhv: Box::new(Value::Nothing),
                pos: 8,
            }));
        }

        #[test]
        fn subcontexting() {
            assert_eq!(evaluate("{x:10}.x"), Value::Numb(10.0));
            assert_eq!(evaluate("{x:10, y:20}.(x+y)"), Value::Numb(30.0));
            assert_eq!(evaluate("{x:10, y:20}.TRUE"), Value::Bool(true));
            match evaluate("x=10, x.TRUE") {
                Value::Undefined(Undef::BinaryOperation { op, lhv, rhv, pos }) => {
                    assert_eq!(op, "subcontexting");
                    assert_eq!(*lhv, Value::Numb(10.0));
                    assert_eq!(*rhv, Value::Nothing);
                    assert_eq!(pos, 7);
                }
                other => panic!("expected Undefined::BinaryOperation, got {:?}", other),
            }
        }

        #[test]
        fn mapping() {
            assert_eq!(evaluate("f = a -> 2*a , (2,3,4) => f"), Value::Tuple(Rc::new(vec![
                Value::Numb(4.0),
                Value::Numb(6.0),
                Value::Numb(8.0),
            ])));
            assert_eq!(evaluate("f = a -> 2*a , 2 => f"), Value::Numb(4.0));
            assert_eq!(evaluate("f = a -> 2*a , () => f"), Value::Nothing);
            assert_eq!(evaluate("f = a -> () , (1,2,3) => f"), Value::Nothing);
        }
    }

    mod unary_operations {
        use super::*;

        #[test]
        fn affirm() {
            // number
            assert_eq!(evaluate("+2"), Value::Numb(2.0));

            // +X should be a pure identity — it must NOT return Undef for
            // non-numeric types (was the bug described in review item #10).
            let scope = Scope::new();
            let text_val = Value::Text("hello".into());

            // Build a tiny AST manually: AffirmOp wrapping a Text literal.
            let text_leaf = crate::ast::Node::Text(crate::ast::Leaf { pos: 0, val: "hello".into() });
            let affirm_node = crate::ast::Node::AffirmOp(crate::ast::UnaryOp {
                pos: 0,
                opd: Box::new(text_leaf),
            });
            assert_eq!(eval(&affirm_node, &scope), text_val);
        }

        #[test]
        fn negate() {
            assert_eq!(evaluate("-2"), Value::Numb(-2.0));
            assert_eq!(evaluate("-(2, 3, 4)"), Value::Tuple(Rc::new(vec![
                Value::Numb(-2.0),
                Value::Numb(-3.0),
                Value::Numb(-4.0)
            ])));
        }

        #[test]
        fn logical_negation() {
            assert_eq!(evaluate("!TRUE"), Value::Bool(false));
            assert_eq!(evaluate("!FALSE"), Value::Bool(true));
            assert_eq!(evaluate("!1"), Value::Bool(false));
            assert_eq!(evaluate("!0"), Value::Bool(true));
            assert_eq!(evaluate("!!0"), Value::Bool(false));
            assert_eq!(evaluate("!!1"), Value::Bool(true));
        }

        #[test]
        fn lifting() {
            assert_eq!(evaluate("{x=1}.{x=2}.x"), Value::Numb(2.0));
            assert_eq!(evaluate("{x=1}.{x=2}.$x"), Value::Numb(1.0));
        }
    }

    mod arithmetic_operations {
        use super::*;

        #[test]
        fn sum() {
            assert_eq!(evaluate("() + ()"), Value::Nothing);

            assert_eq!(evaluate("1 + 2"), Value::Numb(3.0));

            assert_eq!(evaluate("TRUE + TRUE"), Value::Bool(true));
            assert_eq!(evaluate("TRUE + FALSE"), Value::Bool(true));
            assert_eq!(evaluate("FALSE + TRUE"), Value::Bool(true));
            assert_eq!(evaluate("FALSE + FALSE"), Value::Bool(false));

            assert_eq!(evaluate("\"abc\" + \"def\""), Value::Text("abcdef".into()));

            assert_eq!(evaluate("[1,2,3] + [4,5,6]"), Value::List(Rc::new(vec![
                        Value::Numb(1.0),
                        Value::Numb(2.0),
                        Value::Numb(3.0),
                        Value::Numb(4.0),
                        Value::Numb(5.0),
                        Value::Numb(6.0)
                    ])));

            assert_eq!(evaluate("(1, 2) + (3, 4)"), Value::Tuple(Rc::new(vec![
                        Value::Numb(4.0),
                        Value::Numb(6.0),
                    ])));

            match evaluate("1 + \"abc\"") {
                Value::Undefined(Undef::BinaryOperation { op, lhv, rhv, pos }) => {
                    assert_eq!(op, "sum");
                    assert_eq!(*lhv, Value::Numb(1.0));
                    assert_eq!(*rhv, Value::Text("abc".into()));
                    assert_eq!(pos, 2);
                }
                other => panic!("expected Undefined::BinaryOperation, got {:?}", other),
            }
        }

        #[test]
        fn subtraction() {
            assert_eq!(evaluate("() - ()"), Value::Nothing);

            assert_eq!(evaluate("7 - 2"), Value::Numb(5.0));

            assert_eq!(evaluate("(10, 20) - (1, 2)"), Value::Tuple(Rc::new(vec![
                        Value::Numb(9.0),
                        Value::Numb(18.0),
                    ])));

            match evaluate("1 - \"abc\"") {
                Value::Undefined(Undef::BinaryOperation { op, lhv, rhv, pos }) => {
                    assert_eq!(op, "subtraction");
                    assert_eq!(*lhv, Value::Numb(1.0));
                    assert_eq!(*rhv, Value::Text("abc".into()));
                    assert_eq!(pos, 2);
                }
                other => panic!("expected Undefined::BinaryOperation, got {:?}", other),
            }
        }

        #[test]
        fn product() {
            assert_eq!(evaluate("() * ()"), Value::Nothing);

            assert_eq!(evaluate("7 * 2"), Value::Numb(14.0));

            assert_eq!(evaluate("TRUE * TRUE"), Value::Bool(true));
            assert_eq!(evaluate("TRUE * FALSE"), Value::Bool(false));
            assert_eq!(evaluate("FALSE * TRUE"), Value::Bool(false));
            assert_eq!(evaluate("FALSE * FALSE"), Value::Bool(false));

            assert_eq!(evaluate("(2, 3) * (4, 5)"), Value::Tuple(Rc::new(vec![
                        Value::Numb(8.0),
                        Value::Numb(15.0),
                    ])));

            match evaluate("1 * \"abc\"") {
                Value::Undefined(Undef::BinaryOperation { op, lhv, rhv, pos }) => {
                    assert_eq!(op, "product");
                    assert_eq!(*lhv, Value::Numb(1.0));
                    assert_eq!(*rhv, Value::Text("abc".into()));
                    assert_eq!(pos, 2);
                }
                other => panic!("expected Undefined::BinaryOperation, got {:?}", other),
            }
        }

        #[test]
        fn division() {
            assert_eq!(evaluate("() / ()"), Value::Nothing);

            assert_eq!(evaluate("7 / 2"), Value::Numb(3.5));
            assert_eq!(evaluate("7 / 0"), Value::Numb(f64::INFINITY));

            assert_eq!(evaluate("(10, 20) / (2, 5)"), Value::Tuple(Rc::new(vec![
                        Value::Numb(5.0),
                        Value::Numb(4.0),
                    ])));

            match evaluate("1 / \"abc\"") {
                Value::Undefined(Undef::BinaryOperation { op, lhv, rhv, pos }) => {
                    assert_eq!(op, "division");
                    assert_eq!(*lhv, Value::Numb(1.0));
                    assert_eq!(*rhv, Value::Text("abc".into()));
                    assert_eq!(pos, 2);
                }
                other => panic!("expected Undefined::BinaryOperation, got {:?}", other),
            }
        }

        #[test]
        fn modulo() {
            assert_eq!(evaluate("() % ()"), Value::Nothing);

            assert_eq!(evaluate("8 % 3"), Value::Numb(2.0));

            assert_eq!(evaluate("(10, 20) % (3, 7)"), Value::Tuple(Rc::new(vec![
                        Value::Numb(1.0),
                        Value::Numb(6.0),
                    ])));

            match evaluate("1 % \"abc\"") {
                Value::Undefined(Undef::BinaryOperation { op, lhv, rhv, pos }) => {
                    assert_eq!(op, "modulo");
                    assert_eq!(*lhv, Value::Numb(1.0));
                    assert_eq!(*rhv, Value::Text("abc".into()));
                    assert_eq!(pos, 2);
                }
                other => panic!("expected Undefined::BinaryOperation, got {:?}", other),
            }
        }

        #[test]
        fn power() {
            assert_eq!(evaluate("() ^ ()"), Value::Nothing);

            assert_eq!(evaluate("2 ^ 3"), Value::Numb(8.0));

            assert_eq!(evaluate("(10, 2) ^ (2, 4)"), Value::Tuple(Rc::new(vec![
                        Value::Numb(100.0),
                        Value::Numb(16.0),
                    ])));

            match evaluate("1 ^ \"abc\"") {
                Value::Undefined(Undef::BinaryOperation { op, lhv, rhv, pos }) => {
                    assert_eq!(op, "power");
                    assert_eq!(*lhv, Value::Numb(1.0));
                    assert_eq!(*rhv, Value::Text("abc".into()));
                    assert_eq!(pos, 2);
                }
                other => panic!("expected Undefined::BinaryOperation, got {:?}", other),
            }
        }
    }

    mod comparison_operations {
        use super::*;

        #[test]
        fn equal_to() {

            // bool
            assert_eq!(evaluate_bool("TRUE == TRUE")  , true);
            assert_eq!(evaluate_bool("FALSE == FALSE"), true);
            assert_eq!(evaluate_bool("TRUE == FALSE") , false);

            // numb
            assert_eq!(evaluate_bool("1 == 1"), true);
            assert_eq!(evaluate_bool("1 == 2"), false);

            // text
            assert_eq!(evaluate_bool("\"abc\" == \"abc\""), true);
            assert_eq!(evaluate_bool("\"abc\" == \"xy\"") , false);

            // list
            assert_eq!(evaluate_bool("[1,2,3] == [1,2,3]"), true);
            assert_eq!(evaluate_bool("[1,2,3] == [1,2]")  , false);

            // namespace
            assert_eq!(evaluate_bool("{x:1,y:2} == {x:1,y:2}"), true);
            assert_eq!(evaluate_bool("{x:1,y:2} == {x:1,z:2}"), false);

            // tuple
            assert_eq!(evaluate_bool("(1,2,3) == (1,2,3)"), true);
            assert_eq!(evaluate_bool("(1,2,3) == (1,2)")  , false);

            // nothing
            assert_eq!(evaluate_bool("() == ()"), true);
            assert_eq!(evaluate_bool("() == FALSE  "), false);
            assert_eq!(evaluate_bool("() == 1      "), false);
            assert_eq!(evaluate_bool("() == \"abc\""), false);
            assert_eq!(evaluate_bool("() == [1,2,3]"), false);
            assert_eq!(evaluate_bool("() == {x: 10}"), false);

            // undefined
            assert_eq!(evaluate_bool("(-[]) == (-[])"), true);
            assert_eq!(evaluate_bool("(-[]) == (-{})"), false);
            assert_eq!(evaluate_bool("([]^2) == ([]^2)"), true);
            assert_eq!(evaluate_bool("([]^2) == ({}^2)"), false);
            assert_eq!(evaluate_bool("(@) == (@)"), true);
            assert_eq!(evaluate_bool("(~) == (@)"), false);
            assert_eq!(evaluate("(1,2"), evaluate("(1,2 "));
        }

        #[test]
        fn not_equal_to() {

            // bool
            assert_eq!(evaluate_bool("TRUE != TRUE"), false);
            assert_eq!(evaluate_bool("FALSE != FALSE"), false);
            assert_eq!(evaluate_bool("TRUE != FALSE"), true);

            // numb
            assert_eq!(evaluate_bool("1 != 1"), false);
            assert_eq!(evaluate_bool("1 != 2"), true);

            // text
            assert_eq!(evaluate_bool("\"abc\" != \"abc\""), false);
            assert_eq!(evaluate_bool("\"abc\" != \"xy\"") , true);

            // list
            assert_eq!(evaluate_bool("[1,2,3] != [1,2,3]"), false);
            assert_eq!(evaluate_bool("[1,2,3] != [1,2]")  , true);

            // namespace
            assert_eq!(evaluate_bool("{x:1,y:2} != {x:1,y:2}"), false);
            assert_eq!(evaluate_bool("{x:1,y:2} != {x:1,z:2}"), true);

            // tuple
            assert_eq!(evaluate_bool("(1,2,3) != (1,2,3)"), false);
            assert_eq!(evaluate_bool("(1,2,3) != (1,2)")  , true);

            // nothing
            assert_eq!(evaluate_bool("() != ()"), false);
            assert_eq!(evaluate_bool("() != FALSE  "), true);
            assert_eq!(evaluate_bool("() != 1      "), true);
            assert_eq!(evaluate_bool("() != \"abc\""), true);
            assert_eq!(evaluate_bool("() != [1,2,3]"), true);
            assert_eq!(evaluate_bool("() != {x: 10}"), true);

            // undefined
            assert_eq!(evaluate_bool("(-[]) != (-[])"), false);
            assert_eq!(evaluate_bool("(-[]) != (-{})"), true);
            assert_eq!(evaluate_bool("([]^2) != ([]^2)"), false);
            assert_eq!(evaluate_bool("([]^2) != ({}^2)"), true);
            assert_eq!(evaluate_bool("(@) != (@)"), false);
            assert_eq!(evaluate_bool("(~) != (@)"), true);
            assert_ne!(evaluate("(1,2"), evaluate("[1,2"));
        }

        #[test]
        fn greater_than() {

            // bool
            assert_eq!(evaluate_bool("TRUE > TRUE  "), false);
            assert_eq!(evaluate_bool("FALSE > FALSE"), false);
            assert_eq!(evaluate_bool("TRUE > FALSE "), true);
            assert_eq!(evaluate_bool("FALSE > TRUE "), false);

            // numb
            assert_eq!(evaluate_bool("1 > 1"), false);
            assert_eq!(evaluate_bool("1 > 2"), false);
            assert_eq!(evaluate_bool("2 > 1"), true);
            assert_eq!(evaluate_bool("2 > 2"), false);

            // text 
            assert_eq!(evaluate_bool("\"abc\" > \"abc\""), false);
            assert_eq!(evaluate_bool("\"abc\" > \"xyz\""), false);
            assert_eq!(evaluate_bool("\"xyz\" > \"abc\""), true);

            // list
            assert_eq!(evaluate_bool("[1,2,3] > [1,2,3]"), false);
            assert_eq!(evaluate_bool("[1,2,3] > [1,3,4]"), false);
            assert_eq!(evaluate_bool("[1,3,4] > [1,2,3]"), true);
            assert_eq!(evaluate_bool("[1,2  ] > [1,2,3]"), false);
            assert_eq!(evaluate_bool("[1,2,3] > [1,2  ]"), true);
            
            // TODO: namespace
            
            // tuple
            assert_eq!(evaluate_bool("(1,2,3) > (1,2,3)"), false);
            assert_eq!(evaluate_bool("(1,2,3) > (1,3,4)"), false);
            assert_eq!(evaluate_bool("(1,3,4) > (1,2,3)"), true);
            assert_eq!(evaluate_bool("(1,2  ) > (1,2,3)"), false);
            assert_eq!(evaluate_bool("(1,2,3) > (1,2  )"), true);

            // nothing
            assert_eq!(evaluate_bool("() > ()"), false);
            assert_eq!(evaluate_bool("() > FALSE  "), false);
            assert_eq!(evaluate_bool("() > 1      "), false);
            assert_eq!(evaluate_bool("() > \"abc\""), false);
            assert_eq!(evaluate_bool("() > [1,2,3]"), false);
            assert_eq!(evaluate_bool("() > {x: 10}"), false);
            assert_eq!(evaluate_bool("FALSE   > ()"), true);
            assert_eq!(evaluate_bool("1       > ()"), true);
            assert_eq!(evaluate_bool("\"abc\" > ()"), true);
            assert_eq!(evaluate_bool("[1,2,3] > ()"), true);
            assert_eq!(evaluate_bool("{x: 10} > ()"), true);

            // undefined order
            assert_eq!(evaluate("TRUE > 1"), Value::Undefined(Undef::BinaryOperation{
                op : "order", 
                lhv: Box::new(Value::Bool(true)), 
                rhv: Box::new(Value::Numb(1.0)), 
                pos: 5
            }));
        }

        #[test]
        fn less_than() {

            // bool
            assert_eq!(evaluate_bool("TRUE < TRUE  "), false);
            assert_eq!(evaluate_bool("FALSE < FALSE"), false);
            assert_eq!(evaluate_bool("TRUE < FALSE "), false);
            assert_eq!(evaluate_bool("FALSE < TRUE "), true);

            // numb
            assert_eq!(evaluate_bool("1 < 1"), false);
            assert_eq!(evaluate_bool("1 < 2"), true);
            assert_eq!(evaluate_bool("2 < 1"), false);
            assert_eq!(evaluate_bool("2 < 2"), false);

            // text 
            assert_eq!(evaluate_bool("\"abc\" < \"abc\""), false);
            assert_eq!(evaluate_bool("\"abc\" < \"xyz\""), true);
            assert_eq!(evaluate_bool("\"xyz\" < \"abc\""), false);

            // list
            assert_eq!(evaluate_bool("[1,2,3] < [1,2,3]"), false);
            assert_eq!(evaluate_bool("[1,2,3] < [1,3,4]"), true);
            assert_eq!(evaluate_bool("[1,3,4] < [1,2,3]"), false);
            assert_eq!(evaluate_bool("[1,2  ] < [1,2,3]"), true);
            assert_eq!(evaluate_bool("[1,2,3] < [1,2  ]"), false);
            
            // TODO: namespace
            
            // tuple
            assert_eq!(evaluate_bool("(1,2,3) < (1,2,3)"), false);
            assert_eq!(evaluate_bool("(1,2,3) < (1,3,4)"), true);
            assert_eq!(evaluate_bool("(1,3,4) < (1,2,3)"), false);
            assert_eq!(evaluate_bool("(1,2  ) < (1,2,3)"), true);
            assert_eq!(evaluate_bool("(1,2,3) < (1,2  )"), false);

            // nothing
            assert_eq!(evaluate_bool("() < ()"), false);
            assert_eq!(evaluate_bool("() < FALSE  "), true);
            assert_eq!(evaluate_bool("() < 1      "), true);
            assert_eq!(evaluate_bool("() < \"abc\""), true);
            assert_eq!(evaluate_bool("() < [1,2,3]"), true);
            assert_eq!(evaluate_bool("() < {x: 10}"), true);
            assert_eq!(evaluate_bool("FALSE   < ()"), false);
            assert_eq!(evaluate_bool("1       < ()"), false);
            assert_eq!(evaluate_bool("\"abc\" < ()"), false);
            assert_eq!(evaluate_bool("[1,2,3] < ()"), false);
            assert_eq!(evaluate_bool("{x: 10} < ()"), false);

            // undefined order
            assert_eq!(evaluate("TRUE < 1"), Value::Undefined(Undef::BinaryOperation{
                op : "order", 
                lhv: Box::new(Value::Bool(true)), 
                rhv: Box::new(Value::Numb(1.0)), 
                pos: 5
            }));
        }

        #[test]
        fn greater_than_or_equal_to() {

            // bool
            assert_eq!(evaluate_bool("TRUE >= TRUE  "), true);
            assert_eq!(evaluate_bool("FALSE >= FALSE"), true);
            assert_eq!(evaluate_bool("TRUE >= FALSE "), true);
            assert_eq!(evaluate_bool("FALSE >= TRUE "), false);

            // numb
            assert_eq!(evaluate_bool("1 >= 1"), true);
            assert_eq!(evaluate_bool("1 >= 2"), false);
            assert_eq!(evaluate_bool("2 >= 1"), true);
            assert_eq!(evaluate_bool("2 >= 2"), true);

            // text 
            assert_eq!(evaluate_bool("\"abc\" >= \"abc\""), true);
            assert_eq!(evaluate_bool("\"abc\" >= \"xyz\""), false);
            assert_eq!(evaluate_bool("\"xyz\" >= \"abc\""), true);

            // list
            assert_eq!(evaluate_bool("[1,2,3] >= [1,2,3]"), true);
            assert_eq!(evaluate_bool("[1,2,3] >= [1,3,4]"), false);
            assert_eq!(evaluate_bool("[1,3,4] >= [1,2,3]"), true);
            assert_eq!(evaluate_bool("[1,2  ] >= [1,2,3]"), false);
            assert_eq!(evaluate_bool("[1,2,3] >= [1,2  ]"), true);
            
            // TODO: namespace
            
            // tuple
            assert_eq!(evaluate_bool("(1,2,3) >= (1,2,3)"), true);
            assert_eq!(evaluate_bool("(1,2,3) >= (1,3,4)"), false);
            assert_eq!(evaluate_bool("(1,3,4) >= (1,2,3)"), true);
            assert_eq!(evaluate_bool("(1,2  ) >= (1,2,3)"), false);
            assert_eq!(evaluate_bool("(1,2,3) >= (1,2  )"), true);

            // nothing
            assert_eq!(evaluate_bool("() >= ()"), true);
            assert_eq!(evaluate_bool("() >= FALSE  "), false);
            assert_eq!(evaluate_bool("() >= 1      "), false);
            assert_eq!(evaluate_bool("() >= \"abc\""), false);
            assert_eq!(evaluate_bool("() >= [1,2,3]"), false);
            assert_eq!(evaluate_bool("() >= {x: 10}"), false);
            assert_eq!(evaluate_bool("FALSE   >= ()"), true);
            assert_eq!(evaluate_bool("1       >= ()"), true);
            assert_eq!(evaluate_bool("\"abc\" >= ()"), true);
            assert_eq!(evaluate_bool("[1,2,3] >= ()"), true);
            assert_eq!(evaluate_bool("{x: 10} >= ()"), true);

            // undefined order
            assert_eq!(evaluate("TRUE >= 1"), Value::Undefined(Undef::BinaryOperation{
                op : "order", 
                lhv: Box::new(Value::Bool(true)), 
                rhv: Box::new(Value::Numb(1.0)), 
                pos: 5
            }));
        }

        #[test]
        fn less_than_or_equal_to() {

            // bool
            assert_eq!(evaluate_bool("TRUE <= TRUE  "), true);
            assert_eq!(evaluate_bool("FALSE <= FALSE"), true);
            assert_eq!(evaluate_bool("TRUE <= FALSE "), false);
            assert_eq!(evaluate_bool("FALSE <= TRUE "), true);

            // numb
            assert_eq!(evaluate_bool("1 <= 1"), true);
            assert_eq!(evaluate_bool("1 <= 2"), true);
            assert_eq!(evaluate_bool("2 <= 1"), false);
            assert_eq!(evaluate_bool("2 <= 2"), true);

            // text 
            assert_eq!(evaluate_bool("\"abc\" <= \"abc\""), true);
            assert_eq!(evaluate_bool("\"abc\" <= \"xyz\""), true);
            assert_eq!(evaluate_bool("\"xyz\" <= \"abc\""), false);

            // list
            assert_eq!(evaluate_bool("[1,2,3] <= [1,2,3]"), true);
            assert_eq!(evaluate_bool("[1,2,3] <= [1,3,4]"), true);
            assert_eq!(evaluate_bool("[1,3,4] <= [1,2,3]"), false);
            assert_eq!(evaluate_bool("[1,2  ] <= [1,2,3]"), true);
            assert_eq!(evaluate_bool("[1,2,3] <= [1,2  ]"), false);
            
            // TODO: namespace
            
            // tuple
            assert_eq!(evaluate_bool("(1,2,3) <= (1,2,3)"), true);
            assert_eq!(evaluate_bool("(1,2,3) <= (1,3,4)"), true);
            assert_eq!(evaluate_bool("(1,3,4) <= (1,2,3)"), false);
            assert_eq!(evaluate_bool("(1,2  ) <= (1,2,3)"), true);
            assert_eq!(evaluate_bool("(1,2,3) <= (1,2  )"), false);

            // nothing
            assert_eq!(evaluate_bool("() <= ()"), true);
            assert_eq!(evaluate_bool("() <= FALSE  "), true);
            assert_eq!(evaluate_bool("() <= 1      "), true);
            assert_eq!(evaluate_bool("() <= \"abc\""), true);
            assert_eq!(evaluate_bool("() <= [1,2,3]"), true);
            assert_eq!(evaluate_bool("() <= {x: 10}"), true);
            assert_eq!(evaluate_bool("FALSE   <= ()"), false);
            assert_eq!(evaluate_bool("1       <= ()"), false);
            assert_eq!(evaluate_bool("\"abc\" <= ()"), false);
            assert_eq!(evaluate_bool("[1,2,3] <= ()"), false);
            assert_eq!(evaluate_bool("{x: 10} <= ()"), false);

            // undefined order
            assert_eq!(evaluate("TRUE <= 1"), Value::Undefined(Undef::BinaryOperation{
                op : "order", 
                lhv: Box::new(Value::Bool(true)), 
                rhv: Box::new(Value::Numb(1.0)), 
                pos: 5
            }));
        }
    }

    mod selection_and_alternative_operations {
        use super::*;

        #[test]
        fn conditional() {
            // Left side casts to true -> returns right-hand side operand
            assert_eq!(evaluate("TRUE ? 10"), Value::Numb(10.0));
            assert_eq!(evaluate("1 ? \"yes\""), Value::Text("yes".into()));
            assert_eq!(evaluate("[1] ? 42"), Value::Numb(42.0));

            // Left side casts to false -> returns Nothing
            assert_eq!(evaluate("FALSE ? 10"), Value::Nothing);
            assert_eq!(evaluate("0 ? 10"), Value::Nothing);
            assert_eq!(evaluate("\"\" ? 10"), Value::Nothing);
            assert_eq!(evaluate("() ? 10"), Value::Nothing);

            // Tuples: truthy if at least one element converts to true
            assert_eq!(evaluate("(FALSE, 0, 1) ? \"has_true\""), Value::Text("has_true".into()));
            assert_eq!(evaluate("(FALSE, 0, \"\") ? \"all_false\""), Value::Nothing);
        }

        #[test]
        fn alternative() {
            assert_eq!(evaluate("() ; 2"), Value::Numb(2.0));
            assert_eq!(evaluate("1 ; 2"), Value::Numb(1.0));
            assert_eq!(evaluate("FALSE ; 2"), Value::Bool(false));
        }
    }

    mod logic_operations {
        use super::*;

        #[test]
        fn and_op() {
            // True & RHS -> returns RHS
            assert_eq!(evaluate("TRUE & 10"), Value::Numb(10.0));
            assert_eq!(evaluate("TRUE & FALSE"), Value::Bool(false));
            assert_eq!(evaluate("1 & \"hello\""), Value::Text("hello".into()));
            assert_eq!(evaluate("(FALSE, 1) & 42"), Value::Numb(42.0));

            // False & RHS -> short-circuits and returns LHS
            assert_eq!(evaluate("FALSE & 10"), Value::Bool(false));
            assert_eq!(evaluate("0 & 10"), Value::Numb(0.0));
            assert_eq!(evaluate("\"\" & 10"), Value::Text("".into()));
            assert_eq!(evaluate("() & 10"), Value::Nothing);
            assert_eq!(evaluate("(FALSE, 0) & 42"), Value::Tuple(Rc::new(vec![
                Value::Bool(false),
                Value::Numb(0.0),
            ])));
        }

        #[test]
        fn or_op() {
            // True | RHS -> short-circuits and returns LHS
            assert_eq!(evaluate("TRUE | 10"), Value::Bool(true));
            assert_eq!(evaluate("1 | 10"), Value::Numb(1.0));
            assert_eq!(evaluate("\"yes\" | 10"), Value::Text("yes".into()));
            assert_eq!(evaluate("(FALSE, 1) | 42"), Value::Tuple(Rc::new(vec![
                Value::Bool(false),
                Value::Numb(1.0),
            ])));

            // False | RHS -> returns RHS
            assert_eq!(evaluate("FALSE | 10"), Value::Numb(10.0));
            assert_eq!(evaluate("0 | \"fallback\""), Value::Text("fallback".into()));
            assert_eq!(evaluate("\"\" | TRUE"), Value::Bool(true));
            assert_eq!(evaluate("() | 5"), Value::Numb(5.0));
            assert_eq!(evaluate("(FALSE, 0) | 42"), Value::Numb(42.0));
        }
    }

    mod function_composition {
        use super::*;

        #[test]
        fn composition() {
            assert_eq!(evaluate("f = y -> 2*y , g = x -> x+1 , (f<<g)(3)"), Value::Numb(8.0));
        }

        #[test]
        fn reverse_composition() {
            assert_eq!(evaluate("f = y -> 2*y , g = x -> x+1 , (f>>g)(3)"), Value::Numb(7.0));
        }
    }

    mod precedence {
        use super::*;

        #[test]
        fn application_and_subcontexting() {
            assert_eq!(evaluate("ns = {f: x -> 2*x}, ns.f 3"), Value::Numb(6.0));
            assert_eq!(evaluate("ns = {a=10}, f = x -> 2*x, f ns.a"), Value::Numb(20.0));
            assert_eq!(evaluate("f = x -> {y:x}, (f 3).y"), Value::Numb(3.0));
        }
    }
}
````

## File: src/lexer.rs
````rust
use std::iter::Peekable;
use std::str::Chars;
use crate::token::Token;


/// A zero-copy lexer for Swan source text.
///
/// `Lexer` converts a source `&str` into a sequence of [`Token`]s.  Each
/// token that carries a string payload (`Identifier`, `Text`, `Comment`,
/// `Undefined`) borrows a slice directly from the original source, so no heap
/// allocations are needed during tokenisation.
///
/// # Usage
///
/// Call [`next_token`](Lexer::next_token) in a loop to pull tokens one at a
/// time, or use the [`Iterator`] implementation to drive the lexer with
/// standard iterator adapters.  The iterator stops (returns `None`) at
/// [`Token::EndOfExpression`]; `next_token` returns that sentinel instead.
///
/// ```
/// use swan_core::lexer::Lexer;
/// use swan_core::token::Token;
///
/// let tokens: Vec<_> = Lexer::new("x = 42").collect();
/// assert_eq!(tokens[0], (0, Token::Identifier("x")));
/// assert_eq!(tokens[1], (2, Token::AssignmentOp));
/// assert_eq!(tokens[2], (4, Token::Number(42.0)));
/// ```
///
/// # Position tracking
///
/// Every call to `next_token` returns a `(usize, Token)` pair where the
/// `usize` is the **byte offset** of the token's first character inside the
/// source string.  Positions are byte-based (not char-based) and are safe to
/// use as indices into the original `&str`.

pub struct Lexer<'a> {
    /// The full source string — never modified; sliced into by token variants.
    source: &'a str,
    /// Character iterator with one-character lookahead.
    chars: Peekable<Chars<'a>>,
    /// Byte offset of the *next* character to be consumed by `next_char`.
    /// Starts at 0.  Incremented by `ch.len_utf8()` with every consumed char,
    /// keeping it valid as a UTF-8 byte index into `source`.
    position: usize,
}

impl<'a> Lexer<'a> {

    /// Creates a new `Lexer` positioned at the start of `source`.
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            chars: source.chars().peekable(),
            position: 0,
        }
    }

    // -------------------------------------------------------------------------
    // Low-level character helpers
    // -------------------------------------------------------------------------

    /// Consumes and returns the next character, advancing `position` by the
    /// character's UTF-8 byte length.  Returns `None` at end-of-input.
    fn next_char(&mut self) -> Option<char> {
        let ch = self.chars.next()?;
        self.position += ch.len_utf8();
        Some(ch)
    }

    /// Returns the next character without consuming it.
    fn peek_char(&mut self) -> Option<char> {
        self.chars.peek().copied()
    }

    /// Returns `true` if the next (unconsumed) character equals `ch`.
    fn next_char_is(&mut self, ch: char) -> bool {
        self.peek_char() == Some(ch)
    }

    /// Skips all leading whitespace characters (space, tab, newline, …).
    fn skip_whitespaces(&mut self) {
        while let Some(ch) = self.peek_char() {
            if ch.is_whitespace() {
                self.next_char();
            } else {
                break;
            }
        }
    }

    // -------------------------------------------------------------------------
    // Token-specific scanners
    // -------------------------------------------------------------------------

    /// Scans the rest of an identifier, given that its first character has
    /// already been consumed and `start_pos` is its byte offset.
    ///
    /// Identifier continuation characters are letters, digits, and `_`.
    /// The first character (already consumed by the caller) is either a letter
    /// or `_`, so this method only extends the span rightward.
    ///
    /// Returns a [`Token::Identifier`] borrowing the scanned slice from
    /// `source`.
    fn next_identifier(&mut self, start_pos: usize) -> Token<'a> {
        while let Some(ch) = self.peek_char() {
            if ch == '_' || ch.is_alphanumeric() {
                self.next_char();
            } else {
                break;
            }
        }
        let literal = &self.source[start_pos..self.position];
        Token::Identifier(literal)
    }

    /// Consumes a run of ASCII decimal digits without returning anything.
    /// Used as a building block inside [`next_numeric_literal`](Self::next_numeric_literal).
    fn consume_digits(&mut self) {
        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_digit() {
                self.next_char();
            } else {
                break;
            }
        }
    }

    /// Scans a numeric literal, given that its first digit has already been
    /// consumed and `start_pos` is its byte offset.
    ///
    /// Accepts the following forms (matching the JS reference grammar):
    ///
    /// ```text
    /// integer:    123
    /// decimal:    12.3
    /// scientific: 12.3e2  12.3E+2  12.3e-2
    /// ```
    ///
    /// Returns [`Token::Number`] on success, or [`Token::Undefined`] when the
    /// literal is syntactically malformed (e.g. `12.` — trailing dot without
    /// digits, or `1e+` — exponent sign without digits).
    ///
    /// > **Known issue**: a number immediately followed by an identifier
    /// > character (e.g. `123abc`) is *not* treated as an error — the lexer
    /// > yields `Number(123)` then `Identifier("abc")`.  The JS reference
    /// > lexer throws a syntax error in that case.
    fn next_numeric_literal(&mut self, start_pos: usize) -> Token<'a> {

        // Consume the integer part (the first digit was already consumed by the
        // caller, but consume_digits will handle any remaining leading digits).
        self.consume_digits();

        // Optional fractional part: `.` followed by one or more digits.
        //
        // NOTE: the `.` is consumed *before* checking for a following digit.
        // This means that `12.x` yields `Undefined("12.")` + `Identifier("x")`
        // rather than `Number(12)` + `SubcontextingOp` + `Identifier("x")`.
        // Fixing this would require a two-character lookahead.
        if self.peek_char() == Some('.') {
            self.next_char(); // consume '.'
            if matches!(self.peek_char(), Some('0'..='9')) {
                self.consume_digits();
            } else {
                // Dot not followed by a digit → malformed literal.
                return Token::Undefined(&self.source[start_pos..self.position]);
            }
        }

        // Optional exponent part: `e` or `E`, optional sign, one or more digits.
        if matches!(self.peek_char(), Some('e') | Some('E')) {
            self.next_char(); // consume 'e'/'E'
            if matches!(self.peek_char(), Some('+') | Some('-')) {
                self.next_char(); // consume optional sign
            }
            if matches!(self.peek_char(), Some('0'..='9')) {
                self.consume_digits();
            } else {
                // Exponent marker not followed by digits → malformed literal.
                return Token::Undefined(&self.source[start_pos..self.position]);
            }
        }

        // The accumulated substring is a well-formed f64 string; Rust's
        // standard parser handles all three forms above.
        let number_str = &self.source[start_pos..self.position];
        let value: f64 = number_str
            .parse()
            .expect("BUG: lexer produced a number string that f64::parse() rejected");
        Token::Number(value)
    }

    /// Scans a double-quoted string literal, given that the opening `"` has
    /// already been consumed and `start_pos` is the byte offset of the first
    /// content character (i.e. `self.position` right after consuming `"`).
    ///
    /// The returned [`Token::Text`] slice contains only the content between
    /// the quotes; the delimiters are not included.
    ///
    /// If the input ends before a closing `"` is found, the slice extends to
    /// the end of the source rather than returning an error.
    fn next_string_literal(&mut self, start_pos: usize) -> Token<'a> {
        let end_pos: usize = loop {
            let ch = self.peek_char();
            if ch == Some('"') {
                // Found the closing quote.  Record the end position *before*
                // consuming it so the quote is excluded from the slice.
                let end_pos = self.position;
                self.next_char(); // consume closing '"'
                break end_pos;
            } else if ch.is_none() {
                // End of input without a closing quote — emit the content so
                // far rather than erroring (lenient, matching the JS behaviour).
                break self.position;
            } else {
                self.next_char();
            }
        };
        Token::Text(&self.source[start_pos..end_pos])
    }

    /// Scans a `#`-prefixed line comment, given that the `#` has already been
    /// consumed and `start_pos` is the byte offset of the first content
    /// character (i.e. `self.position` right after consuming `#`).
    ///
    /// The comment extends to (but does not include) the next `\n`, or to the
    /// end of the source if there is no newline.  The `\n` itself is consumed
    /// and discarded.
    ///
    /// The returned [`Token::Comment`] slice contains only the text after `#`
    /// and before the terminating newline.
    fn next_comment(&mut self, start_pos: usize) -> Token<'a> {
        let end_pos: usize = loop {
            let ch = self.peek_char();
            if ch == Some('\n') {
                // Newline terminates the comment; exclude it from the slice.
                let end_pos = self.position;
                self.next_char(); // consume '\n'
                break end_pos;
            } else if ch.is_none() {
                // End of input — comment runs to the end of the source.
                break self.position;
            } else {
                self.next_char();
            }
        };
        Token::Comment(&self.source[start_pos..end_pos])
    }

    // -------------------------------------------------------------------------
    // Main entry point
    // -------------------------------------------------------------------------

    /// Returns the next token and its byte position in the source.
    ///
    /// Leading whitespace is skipped before each token.  When the source is
    /// exhausted this method returns `(pos, Token::EndOfExpression)` and
    /// continues to do so on every subsequent call — the position will be
    /// pinned at the end of the source.
    ///
    /// Two-character operators (`==`, `!=`, `<=`, `>=`, `->`, `<<`, `>>`,
    /// `=>`) are recognised greedily: the first character is consumed by the
    /// main dispatch, then if the following character completes the operator it
    /// is consumed too before the token is returned.
    pub fn next_token(&mut self) -> (usize, Token<'a>) {
        self.skip_whitespaces();

        // Snapshot the byte offset of the token we are about to produce.
        let ch_pos = self.position;

        // Consume the first character; return EndOfExpression at end-of-input.
        let ch = match self.next_char() {
            Some(ch) => ch,
            None => return (ch_pos, Token::EndOfExpression),
        };

        // --- Identifiers -------------------------------------------------------
        // Start character: letter or `_`.  The rest of the identifier is
        // consumed by next_identifier.
        if ch == '_' || ch.is_alphabetic() {
            (ch_pos, self.next_identifier(ch_pos))
        }

        // --- Numeric literals --------------------------------------------------
        // Start character: ASCII digit.
        else if ch.is_ascii_digit() {
            (ch_pos, self.next_numeric_literal(ch_pos))
        }

        // --- String literals ---------------------------------------------------
        // Opening `"`.  self.position now points past the quote.
        else if ch == '"' {
            (ch_pos, self.next_string_literal(self.position))
        }

        // --- Line comments -----------------------------------------------------
        // Opening `#`.  self.position now points past the `#`.
        else if ch == '#' {
            (ch_pos, self.next_comment(self.position))
        }

        // --- Two-character operators -------------------------------------------
        // Each branch peeks at the next character before committing.

        // `==`
        else if ch == '=' && self.next_char_is('=') {
            self.next_char(); (ch_pos, Token::EqualToOp)
        }
        // `!=`
        else if ch == '!' && self.next_char_is('=') {
            self.next_char(); (ch_pos, Token::NotEqualToOp)
        }
        // `<=`
        else if ch == '<' && self.next_char_is('=') {
            self.next_char(); (ch_pos, Token::LessThanOrEqualToOp)
        }
        // `>=`
        else if ch == '>' && self.next_char_is('=') {
            self.next_char(); (ch_pos, Token::GreaterThanOrEqualToOp)
        }
        // `->`
        else if ch == '-' && self.next_char_is('>') {
            self.next_char(); (ch_pos, Token::FunctionDefinitionOp)
        }
        // `<<`
        else if ch == '<' && self.next_char_is('<') {
            self.next_char(); (ch_pos, Token::CompositionOp)
        }
        // `>>`
        else if ch == '>' && self.next_char_is('>') {
            self.next_char(); (ch_pos, Token::ReverseCompositionOp)
        }
        // `=>`
        else if ch == '=' && self.next_char_is('>') {
            self.next_char(); (ch_pos, Token::MappingOp)
        }

        // --- Single-character tokens -------------------------------------------
        else {
            match ch {
                '(' => (ch_pos, Token::LeftRoundBracket),
                ')' => (ch_pos, Token::RightRoundBracket),

                '[' => (ch_pos, Token::LeftSquareBracket),
                ']' => (ch_pos, Token::RightSquareBracket),

                '{' => (ch_pos, Token::LeftCurlyBracket),
                '}' => (ch_pos, Token::RightCurlyBracket),

                '.' => (ch_pos, Token::SubcontextingOp),

                '^' => (ch_pos, Token::PowerOp),
                '*' => (ch_pos, Token::ProductOp),
                '/' => (ch_pos, Token::DivisionOp),
                '%' => (ch_pos, Token::ModuloOp),
                '+' => (ch_pos, Token::PlusOp),
                '-' => (ch_pos, Token::MinusOp),

                '<' => (ch_pos, Token::LessThanOp),
                '>' => (ch_pos, Token::GreaterThanOp),

                '&' => (ch_pos, Token::AndOp),
                '|' => (ch_pos, Token::OrOp),

                '?' => (ch_pos, Token::ConditionalOp),
                ';' => (ch_pos, Token::AlternativeOp),

                '=' => (ch_pos, Token::AssignmentOp),
                ':' => (ch_pos, Token::LabellingOp),

                ',' => (ch_pos, Token::PairingOp),

                '!' => (ch_pos, Token::NotOp),
                '$' => (ch_pos, Token::LiftingOp),

                // Anything else is returned as an Undefined token containing
                // exactly the one unrecognised character.
                _ => (ch_pos, Token::Undefined(&self.source[ch_pos..self.position])),
            }
        }
    }
}


// -----------------------------------------------------------------------------
// Iterator implementation
// -----------------------------------------------------------------------------

/// Drives the lexer as a standard Rust iterator.
///
/// Each call to `next` delegates to [`next_token`](Lexer::next_token) and maps
/// `Token::EndOfExpression` to `None`, stopping iteration.  All other tokens
/// (including [`Token::Comment`]) are yielded.
///
/// > **Note**: Comment tokens are *not* silently discarded here.  The parser
/// > is responsible for skipping them during expression parsing.
impl<'a> Iterator for Lexer<'a> {
    type Item = (usize, Token<'a>);

    fn next(&mut self) -> Option<Self::Item> {
        let (pos, token) = self.next_token();
        if token == Token::EndOfExpression {
            None
        } else {
            Some((pos, token))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lexer_next_token() {

        macro_rules! assert_token {
            ($input:expr, $token:expr, $pos:expr) => {
                assert_eq!(
                    Lexer::new($input).next_token(),
                    ($pos, $token))
            }
        }

        // Identifier
        assert_token!("Abc1", Token::Identifier("Abc1"), 0);
        assert_token!("abc1", Token::Identifier("abc1"), 0);
        assert_token!("_ab1", Token::Identifier("_ab1"), 0);

        // Numeric Literal
        assert_token!("123", Token::Number(123.0), 0);
        assert_token!("12.3", Token::Number(12.3), 0);
        assert_token!("12.3e2", Token::Number(1230.0), 0);
        assert_token!("12.3E+2", Token::Number(1230.0), 0);
        assert_token!("12.3e-2", Token::Number(0.123), 0);
        assert_token!("12.", Token::Undefined("12."), 0);
        assert_token!("12.3e", Token::Undefined("12.3e"), 0);
        assert_token!("12.3e+", Token::Undefined("12.3e+"), 0);
        assert_token!("12.3E-", Token::Undefined("12.3E-"), 0);

        // Text Literal
        assert_token!("\"abc\"", Token::Text("abc"), 0);
        assert_token!("\"abc", Token::Text("abc"), 0);

        // Brackets
        assert_token!("(", Token::LeftRoundBracket, 0);
        assert_token!(")", Token::RightRoundBracket, 0);
        assert_token!("[", Token::LeftSquareBracket, 0);
        assert_token!("]", Token::RightSquareBracket, 0);
        assert_token!("{", Token::LeftCurlyBracket, 0);
        assert_token!("}", Token::RightCurlyBracket, 0);

        // Binary operators
        assert_token!(".", Token::SubcontextingOp, 0);

        assert_token!("^", Token::PowerOp, 0);
        assert_token!("*", Token::ProductOp, 0);
        assert_token!("/", Token::DivisionOp, 0);
        assert_token!("%", Token::ModuloOp, 0);
        assert_token!("+", Token::PlusOp, 0);
        assert_token!("-", Token::MinusOp, 0);

        assert_token!("==", Token::EqualToOp, 0);
        assert_token!("!=", Token::NotEqualToOp, 0);
        assert_token!("<", Token::LessThanOp, 0);
        assert_token!("<=", Token::LessThanOrEqualToOp, 0);
        assert_token!(">", Token::GreaterThanOp, 0);
        assert_token!(">=", Token::GreaterThanOrEqualToOp, 0);
        
        assert_token!("&", Token::AndOp, 0);
        assert_token!("|", Token::OrOp, 0);

        assert_token!("?", Token::ConditionalOp, 0);
        assert_token!(";", Token::AlternativeOp, 0);

        assert_token!("->", Token::FunctionDefinitionOp, 0);

        assert_token!("<<", Token::CompositionOp, 0);
        assert_token!(">>", Token::ReverseCompositionOp, 0);

        assert_token!("=>", Token::MappingOp, 0);

        assert_token!("=", Token::AssignmentOp, 0);
        assert_token!(":", Token::LabellingOp, 0);

        assert_token!(",", Token::PairingOp, 0);

        // Special tokens
        assert_token!("@", Token::Undefined("@"), 0);

        assert_token!("# this is a comment \n\n", Token::Comment(" this is a comment "), 0);
        assert_token!("# this is a comment ", Token::Comment(" this is a comment "), 0);

        assert_token!("", Token::EndOfExpression, 0);

        // Spaces skipping
        assert_token!("  *", Token::ProductOp, 2);
    }

    #[test]
    fn test_lexer_iterator() {
        let input = "msg = \"hello\"";
        let lexer = Lexer::new(input);
        
        let tokens: Vec<(usize, Token)> = lexer.collect();
        assert_eq!(
            tokens,
            vec![
                (0, Token::Identifier("msg")),
                (4, Token::AssignmentOp),
                (6, Token::Text("hello")),
            ]
        );
    }
}
````

## File: src/lib.rs
````rust
pub mod token;
pub mod lexer;

pub mod ast;
pub mod parser;

pub mod value;
pub mod eval;
pub mod builtins;
````

## File: src/parser.rs
````rust
// =============================================================================
// parser.rs — Pratt (Top-Down Operator Precedence) parser for Swan
// =============================================================================
//
// What is a Pratt parser?
// -----------------------
// A Pratt parser is a recursive-descent parser where every token has two
// optional "parsing functions" associated with it:
//
//   • prefix_fn  — called when the token appears at the START of an
//                  expression (e.g. a number, an identifier, a `(`, a
//                  unary `-`).
//
//   • infix_fn   — called when the token appears BETWEEN two expressions
//                  (e.g. `+`, `*`, `.`).  The left-hand expression is
//                  passed in as an argument.
//
// The core loop (`parse_expr`) works like this:
//
//   1. Call `parse_prefix` to parse the left-hand side (lhs).
//   2. Peek at the next token.  If its BINDING POWER (precedence) is
//      greater than `min_bp`, consume the token and call `parse_infix`
//      to combine the lhs with the rhs into a new node.
//   3. Repeat step 2 until the next token's binding power is ≤ `min_bp`.
//
// Binding power controls associativity:
//   • Left-associative  (+, -, *, …) : pass the SAME power as the infix op
//                                      when recursing for the rhs.
//   • Right-associative (^, ->)      : pass ONE LESS than the infix op's
//                                      power so that the rhs grabs more.
//
// References
// ----------
// • Vaughan Pratt, "Top Down Operator Precedence" (1973)
// • https://matklad.github.io/2020/04/13/simple-but-powerful-pratt-parsing.html

use crate::ast::{BinaryOp, Leaf, Node, UnaryOp};
use crate::lexer::Lexer;
use crate::token::Token;





// =============================================================================
// Binding-power table
// =============================================================================
//
// Each infix operator has a LEFT and a RIGHT binding power (lbp / rbp).
// The parser compares the NEXT token's lbp against the current minimum; it
// consumes the token only when  lbp > min_bp.
//
// For a left-associative operator the rbp equals the lbp (so the same level
// cannot re-associate on the right).  For a right-associative operator the
// rbp is lbp - 1 (one less), allowing the same operator to appear again on
// the right and bind first.
//
// The numbers below match the precedence values documented in token.rs.

/// Returns the LEFT binding power of an infix/postfix operator token.
///
/// Returns `0` for tokens that are NOT infix operators (terminators, closing
/// brackets, unknown tokens). This causes the main loop to stop.
fn lbp(token: &Token) -> u32 {
    match token {
        // Lowest-precedence infix operators come first.
        Token::PairingOp             => 10,   // ,

        Token::AssignmentOp          => 12,   // =
        Token::LabellingOp           => 12,   // :

        Token::MappingOp             => 13,   // =>

        Token::ReverseCompositionOp  => 14,   // >>
        Token::CompositionOp         => 14,   // <<

        Token::FunctionDefinitionOp  => 15,   // ->

        Token::AlternativeOp         => 21,   // ;
        Token::ConditionalOp         => 22,   // ?

        Token::OrOp                  => 23,   // |
        Token::AndOp                 => 23,   // &

        Token::EqualToOp             => 24,   // ==
        Token::NotEqualToOp          => 24,   // !=
        Token::LessThanOp            => 24,   // <
        Token::LessThanOrEqualToOp   => 24,   // <=
        Token::GreaterThanOp         => 24,   // >
        Token::GreaterThanOrEqualToOp => 24,  // >=

        Token::PlusOp                => 25,   // +
        Token::MinusOp               => 25,   // -

        Token::ProductOp             => 26,   // *
        Token::DivisionOp            => 26,   // /
        Token::ModuloOp              => 26,   // %

        Token::PowerOp               => 27,   // ^  (right-assoc, see rbp)

        Token::SubcontextingOp       => 30,   // .

        // Everything else is not an infix operator → lbp = 0 stops the loop.
        _ => 0,
    }
}

/// Returns the RIGHT binding power of an infix operator token.
///
/// For left-associative operators this equals `lbp(token)` (the rhs
/// recurses at the same level, preventing re-association at this level).
/// For right-associative operators this is `lbp(token) - 1` (the rhs
/// recurses one level lower, so two operators at the same level bind
/// right-to-left).
fn rbp(token: &Token) -> u32 {
    match token {
        // Right-associative operators.
        Token::PowerOp              => lbp(token) - 1,   // ^
        Token::FunctionDefinitionOp => lbp(token) - 1,   // ->
        Token::MappingOp            => lbp(token) - 1,   // =>

        // All other infix operators are left-associative.
        _ => lbp(token),
    }
}





// =============================================================================
// Application (empty) operator
// =============================================================================
//
// The application operator has no token.  It is triggered when the look-ahead
// can begin an expression (i.e. it is in "prefix position") while the parser
// is in the middle of the infix loop.
//
// Binding power: 28 — tighter than exponentiation (27) so `f x ^ 2` parses
// as `(f x) ^ 2`, but looser than sub-contexting (30) so `f x.y` parses
// as `f (x.y)`.  Application is left-associative (lbp == rbp).

/// The left *and* right binding power of the implicit application operator.
const APPLICATION_BP: u32 = 28;

/// Returns `true` when `token` can start a new expression (prefix position).
///
/// These are exactly the tokens for which `parse_prefix` has a real (non-`_`)
/// branch.  When one of them appears in the look-ahead after a complete lhs,
/// we synthesise an `ApplicationOp` without consuming any infix token.
fn is_application_starter(token: &Token) -> bool {
    matches!(
        token,
        Token::Identifier(_)
            | Token::Number(_)
            | Token::Text(_)
            | Token::LeftRoundBracket
            | Token::LeftSquareBracket
            | Token::LeftCurlyBracket
    )
}





// =============================================================================
// Parser struct
// =============================================================================
//
// The parser keeps a ONE-TOKEN look-ahead.
//
// Why own Token instead of borrowing it?
// ---------------------------------------
// The lexer returns `(usize, Token<'a>)` by VALUE each time `next_token` is
// called. Storing those values as `&'a Token<'a>` would be storing a reference
// to a LOCAL variable that is about to be dropped — a classic Rust lifetime
// error. Owning the token (no `&`) is the correct fix.

pub struct Parser<'a> {
    /// The lexer feeds us tokens on demand.
    lexer: Lexer<'a>,

    /// The CURRENT token (just consumed) and its byte position in the source.
    /// Initialised to `(0, Token::EndOfExpression)` before the first real
    /// token is fetched.
    current_token: Token<'a>,
    current_pos:   usize,

    /// The NEXT token (one step ahead) and its position.
    /// Pre-filled by `new` so every call to peek is O(1).
    peek_token: Token<'a>,
    peek_pos:   usize,
}

impl<'a> Parser<'a> {

    // -------------------------------------------------------------------------
    // Construction
    // -------------------------------------------------------------------------

    /// Creates a new parser wrapping `lexer` and pre-loads the look-ahead.
    pub fn new(mut lexer: Lexer<'a>) -> Self {
        // Prime the look-ahead with the very first real token from the lexer.
        // `current_token` starts as a dummy; it will be replaced on the first
        // call to `advance`.
        let (peek_pos, peek_token) = lexer.next_token();
        Self {
            lexer,
            current_token: Token::EndOfExpression,
            current_pos:   0,
            peek_token,
            peek_pos,
        }
    }

    // -------------------------------------------------------------------------
    // Token management helpers
    // -------------------------------------------------------------------------

    /// Advances the parser by one token.
    ///
    /// Moves the look-ahead into `current_*` and fetches a new look-ahead from
    /// the lexer.
    ///
    /// `std::mem::replace` lets us MOVE `self.peek_token` out of the struct
    /// (which we can't do with a plain assignment because the struct must
    /// always remain fully initialised) while simultaneously writing the new
    /// look-ahead value into its place.
    fn advance(&mut self) {
        let (new_peek_pos, new_peek_token) = self.lexer.next_token();

        self.current_token = std::mem::replace(&mut self.peek_token, new_peek_token);
        self.current_pos   = self.peek_pos;
        self.peek_pos      = new_peek_pos;
    }

    /// Skips any `Comment` tokens from the look-ahead.
    ///
    /// The lexer yields comments as real tokens; the parser ignores them.
    /// Call this whenever you are about to inspect the look-ahead.
    fn skip_comments(&mut self) {
        // Keep advancing while the upcoming token is a comment.
        while matches!(self.peek_token, Token::Comment(_)) {
            self.advance();
        }
    }

    // -------------------------------------------------------------------------
    // Core Pratt parsing engine
    // -------------------------------------------------------------------------

    /// Parses a single complete expression at the given minimum binding power.
    ///
    /// `min_bp` is the binding power of the operator that called us.  We only
    /// consume infix operators whose left-binding-power is GREATER than
    /// `min_bp`; everything else is left for the caller to handle.
    ///
    /// Call with `min_bp = 0` to parse a full expression.
    ///
    /// # Algorithm (step by step)
    ///
    /// 1. Parse a PREFIX production → gives us the left-hand node (lhs).
    /// 2. Loop:
    ///    a. Skip comments and peek at the next token.
    ///    b. If `lbp(peek) <= min_bp` → STOP; return lhs.
    ///    c. Consume the infix token (advance).
    ///    d. Recursively parse the rhs with `min_bp = rbp(infix_token)`.
    ///    e. Combine lhs + rhs into a new node; that becomes the new lhs.
    pub fn parse_expr(&mut self, min_bp: u32) -> Node {

        // --- Step 1: prefix production ----------------------------------------
        self.skip_comments();
        if min_bp == 0 && matches!(self.peek_token, Token::EndOfExpression) {
            return Node::Nothing(Leaf { pos: self.peek_pos, val: () });
        }
        let mut lhs = self.parse_prefix();

        // --- Step 2: infix loop -----------------------------------------------
        loop {
            self.skip_comments();

            // --- Case A: implicit application operator ------------------------
            //
            // When the look-ahead can start an expression AND the application
            // binding power beats the current minimum, we treat the two adjacent
            // expressions as `lhs rhs` (function application).
            //
            // We do NOT consume any token here — `parse_prefix` inside the
            // recursive call to `parse_expr` will advance past the rhs on its
            // own.  The position we record for the node is the byte offset of
            // the rhs (i.e. `self.peek_pos`), since there is no operator token.
            if APPLICATION_BP > min_bp && is_application_starter(&self.peek_token) {
                let pos = self.peek_pos;
                // Right binding power equals left (left-associative), so we
                // recurse at APPLICATION_BP to prevent right-nesting at the
                // same level.
                let rhs = self.parse_expr(APPLICATION_BP);
                lhs = Node::ApplicationOp(BinaryOp {
                    pos,
                    lho: Box::new(lhs),
                    rho: Box::new(rhs),
                });
                continue;
            }

            // --- Case B: explicit infix operator ------------------------------

            // Peek: if the next token isn't an infix operator (or has too low
            // a binding power), we're done with this sub-expression.
            let next_lbp = lbp(&self.peek_token);
            if next_lbp <= min_bp {
                break;
            }

            // Consume the infix operator so `current_token` holds it.
            self.advance();

            // IMPORTANT: snapshot the infix operator NOW, before we recurse.
            // The recursive call to parse_expr will call advance() many times,
            // overwriting self.current_token with whatever the recursion
            // consumes last. If we don't save it here, parse_infix will see
            // the wrong token and panic.
            //
            // `Token` derives `Clone`, so this is a cheap copy for unit
            // variants (operators have no heap data) and a heap allocation only
            // for string-carrying variants like Identifier — which never appear
            // in the infix position anyway.
            let infix_token = self.current_token.clone();
            let infix_pos   = self.current_pos;

            // Parse the right-hand side at the right binding power of this op.
            // Using rbp instead of lbp here is what makes right-associative
            // operators work: rbp < lbp means the recursive call can "steal"
            // another operator at the same level.
            let right_bp = rbp(&infix_token);
            let rhs = self.parse_expr(right_bp);

            // Combine lhs and rhs using the saved infix token.
            lhs = self.parse_infix(lhs, rhs, infix_token, infix_pos);
        }

        lhs
    }

    // -------------------------------------------------------------------------
    // Prefix productions
    // -------------------------------------------------------------------------

    /// Called at the start of each expression.
    ///
    /// Advances to the next token, then dispatches on its kind to build the
    /// appropriate AST node.  Each branch handles one "prefix" form:
    ///   • Literal values (number, text, identifier)
    ///   • Unary operators (prefix `+`, prefix `-`)
    ///   • Grouping (`(`, `[`, `{`)
    fn parse_prefix(&mut self) -> Node {
        self.advance();                     // consume the first token of this expression
        let pos = self.current_pos;

        match &self.current_token {

            // -----------------------------------------------------------------
            // Literals
            // -----------------------------------------------------------------

            // A numeric literal becomes a `Node::Numb` leaf.
            Token::Number(val) => {
                let val = *val;             // copy the f64 out before the borrow ends
                Node::Numb(Leaf { pos, val })
            }

            // A string literal becomes a `Node::Text` leaf.
            Token::Text(val) => {
                let val = val.to_string();
                Node::Text(Leaf { pos, val })
            }

            // An identifier becomes a `Node::Identifier` leaf.
            Token::Identifier(name) => {
                let val = name.to_string();
                Node::Identifier(Leaf { pos, val })
            }

            // -----------------------------------------------------------------
            // Grouped expressions
            // -----------------------------------------------------------------

            // A `(` starts a parenthesised sub-expression.
            // Empty parentheses `()` represent Nothing.
            // Otherwise, parse the inner expression at precedence 0 (so ALL
            // operators inside bind), then expect a closing `)`.
            Token::LeftRoundBracket => {
                self.skip_comments();
                if matches!(self.peek_token, Token::RightRoundBracket) {
                    self.advance();
                    Node::Nothing(Leaf { pos, val: () })
                } else {
                    let inner = self.parse_expr(0);
                    // Consume the matching `)`.
                    self.advance();
                    if matches!(self.current_token, Token::RightRoundBracket) {
                        inner
                    } else {
                        Node::Unclosed(Leaf {pos:self.current_pos, val:')'})
                    }
                }
            }

            // A `[` starts a list literal.
            // An empty bracket pair `[]` is a list containing Nothing (which yields an empty list).
            // Parse the inner expression at precedence 0, then expect `]`.
            Token::LeftSquareBracket => {
                self.skip_comments();
                let inner = if matches!(self.peek_token, Token::RightSquareBracket) {
                    let void_pos = self.peek_pos;
                    self.advance();
                    Node::Nothing(Leaf { pos: void_pos, val: () })
                } else {
                    let inner = self.parse_expr(0);
                    self.advance();
                    if matches!(self.current_token, Token::RightSquareBracket) {
                        inner
                    } else {
                        Node::Unclosed(Leaf {pos:self.current_pos, val:']'})
                    }
                };
                Node::List(UnaryOp { pos, opd: Box::new(inner) })
            }

            // A `{` starts a namespace literal.
            // Parse the inner expression at precedence 0, then expect `}`.
            Token::LeftCurlyBracket => {
                self.skip_comments();
                let inner = if matches!(self.peek_token, Token::RightCurlyBracket) {
                    let void_pos = self.peek_pos;
                    self.advance();
                    Node::Nothing(Leaf { pos: void_pos, val: () })
                } else {
                    let inner = self.parse_expr(0);
                    self.advance();
                    if matches!(self.current_token, Token::RightCurlyBracket) {
                        inner
                    } else {
                        Node::Unclosed(Leaf {pos:self.current_pos, val:'}'})
                    }
                };
                Node::Namespace(UnaryOp { pos, opd: Box::new(inner) })
            }

            // -----------------------------------------------------------------
            // Unary (prefix) operators
            // -----------------------------------------------------------------

            // Prefix `+` — unary affirmation (no real effect on numbers, but
            // it is syntactically valid: `+x`).
            // We parse the operand at a HIGH binding power (50) so that the
            // unary op grabs its argument tightly.
            Token::PlusOp => {
                let opd = Box::new(self.parse_expr(50));
                Node::AffirmOp(UnaryOp { pos, opd })
            }

            // Prefix `-` — unary negation.
            Token::MinusOp => {
                let opd = Box::new(self.parse_expr(50));
                Node::NegateOp(UnaryOp { pos, opd })
            }

            // Prefix `!` — unary lofical negation.
            Token::NotOp => {
                let opd = Box::new(self.parse_expr(50));
                Node::LogicalNegationOp(UnaryOp { pos, opd })
            }

            // Prefix `$` — unary lifting operation.
            Token::LiftingOp => {
                let opd = Box::new(self.parse_expr(50));
                Node::LiftingOp(UnaryOp { pos, opd })
            }

            // -----------------------------------------------------------------
            // Anything else is unexpected at the start of an expression.
            // -----------------------------------------------------------------
            _ => { 
                let val = format!("{:?}", self.current_token);
                Node::UnexpectedToken( Leaf {pos, val})
            }
        }
    }

    // -------------------------------------------------------------------------
    // Infix productions
    // -------------------------------------------------------------------------

    /// Combines a parsed `lhs` and `rhs` into the correct binary `Node`.
    ///
    /// # Why take the token as a parameter?
    ///
    /// After `parse_expr(right_bp)` returns, `self.current_token` has been
    /// overwritten many times by the recursive calls to `advance()` inside it.
    /// It no longer holds the infix operator — it holds whatever the deepest
    /// recursion consumed last.  We therefore snapshot the infix token in
    /// `parse_expr` (with `.clone()`) before recursing and pass it in here.
    ///
    /// `pos`   — byte position of the operator in the source.
    /// `token` — the infix operator token (already consumed, passed by value).
    fn parse_infix(&mut self, lhs: Node, rhs: Node, token: Token<'a>, pos: usize) -> Node {

        // Helper closure — packages lhs + rhs into a `BinaryOp`.
        let bin = |lho: Node, rho: Node| BinaryOp {
            pos,
            lho: Box::new(lho),
            rho: Box::new(rho),
        };

        match token {

            // Arithmetic
            Token::PlusOp       => Node::SumOp(bin(lhs, rhs)),
            Token::MinusOp      => Node::SubtractionOp(bin(lhs, rhs)),
            Token::ProductOp    => Node::ProductOp(bin(lhs, rhs)),
            Token::DivisionOp   => Node::DivisionOp(bin(lhs, rhs)),
            Token::ModuloOp     => Node::ModuloOp(bin(lhs, rhs)),
            Token::PowerOp      => Node::PowerOp(bin(lhs, rhs)),

            // Comparison
            Token::EqualToOp              => Node::EqualToOp(bin(lhs, rhs)),
            Token::NotEqualToOp           => Node::NotEqualToOp(bin(lhs, rhs)),
            Token::LessThanOp             => Node::LessThanOp(bin(lhs, rhs)),
            Token::LessThanOrEqualToOp    => Node::LessThanOrEqualToOp(bin(lhs, rhs)),
            Token::GreaterThanOp          => Node::GreaterThanOp(bin(lhs, rhs)),
            Token::GreaterThanOrEqualToOp => Node::GreaterThanOrEqualToOp(bin(lhs, rhs)),

            // Logic
            Token::AndOp => Node::AndOp(bin(lhs, rhs)),
            Token::OrOp  => Node::OrOp(bin(lhs, rhs)),

            // Control flow / short-circuit
            Token::ConditionalOp  => Node::ConditionalOp(bin(lhs, rhs)),
            Token::AlternativeOp  => Node::AlternativeOp(bin(lhs, rhs)),

            // Function-related
            Token::FunctionDefinitionOp => Node::FunctionDefinitionOp(bin(lhs, rhs)),
            Token::MappingOp            => Node::MappingOp(bin(lhs, rhs)),

            // Composition
            Token::CompositionOp        => Node::CompositionOp(bin(lhs, rhs)),
            Token::ReverseCompositionOp => Node::ReverseCompositionOp(bin(lhs, rhs)),

            // Namespace access: `foo.bar`
            Token::SubcontextingOp => Node::SubcontextingOp(bin(lhs, rhs)),

            // Assignment and labelling
            Token::AssignmentOp => Node::AssignmentOp(bin(lhs, rhs)),
            Token::LabellingOp  => Node::LabellingOp(bin(lhs, rhs)),

            // Pairing (tuple / comma list): `a, b, c`
            Token::PairingOp => Node::PairingOp(bin(lhs, rhs)),

            // Anything that reaches here is a parser bug.
            _ => {
                let val = format!("{:?}", token);
                Node::UnexpectedToken( Leaf {pos, val})
            }
        }
    }

    // -------------------------------------------------------------------------
    // Public entry point
    // -------------------------------------------------------------------------

    /// Parse the entire input as a single expression and return the root node.
    ///
    /// ```
    /// use swan_core::lexer::Lexer;
    /// use swan_core::parser::Parser;
    ///
    /// let lexer = Lexer::new("1 + 2 * 3");
    /// let mut parser = Parser::new(lexer);
    /// let ast = parser.parse();   // PlusOp(1, ProductOp(2, 3))
    /// ```
    pub fn parse(&mut self) -> Node {
        // Start with the lowest possible binding power so nothing is "held
        // back" — the entire input becomes one big expression.
        self.parse_expr(0)
    }
}





// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{BinaryOp, Leaf, Node, UnaryOp};
    use crate::lexer::Lexer;

    /// Convenience: parse `src` and return the root node.
    fn parse(src: &str) -> Node {
        Parser::new(Lexer::new(src)).parse()
    }

    // -------------------------------------------------------------------------
    // Literals
    // -------------------------------------------------------------------------

    #[test]
    fn test_number_literal() {
        assert_eq!(parse("42"), Node::Numb(Leaf { pos: 0, val: 42.0 }));
    }

    #[test]
    fn test_empty_expression() {
        assert_eq!(parse(""), Node::Nothing(Leaf { pos: 0, val: () }));
    }

    #[test]
    fn test_whitespace_empty_expression() {
        assert_eq!(parse("   "), Node::Nothing(Leaf { pos: 3, val: () }));
    }

    #[test]
    fn test_comment_only_expression() {
        assert_eq!(parse("# comment\n"), Node::Nothing(Leaf { pos: 10, val: () }));
    }

    #[test]
    fn test_nothing_literal() {
        assert_eq!(parse("()"), Node::Nothing(Leaf { pos: 0, val: () }));
    }

    #[test]
    fn test_nothing_literal_with_whitespace_and_comment() {
        assert_eq!(parse("(\n  # comment\n)"), Node::Nothing(Leaf { pos: 0, val: () }));
    }

    #[test]
    fn test_nested_nothing_literal() {
        assert_eq!(parse("(())"), Node::Nothing(Leaf { pos: 1, val: () }));
    }

    #[test]
    fn test_empty_list_literal() {
        assert_eq!(
            parse("[]"),
            Node::List(UnaryOp {
                pos: 0,
                opd: Box::new(Node::Nothing(Leaf { pos: 1, val: () })),
            })
        );
    }

    #[test]
    fn test_empty_namespace_literal() {
        assert_eq!(
            parse("{}"),
            Node::Namespace(UnaryOp {
                pos: 0,
                opd: Box::new(Node::Nothing(Leaf { pos: 1, val: () })),
            })
        );
    }

    #[test]
    fn test_application_with_nothing_argument() {
        assert_eq!(
            parse("f ()"),
            Node::ApplicationOp(BinaryOp {
                pos: 2,
                lho: Box::new(Node::Identifier(Leaf { pos: 0, val: "f".to_string() })),
                rho: Box::new(Node::Nothing(Leaf { pos: 2, val: () })),
            })
        );
    }

    #[test]
    fn test_text_literal() {
        assert_eq!(
            parse("\"hello\""),
            Node::Text(Leaf { pos: 0, val: "hello".to_string() })
        );
    }

    #[test]
    fn test_identifier_literal() {
        assert_eq!(
            parse("foo"),
            Node::Identifier(Leaf { pos: 0, val: "foo".to_string() })
        );
    }


    // -------------------------------------------------------------------------
    // Arithmetic — precedence and associativity
    // -------------------------------------------------------------------------

    #[test]
    fn test_addition() {
        // `1 + 2` → PlusOp(1, 2)
        assert_eq!(
            parse("1 + 2"),
            Node::SumOp(BinaryOp {
                pos: 2,
                lho: Box::new(Node::Numb(Leaf { pos: 0, val: 1.0 })),
                rho: Box::new(Node::Numb(Leaf { pos: 4, val: 2.0 })),
            })
        );
    }

    #[test]
    fn test_precedence_mul_over_add() {
        // `1 + 2 * 3` must parse as `1 + (2 * 3)` because `*` binds tighter.
        // The expected tree: PlusOp(1, ProductOp(2, 3))
        let expected = Node::SumOp(BinaryOp {
            pos: 2,
            lho: Box::new(Node::Numb(Leaf { pos: 0, val: 1.0 })),
            rho: Box::new(Node::ProductOp(BinaryOp {
                pos: 6,
                lho: Box::new(Node::Numb(Leaf { pos: 4, val: 2.0 })),
                rho: Box::new(Node::Numb(Leaf { pos: 8, val: 3.0 })),
            })),
        });
        assert_eq!(parse("1 + 2 * 3"), expected);
    }

    #[test]
    fn test_left_associativity_of_addition() {
        // `1 + 2 + 3` must parse as `(1 + 2) + 3` (left-associative).
        // The expected tree: PlusOp(PlusOp(1, 2), 3)
        let expected = Node::SumOp(BinaryOp {
            pos: 6,
            lho: Box::new(Node::SumOp(BinaryOp {
                pos: 2,
                lho: Box::new(Node::Numb(Leaf { pos: 0, val: 1.0 })),
                rho: Box::new(Node::Numb(Leaf { pos: 4, val: 2.0 })),
            })),
            rho: Box::new(Node::Numb(Leaf { pos: 8, val: 3.0 })),
        });
        assert_eq!(parse("1 + 2 + 3"), expected);
    }

    #[test]
    fn test_right_associativity_of_power() {
        // `2 ^ 3 ^ 4` must parse as `2 ^ (3 ^ 4)` (right-associative).
        let expected = Node::PowerOp(BinaryOp {
            pos: 2,
            lho: Box::new(Node::Numb(Leaf { pos: 0, val: 2.0 })),
            rho: Box::new(Node::PowerOp(BinaryOp {
                pos: 6,
                lho: Box::new(Node::Numb(Leaf { pos: 4, val: 3.0 })),
                rho: Box::new(Node::Numb(Leaf { pos: 8, val: 4.0 })),
            })),
        });
        assert_eq!(parse("2 ^ 3 ^ 4"), expected);
    }


    #[test]
    fn test_unary_minus() {
        // `-3` → NegateOp(3)
        assert_eq!(
            parse("-3"),
            Node::NegateOp(UnaryOp {
                pos: 0,
                opd: Box::new(Node::Numb(Leaf { pos: 1, val: 3.0 })),
            })
        );
    }

    #[test]
    fn test_parentheses_grouping() {
        // `(1 + 2)` → PlusOp(1, 2)  — parentheses are transparent in the AST.
        assert_eq!(
            parse("(1 + 2)"),
            Node::SumOp(BinaryOp {
                pos: 3,
                lho: Box::new(Node::Numb(Leaf { pos: 1, val: 1.0 })),
                rho: Box::new(Node::Numb(Leaf { pos: 5, val: 2.0 })),
            })
        );
    }

    #[test]
    fn test_list_literal() {
        // `[42]` → List(42)
        assert_eq!(
            parse("[42]"),
            Node::List(UnaryOp {
                pos: 0,
                opd: Box::new(Node::Numb(Leaf { pos: 1, val: 42.0 })),
            })
        );
    }

    #[test]
    fn test_namespace_literal() {
        // `{42}` → Namespace(42)
        assert_eq!(
            parse("{42}"),
            Node::Namespace(UnaryOp {
                pos: 0,
                opd: Box::new(Node::Numb(Leaf { pos: 1, val: 42.0 })),
            })
        );
    }


    // -------------------------------------------------------------------------
    // Application operator (empty / juxtaposition)
    // -------------------------------------------------------------------------

    #[test]
    fn test_application_simple() {
        // `1 2` → ApplicationOp(1, 2)
        // pos recorded is the position of the rhs token (2), which is at byte 2.
        assert_eq!(
            parse("1 2"),
            Node::ApplicationOp(BinaryOp {
                pos: 2,
                lho: Box::new(Node::Numb(Leaf { pos: 0, val: 1.0 })),
                rho: Box::new(Node::Numb(Leaf { pos: 2, val: 2.0 })),
            })
        );
    }

    #[test]
    fn test_application_left_associative() {
        // `1 2 3` must parse as `(1 2) 3` (left-associative).
        // Expected tree: ApplicationOp(ApplicationOp(1, 2), 3)
        let expected = Node::ApplicationOp(BinaryOp {
            pos: 4,
            lho: Box::new(Node::ApplicationOp(BinaryOp {
                pos: 2,
                lho: Box::new(Node::Numb(Leaf { pos: 0, val: 1.0 })),
                rho: Box::new(Node::Numb(Leaf { pos: 2, val: 2.0 })),
            })),
            rho: Box::new(Node::Numb(Leaf { pos: 4, val: 3.0 })),
        });
        assert_eq!(parse("1 2 3"), expected);
    }

    #[test]
    fn test_application_binds_tighter_than_arithmetic() {
        // `1 2 + 3` must parse as `(1 2) + 3`
        // because application (bp=28) > addition (bp=25).
        let expected = Node::SumOp(BinaryOp {
            pos: 4,
            lho: Box::new(Node::ApplicationOp(BinaryOp {
                pos: 2,
                lho: Box::new(Node::Numb(Leaf { pos: 0, val: 1.0 })),
                rho: Box::new(Node::Numb(Leaf { pos: 2, val: 2.0 })),
            })),
            rho: Box::new(Node::Numb(Leaf { pos: 6, val: 3.0 })),
        });
        assert_eq!(parse("1 2 + 3"), expected);
    }

    #[test]
    fn test_application_parenthesised_argument() {
        // `1(2)` — the rhs is a parenthesised sub-expression.
        // parse("1(2)") → ApplicationOp(1, 2)
        // The inner `(2)` just passes through the grouping rule.
        assert_eq!(
            parse("1(2)"),
            Node::ApplicationOp(BinaryOp {
                pos: 1,
                lho: Box::new(Node::Numb(Leaf { pos: 0, val: 1.0 })),
                rho: Box::new(Node::Numb(Leaf { pos: 2, val: 2.0 })),
            })
        );
    }
}
````

## File: src/token.rs
````rust
/// All token kinds produced by the [`Lexer`](crate::lexer::Lexer).
///
/// The lifetime `'a` is tied to the source string: `Identifier`, `Text`,
/// `Comment`, and `Undefined` variants borrow slices directly from it, so
/// no heap allocation is needed for those payloads.
#[derive(Debug, PartialEq, Clone)]
pub enum Token<'a> {

    // -------------------------------------------------------------------------
    // Literals and identifiers
    // -------------------------------------------------------------------------

    /// A name — starts with a letter or `_`, continues with letters, digits,
    /// or `_`.  Examples: `foo`, `_bar`, `myVar1`.
    Identifier(&'a str),

    /// A floating-point numeric literal.  Integer, decimal, and scientific
    /// notation are all parsed into an `f64`.  Examples: `42`, `3.14`,
    /// `1.5e-3`.
    Number(f64),

    /// A double-quoted string literal.  The slice contains only the content
    /// between the quotes — the delimiters are not included.
    /// Example: `"hello"` → `Text("hello")`.
    ///
    /// > **Note**: if the closing `"` is absent the slice extends to the end
    /// > of the source string (the lexer does not error on unterminated strings).
    Text(&'a str),

    // -------------------------------------------------------------------------
    //  Grouping
    // -------------------------------------------------------------------------

    /// `(`
    LeftRoundBracket,
    /// `)`
    RightRoundBracket,

    /// `[`  — opens a list literal.
    LeftSquareBracket,
    /// `]`  — closes a list literal.
    RightSquareBracket,

    /// `{`  — opens a namespace literal.
    LeftCurlyBracket,
    /// `}`  — closes a namespace literal.
    RightCurlyBracket,

    // -------------------------------------------------------------------------
    //  Binary and unary operators (listed low → high precedence
    // -------------------------------------------------------------------------

    /// `,`  (precedence 10) — pairing / tuple construction.
    PairingOp,

    /// `=`  (precedence 12) — assignment.
    AssignmentOp,
    /// `:`  (precedence 12) — labelling / named argument.
    LabellingOp,

    /// `=>` (precedence 13) — mapping (anonymous function shorthand).
    MappingOp,

    /// `>>` (precedence 14) — reverse composition / pipe (left-to-right).
    ReverseCompositionOp,
    /// `<<` (precedence 14) — composition (right-to-left).
    CompositionOp,

    /// `->` (precedence 15) — function definition (right-associative).
    FunctionDefinitionOp,

    /// `;`  (precedence 21) — alternative: evaluates the right-hand side only
    /// when the left-hand side is `Undefined`.
    AlternativeOp,

    /// `?`  (precedence 22) — conditional: evaluates the right-hand side only
    /// when the left-hand side is truthy.
    ConditionalOp,

    /// `|`  (precedence 23) — logical OR.
    OrOp,
    /// `&`  (precedence 23) — logical AND.
    AndOp,

    /// `==` (precedence 24) — equality comparison.
    EqualToOp,
    /// `!=` (precedence 24) — inequality comparison.
    NotEqualToOp,
    /// `<`  (precedence 24) — less-than comparison.
    LessThanOp,
    /// `<=` (precedence 24) — less-than-or-equal comparison.
    LessThanOrEqualToOp,
    /// `>`  (precedence 24) — greater-than comparison.
    GreaterThanOp,
    /// `>=` (precedence 24) — greater-than-or-equal comparison.
    GreaterThanOrEqualToOp,

    /// `+`  (precedence 25) — addition.
    PlusOp,
    /// `-`  (precedence 25) — subtraction.
    MinusOp,

    /// `*`  (precedence 26) — multiplication.
    ProductOp,
    /// `/`  (precedence 26) — division.
    DivisionOp,
    /// `%`  (precedence 26) — modulo.
    ModuloOp,

    /// `^`  (precedence 27) — exponentiation.
    PowerOp,

    /// `.`  (precedence 30) — sub-contexting: looks up the right-hand name
    /// inside the namespace produced by the left-hand expression.
    /// Example: `math.sin` retrieves `sin` from the `math` namespace.
    SubcontextingOp,

    /// `!` not operator
    NotOp,

    /// `$` lifting operator
    LiftingOp,

    // -------------------------------------------------------------------------
    // Special tokens
    // -------------------------------------------------------------------------

    /// A character sequence the lexer could not classify as any valid token.
    /// The slice covers exactly the unrecognised source characters.
    ///
    /// Examples: `$`, a malformed number like `12.` or `1e+`.
    Undefined(&'a str),

    /// A `#`-prefixed line comment.  The slice contains the comment text
    /// *after* the `#` and *before* the terminating `\n` (or end of source).
    ///
    /// > **Note**: the JS reference lexer silently discards comments.  The
    /// > Rust lexer yields them as tokens so that the parser (or a tool like
    /// > a formatter) can choose what to do with them.  The parser should
    /// > skip `Comment` tokens during expression parsing.
    Comment(&'a str),

    /// Signals that the source string has been fully consumed.
    ///
    /// `next_token` returns this sentinel instead of `Option::None` so that
    /// call-sites do not need to handle two distinct "no more input" states.
    /// The `Iterator` impl converts it to `None`.
    EndOfExpression,
}
````

## File: src/value.rs
````rust
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
        f.debug_struct("ScopeData")
            .field("bindings", &self.bindings)
            // Print whether a parent exists without recursively formatting the parent scope!
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
````

## File: Cargo.toml
````toml
[package]
name = "swan-core"
version = "0.1.0"
edition = "2024"

[dependencies]
````

## File: README.md
````markdown
# swan-core

A zero-copy, hand-written Rust implementation of the **Swan** expression language.

Swan is a small, dynamically-typed, expression-oriented language designed to be
embedded inside template engines and similar host applications. This crate
provides the lexer, parser (AST), and evaluator as a reusable library with no
external parser-generator dependencies.

> **Status**: the lexer is complete and tested. The parser, AST, value types,
> and evaluator modules are stubs under active development.

---

## The Swan Language — Quick Reference

A Swan expression is a sequence of binary operations:

```
operand  operator  operand  operator  operand  ...
```

Operands can be grouped with `(…)`, `[…]`, or `{…}`. Everything from `#` to
the end of the line is a comment and is ignored.

### Operators (highest to lowest precedence)

| Precedence | Operator | Description |
|:---:|---|---|
| 30 | `F X`  ·  `x.y` | Function application / sub-contexting |
| 27 | `x ^ y` | Exponentiation |
| 26 | `x * y`  ·  `x / y`  ·  `x % y` | Multiplicative arithmetic |
| 25 | `x + y`  ·  `x - y` | Additive arithmetic |
| 24 | `x == y`  ·  `x != y`  ·  `x < y`  ·  `x <= y`  ·  `x > y`  ·  `x >= y` | Comparison |
| 23 | `x & y`  ·  `x \| y` | Logical AND / OR |
| 22 | `x ? y` | Conditional: returns `y` when `x` is truthy |
| 21 | `x ; y` | Alternative: returns `x` unless it is `Undefined`, then returns `y` |
| 15 | `params -> body` | Function definition *(right-assoc)* |
| 14 | `f >> g`  ·  `g << f` | Pipe / Compose |
| 13 | `x => f` | Mapping |
| 12 | `name = x`  ·  `name: x` | Assignment / Labelling |
| 10 | `x , y` | Pairing (tuple construction) |

Unary `+x` and `-x` are also supported.

### Types

| Type | Literal / Example | Notes |
|---|---|---|
| `Nothing` | `()` | The unit type / empty tuple |
| `Bool` | `TRUE`, `FALSE` | Returned by comparisons |
| `Numb` | `42`, `3.14`, `1.5e-3` | IEEE-754 double |
| `Text` | `"hello"`, `'hello'` | String; indexable via apply |
| `List` | `[1, 2, 3]` | Ordered sequence; indexable via apply |
| `Namespace` | `{x=1, y=2}` | Name → value mapping; prototype inheritance |
| `Func` | `x -> x * 2` | First-class function; `self` for recursion |
| `Undefined` | *(result of invalid op)* | Operations never panic — they return `Undefined` |
| `Tuple` | `1, 2, "abc"` | Flat product type; not an `Item` |

### A few examples

```
# Arithmetic
(12 + 2) * 3 - 4          # => 38

# Function definition and application
double = x -> x * 2
double 5                  # => 10
double(5)                 # => 10  (parentheses are optional)

# Recursive factorial using `self`
fact = n -> n == 0 ? 1 ; n * self(n-1)
fact 6                    # => 720

# Namespace (records)
point = {x=3, y=4}
point.x                   # => 3   (sub-contexting)
point "y"                 # => 4   (apply)

# Conditional and alternative
x = -1
x > 0 ? "positive" ; "non-positive"   # => "non-positive"

# Composition
add1 = x -> x + 1
mul2 = x -> x * 2
add1 >> mul2               # same as x -> mul2(add1(x))
(add1 >> mul2) 3           # => 8
```

---

## Crate Architecture

```
swan-core
├── token.rs   — Token<'a> enum; all token kinds produced by the lexer
├── lexer.rs   — Zero-copy character scanner; implements Iterator<Item=(usize, Token)>
├── ast.rs     — Expression enum (AST node types) + Anchored<T> position wrapper
├── parser.rs  — [stub] Recursive-descent parser: tokens → AST
├── value.rs   — [stub] Runtime value representation
└── eval.rs    — [stub] Expression evaluator and scope management
```

### `token.rs`

Defines [`Token<'a>`](src/token.rs) — a lifetime-parameterised enum whose
`Identifier`, `Text`, `Comment`, and `Undefined` variants borrow slices
directly from the source string, avoiding heap allocations.

### `lexer.rs`

[`Lexer<'a>`](src/lexer.rs) is a hand-written character state machine.  It
implements `Iterator<Item=(usize, Token<'a>)>` where the `usize` is the byte
offset of each token in the source.

Key properties:
- **Zero-copy** — no `String` allocations during tokenisation.
- **UTF-8 aware** — `position` tracks byte offsets, safe for `&str` slicing.
- **Greedy two-char operators** — `==`, `!=`, `<=`, `>=`, `->`, `<<`, `>>`,
  `=>` are recognised before falling through to their single-char counterparts.

### `ast.rs`

Defines `Expression`, the recursive AST node enum, and `Anchored<T>`, a
wrapper that attaches a source position to any value.

### `parser.rs` *(stub)*

Will implement a shunting-yard recursive-descent parser that converts the
token stream into an `Expression` tree, honouring Swan's operator precedence
and associativity rules.

### `value.rs` *(stub)*

Will define the runtime `Value` type (`Bool`, `Numb`, `Text`, `List`,
`Namespace`, `Func`, `Undefined`, …).

### `eval.rs` *(stub)*

Will implement the evaluator: walks the `Expression` tree and produces a
`Value`, threaded through a scope chain.

---

## Getting Started

Add the crate as a path dependency while it is under development:

```toml
[dependencies]
swan-core = { path = "../swan-core" }
```

### Using the lexer today

```rust
use swan_core::lexer::Lexer;
use swan_core::token::Token;

fn main() {
    let source = r#"x = "hello""#;

    for (pos, token) in Lexer::new(source) {
        println!("{pos:>3}  {token:?}");
    }
}
```

Output:

```
  0  Identifier("x")
  2  AssignmentOp
  4  Text("hello")
```

---

## Development

```bash
# Compile the library
cargo build

# Run the test suite (unit tests + doctests)
cargo test

# Compile without producing artefacts (fast feedback)
cargo check

# Generate and open API documentation
cargo doc --open
```

---

## Reference Implementation

The JS reference implementation lives in `swan-js-repomix.md` (a single-file
snapshot of the original [SwanJS](https://github.com/onlabsorg/swan-js)
repository). The key files are:

| JS file | Rust equivalent |
|---|---|
| `lib/lexer.js` | `src/lexer.rs` |
| `lib/parser.js` | `src/parser.rs` |
| `lib/types.js` | `src/value.rs` |
| `lib/interpreter.js` | `src/eval.rs` |

---

## License

[MIT](https://opensource.org/licenses/MIT)
````
