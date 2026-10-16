//! Evaluation of the `then` operator (`~>`).
//!
//! `P ~> F` chains the function `F` onto the promise `P` and returns a new
//! child promise.
//!
//! # Semantics
//!
//! - **Promise target:** `F` is registered as a continuation of `P`.
//!   - If `P` is already resolved, `F` runs immediately.
//!   - Otherwise it runs when `P` resolves.
//!   - The child promise resolves with the result of applying `F` to `P`'s
//!     value, using the normal function-application rules (`eval_apply`).
//! - **Multiple children:** one promise may be chained several times. Each
//!   chained function gets its own independent child promise.
//! - **Tuple of functions:** `P ~> (F, G)` chains each function and returns a
//!   tuple of child promises, one per function.
//! - **Tuple of promises:** `(P, Q) ~> F` chains `F` onto each promise and
//!   yields a tuple of child promises.
//! - **Undefined results:** a Swan promise never rejects. If a chained
//!   function returns an `Undefined` value, the child promise resolves to
//!   `Nothing` instead.
//! - **Non-promise target:** a value that is not a promise is wrapped in an
//!   already resolved promise, so `3 ~> F` is a promise of `F 3`.
//!
//! # Examples
//!
//! ```text
//! p = promise.create(),
//! c = p ~> (x -> x + 1),   # c is a new, pending promise
//! promise.resolve p 1,     # c now resolves to 2
//! ```

use crate::value::{Promise, Value};


/// Evaluates `target ~> func`.
///
/// # Arguments
///
/// * `target` - The value on the left of `~>`. A value that is not a
///   `Value::Promise` is wrapped in an already resolved promise.
/// * `func`   - The function, or tuple of functions, on the right of `~>`.
///   Tuples are iterated with `Value::items`, so each contained function is
///   chained separately.
/// * `pos`    - Source position, used for error reporting and passed to
///   function application.
///
/// # Returns
///
/// One child promise per function in `func`, collected with `Value::tuple`.
/// A single function yields a single child promise; several functions yield a
/// tuple of child promises.
pub fn eval(target: Value, func: Value, pos: usize) -> Value {
    // If the target is already a promise, use it. 
    // Otherwise, wrap the synchronous value in an immediately resolved promise.
    let promise = match target {
        Value::Promise(p) => p,
        other => {
            let p = Promise::new();
            p.resolve(other);
            p
        }
    };

    // Chain the promise once per function. A single function yields a single
    // child promise; a tuple of functions yields a tuple of child promises.
    let children: Vec<Value> = func
        .items()
        .map(|f| promise.then_apply(f, pos))
        .collect();
    
    Value::tuple(children)
}




// ----------------------------------------------------------------------------
//  Unit tests
// ----------------------------------------------------------------------------

