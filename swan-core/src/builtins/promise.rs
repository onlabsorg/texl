//! The `promise` built-in namespace.
//!
//! Provides functions to create, chain, and resolve non-rejecting promises:
//! - `promise.create()` : returns a new pending Promise.
//! - `promise.chain p f` : curried; applies `f` to the value of promise `p` and returns a child promise.
//! - `promise.resolve p v` : curried; resolves promise `p` with value `v`.

use crate::value::{BuiltinFn, Promise, Scope, Value};

/// Builds the `promise` namespace as a child of `parent`.
pub fn create_namespace(parent: &Scope) -> Value {
    let promise_scope = parent.new_child();

    // Zero-arg constructor
    promise_scope.set("create", Value::Builtin(BuiltinFn::new("promise.create", promise_create)));

    // promise.resolve p v
    promise_scope.set("resolve", Value::Builtin(BuiltinFn::new("promise.resolve", promise_resolve)));

    Value::Namespace(promise_scope)
}

/// `promise.create()`: creates a new pending `Value::Promise`.
fn promise_create(_arg: Value) -> Value {
    Value::Promise(Promise::new())
}

/// `promise.resolve p`: given a promise, returns a function that takes the resolving
/// value `v`, resolves `p` with it, and returns `Nothing`.
fn promise_resolve(p: Value) -> Value {
    match p {
        Value::Promise(promise) => Value::Builtin(BuiltinFn::new(
            "promise.resolve",
            move |v| {
                promise.resolve(v);
                Value::Nothing
            },
        )),
        other => Value::undef_binary("promise.resolve", other, Value::Nothing, 0),
    }
}



// ----------------------------------------------------------------------------
//  Unit tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evaluator::eval;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    /// Root scope containing the `promise` namespace and boolean constants,
    /// matching the setup used in other builtin module tests.
    fn root_scope() -> Scope {
        let scope = Scope::new();
        scope.set("promise", create_namespace(&scope));
        scope.set("TRUE"   , Value::Bool(true));
        scope.set("FALSE"  , Value::Bool(false));
        scope.new_child()
    }

    /// Helper to lex, parse, and evaluate a Swan snippet in a fresh scope.
    fn run(src: &str) -> Value {
        let scope = root_scope();
        let mut parser = Parser::new(Lexer::new(src));
        let ast = parser.parse();
        eval(&ast, &scope)
    }

    #[test]
    fn test_create() {
        // promise.create() returns a Value::Promise
        assert!(matches!(run("promise.create()"), Value::Promise(_)));
    }


    #[test]
    fn test_resolve_undefined_maps_to_nothing() {
        // Swan promises resolve Undefined to Nothing
        let scope = root_scope();
        let src = r#"
            p = promise.create(),
            promise.resolve p (1 / "abc"),
            p
        "#;
        let mut parser = Parser::new(Lexer::new(src));
        let ast = parser.parse();
        let result = eval(&ast, &scope);

        if let Value::Promise(p) = result {
            assert_eq!(p.0.borrow().value, Some(Value::Nothing));
        } else {
            panic!("expected Value::Promise");
        }
    }

    #[test]
    fn test_non_promise_arguments_yield_undefined() {
        // Passing non-promise target operands yields Undefined binary errors
        assert!(matches!(run("promise.resolve 123"), Value::Undefined(_)));
    }

    #[test]
    fn test_partial_application_returns_function() {
        assert!(matches!(run("p = promise.create(), promise.resolve p"), Value::Builtin(_)));
    }

    #[test]
    fn test_resolve_with_tuple_value_not_mapped() {
        let scope = root_scope();
        let src = r#"
            p = promise.create(),
            promise.resolve p (1, 2),
            p
        "#;
        let mut parser = Parser::new(Lexer::new(src));
        let ast = parser.parse();
        let result = eval(&ast, &scope);

        if let Value::Promise(p) = result {
            let resolved_value = p.0.borrow().value.clone();
            assert!(matches!(resolved_value, Some(Value::Tuple(items)) if items.len() == 2));
        } else {
            panic!("expected Value::Promise");
        }
    }
}
