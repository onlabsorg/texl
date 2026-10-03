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

use crate::value::{BuiltinFn, Scope, Value};
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::evaluator::eval;

// Shared helpers and macros (map_1, map_curried, ...).
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
/// use swan_core::evaluator::eval;
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
                    other => Value::undef_unary("eval", other, 0),
                }
            }))
        }
        other => Value::undef_unary("parse", other, 0),
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