#[cfg(test)]
/// Tests for the `~>` operator, covering:
/// - basic chaining and non-promise error handling
/// - resolve-then-chain and chain-then-resolve ordering
/// - multiple and nested children
/// - `Nothing` and `Undefined` results
/// - tuple arguments, tuple targets, and asynchronous assignment via `bind`
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;
    use crate::evaluator::eval;
    use crate::builtins::root_scope;
    // Removed `Undef` from imports since it is no longer used here

    /// Lex, parse and evaluate `src` in a fresh root scope.
    fn run(src: &str) -> Value {
        let scope = root_scope();
        let mut parser = Parser::new(Lexer::new(src));
        let ast = parser.parse();
        eval(&ast, &scope)
    }

    /// Returns the resolved value of a `Value::Promise` (`None` if pending). Panics for non-promises.
    fn resolved(v: &Value) -> Option<Value> {
        match v {
            Value::Promise(p) => p.0.borrow().value.clone(),
            other => panic!("expected Value::Promise, got {:?}", other),
        }
    }

    /// Unwraps a `Value::Tuple` into a `Vec<Value>`. Panics for anything else.
    fn tuple_items(v: Value) -> Vec<Value> {
        match v {
            Value::Tuple(items) => items.iter().cloned().collect(),
            other => panic!("expected Value::Tuple, got {:?}", other),
        }
    }

    #[test]
    fn test_basics() {
        // (a) `P ~> F` returns a new promise.
        let p = run("p = promise.create(), p");
        let c = super::eval(p.clone(), run("x -> x + 1"), 0);
        assert!(matches!(c, Value::Promise(_)));

        // (b) `P ~> F` wraps non-promises in an immediately resolved promise and chains the function.
        let c2 = run("123 ~> (x -> x * 2)");
        assert_eq!(resolved(&c2), Some(Value::Numb(246.0)));
    }

    #[test]
    fn test_resolve_then_chain() {
        // (c) A promise resolves, then a new child is chained.
        let src = r#"
            p = promise.create(),
            promise.resolve p 10,
            p ~> (x -> x * 2)
        "#;
        let c = run(src);
        assert_eq!(resolved(&c), Some(Value::Numb(20.0)));
    }

    #[test]
    fn test_chain_then_resolve() {
        // (d) A child is chained, then the parent resolves.
        let src = r#"
            p = promise.create(),
            c = p ~> (x -> x + 5),
            promise.resolve p 20,
            c
        "#;
        let c = run(src);
        assert_eq!(resolved(&c), Some(Value::Numb(25.0)));
    }

    #[test]
    fn test_chain_multiple_children() {
        // (e) Multiple children promises
        let src = r#"
            p = promise.create(),
            c1 = p ~> (x -> x + 10),
            c2 = p ~> (x -> x * 2),
            promise.resolve p 5,
            (c1, c2)
        "#;
        let children = tuple_items(run(src));
        assert_eq!(resolved(&children[0]), Some(Value::Numb(15.0)));
        assert_eq!(resolved(&children[1]), Some(Value::Numb(10.0)));
    }

    #[test]
    fn test_chain_nested_children() {
        // (f) Nested children promises
        let src = r#"
            p = promise.create(),
            c1 = p ~> (x -> x + 10),
            c2 = c1 ~> (x -> x * 2),
            promise.resolve p 5,
            c2
        "#;
        let c2 = run(src);
        assert_eq!(resolved(&c2), Some(Value::Numb(30.0)));
    }

    #[test]
    fn test_chain_with_nothing() {
        // (g) Chaining a function that produces Nothing
        let src = r#"
            p = promise.create(),
            c = p ~> (x -> ()),
            promise.resolve p 5,
            c
        "#;
        let c = run(src);
        assert_eq!(resolved(&c), Some(Value::Nothing));
    }

    #[test]
    fn test_chain_with_undefined() {
        // (h) Chaining a function that produces Undefined (becomes Nothing)
        let src = r#"
            p = promise.create(),
            c = p ~> (x -> 1 / "a"),
            promise.resolve p 5,
            c
        "#;
        let c = run(src);
        assert_eq!(resolved(&c), Some(Value::Nothing));
    }

    #[test]
    fn test_chain_with_tuple_argument_to_func() {
        // (i) Chaining with a tuple passed to the function
        let src = r#"
            p = promise.create(),
            c = p ~> ((x,y) -> x + y),
            promise.resolve p (1,2),
            c
        "#;
        let c = run(src);
        assert_eq!(resolved(&c), Some(Value::Numb(3.0)));
    }

    #[test]
    fn test_tuple_promise_chain() {
        // (j) Tuple of promises chained by one function
        let src = r#"
            p1 = promise.create(),
            p2 = promise.create(),
            c = (p1, p2) ~> (x -> x * 2),
            promise.resolve p1 1,
            promise.resolve p2 2,
            c
        "#;
        let c = run(src);
        let items = tuple_items(c);
        assert_eq!(resolved(&items[0]), Some(Value::Numb(2.0)));
        assert_eq!(resolved(&items[1]), Some(Value::Numb(4.0)));
    }

    #[test]
    fn asynchronous_assignment() {
        let src = r#"
            p1 = promise.create(),
            p2 = p1 ~> bind "x",
            promise.resolve p1 10,
            x
        "#;
        assert_eq!(run(src), Value::Numb(10.0));
    }
}
