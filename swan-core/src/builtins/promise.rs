//! The `promise` built-in namespace.
//!
//! Provides functions to create and resolve non-rejecting promises. A promise
//! is a single-assignment placeholder (`Value::Promise`) that starts out
//! pending and later receives a value.
//!
//! # Functions
//!
//! - `promise.create ()`   : returns a new pending promise. The argument is
//!   ignored; by convention it is `()` (`Nothing`).
//! - `promise.resolve p v` : curried. `promise.resolve p` returns a function
//!   that, when applied to `v`, resolves `p` with `v` and returns `Nothing`.
//!
//! # Semantics
//!
//! - Promises never reject. There is no error state.
//! - If the resolving value is `Undefined`, the promise is resolved with
//!   `Nothing` instead.
//! - Tuples are stored as-is. They are not mapped over or flattened, so
//!   `promise.resolve p (1, 2)` resolves `p` with the tuple `(1, 2)`.
//! - If the first argument to `promise.resolve` is not a promise, the result is
//!   `Undefined` (binary operation `promise.resolve`, with the offending value
//!   as left operand and `Nothing` as right operand). No function is returned.
//!
//! # Example
//!
//! ```text
//! p = promise.create (),
//! promise.resolve p 42,
//! p                        # a promise already resolved with 42
//! ```
//!
//! Note: chaining (`promise.chain`) is not provided by this module.

use crate::value::{BuiltinFn, Namespace, Promise, Value};

/// Builds the `promise` namespace value.
///
/// The returned `Value::Namespace` binds:
/// - `create`  -> `promise.create`  (see [`promise_create`])
/// - `resolve` -> `promise.resolve` (see [`promise_resolve`])
///
/// The caller is expected to bind the result under the name `promise`.
pub fn create_namespace() -> Value {
    let mut ns = Namespace::new();

    ns.insert("create", Value::Builtin(BuiltinFn::new("promise.create", promise_create)));

    ns.insert("resolve", Value::Builtin(BuiltinFn::new("promise.resolve", promise_resolve)));

    Value::Namespace(ns)
}

/// `promise.create ()`: creates a new pending `Value::Promise`.
///
/// The argument is ignored (conventionally `Nothing`, written `()`).
fn promise_create(_arg: Value) -> Value {
    Value::Promise(Promise::new())
}

/// `promise.resolve p`: the first stage of the curried `promise.resolve p v`.
///
/// - If `p` is a `Value::Promise`, returns a builtin function that takes the
///   resolving value `v`, resolves `p` with it, and returns `Nothing`.
///   An `Undefined` value `v` is stored as `Nothing`; tuples are stored
///   unchanged.
/// - Otherwise returns `Undefined` (binary operation `promise.resolve`,
///   with `p` as left operand and `Nothing` as right operand).
fn promise_resolve(p: Value) -> Value {
    match p {
        Value::Promise(promise) => Value::Builtin(BuiltinFn::capturing(
            "promise.resolve",
            vec![Value::Promise(promise)],
            |captured, v| {
                if let Value::Promise(promise) = &captured[0] {
                    promise.resolve(v);
                }
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
    use crate::value::Scope;
    use crate::evaluator::eval;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    /// Root scope containing the `promise` namespace and boolean constants,
    /// matching the setup used in other builtin module tests.
    fn root_scope() -> Scope {
        let scope = Scope::new();
        scope.set("promise", create_namespace());
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
