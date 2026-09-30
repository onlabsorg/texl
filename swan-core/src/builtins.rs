// src/builtins.rs
//
// Built-in functions and namespaces that are pre-loaded into the root scope.
//
// # How to add a new built-in function
//
//   1. Write a plain Rust function `fn my_fn(arg: Value) -> Value { ... }`.
//   2. Wrap it: `Value::Builtin(BuiltinFn::new("my_fn", my_fn))`.
//   3. `scope.set("my_fn", ...)` in `root_scope()`, or insert it into one of
//      the namespace builders (`math_namespace()`, etc.).
//
// # Multi-argument functions / Curried Higher-Order Functions
//
//   Functions of type f(x, y) that require two parameters are implemented as
//   higher-order functions f(x) that return a g(y) function.
//   For example: `pow10 = math.pow 10`, `pow10 2` returns 1024.
//
// # Error convention
//
//   Never panic. Return `Value::Undefined(Undef::UnaryOperation { ... })` for
//   type errors. Use the function's own `&'static str` name as the `op` field
//   so the caller can identify the source of the error.

use crate::value::{BuiltinFn, Scope, Undef, Value};
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::eval::eval;

mod helpers;
use helpers::*;

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
    let scope = Scope::new();

    // --- Builtin Namespaces ---
    scope.set("bool"  , bool_::create_namespace(&scope));
    scope.set("numb"  , numb::create_namespace(&scope));
    scope.set("text"  , text::create_namespace(&scope));
    scope.set("list"  , list::create_namespace(&scope));
    scope.set("nspace", nspace::create_namespace(&scope));
    scope.set("undef" , undef::create_namespace(&scope));
    scope.set("math"  , math::create_namespace(&scope));
    scope.set("time"  , time::create_namespace(&scope));

    // --- Builtin Constants ---
    scope.set("TRUE" , Value::Bool(true));
    scope.set("FALSE", Value::Bool(false));

    // --- Builtin Functions ---
    scope.set("type" , Value::Builtin(BuiltinFn::new("type" , root_type)));
    scope.set("parse", Value::Builtin(BuiltinFn::new("parse", root_parse)));

    // --- Return empty child ---
    scope.new_child()
}





// ----------------------------------------------------------------------------
//  built-in functions
// ----------------------------------------------------------------------------

fn root_type(val: Value) -> Value {
    match val {
        Value::Nothing => Value::Nothing,
        Value::Tuple(items) => {
            let types: Vec<Value> = items
                .iter()
                .map(|it| Value::Text(type_name(it).into()))
                .collect();
            normalize_tuple(types)
        }
        other => Value::Text(type_name(&other).into()),
    }
}

fn type_name(val: &Value) -> &'static str {
    match val {
        Value::Nothing => "Nothing",
        Value::Bool(_) => "Bool",
        Value::Numb(_) => "Numb",
        Value::Text(_) => "Text",
        Value::List(_) => "List",
        Value::Namespace(_) => "Namespace",
        Value::Func { .. } | Value::Builtin(_) => "Func",
        Value::Undefined(_) => "Undefined",
        Value::Tuple(_) => "Tuple",
        val => panic!("Unknown value type: {:?}", val),
    }
}

/// Builtin `parse` function: takes a string expression `S` and returns an `eval` function.
fn root_parse(arg: Value) -> Value {
    match arg {
        Value::Text(source) => {
            let mut parser = Parser::new(Lexer::new(&source));
            let ast = parser.parse();

            // Return an `eval` function capturing the parsed AST node
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
    use crate::eval::eval;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn run(src: &str) -> Value {
        let scope = root_scope();
        let mut parser = Parser::new(Lexer::new(src));
        let ast = parser.parse();
        eval(&ast, &scope)
    }

    #[test]
    fn test_type() {
        assert_eq!(run("type(1 == 1)"), Value::Text("Bool".into()));
        assert_eq!(run("type 10"), Value::Text("Numb".into()));
        assert_eq!(run(r#"type "abc""#), Value::Text("Text".into()));
        assert_eq!(run("type [1, 2]"), Value::Text("List".into()));
        assert_eq!(run("type {a: 1}"), Value::Text("Namespace".into()));
        assert_eq!(run("type(x -> x)"), Value::Text("Func".into()));
        assert_eq!(run("type()"), Value::Nothing);
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
        // Basic dynamic evaluation with scope
        assert_eq!(
            run(r#"eval_fn = parse "a + b", eval_fn { a = 10, b = 20 }"#),
            Value::Numb(30.0)
        );

        // Dynamic evaluation returning a function/closure
        assert_eq!(
            run(r#"double = (parse "x -> x * 2") {}, double 5"#),
            Value::Numb(10.0)
        );

        // Invalid argument type returns Undefined
        assert!(matches!(run("parse 123"), Value::Undefined(_)));
    }
}
