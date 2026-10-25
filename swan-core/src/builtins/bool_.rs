//! The `bool` built-in namespace.
//!
//! Provides:
//! - `bool.TRUE` / `bool.FALSE`: the boolean constants.
//! - `bool.from`: converts a value to a `Bool`, element-wise over tuples.
//! - `bool.any`:  `true` if at least one item of its argument is truthy.
//! - `bool.all`:  `true` if every item of its argument is truthy.
//!
//! Truthiness itself is defined by `impl From<Value> for bool`.
use crate::value::{BuiltinFn, Namespace, Value};
use super::helpers::*;



// ----------------------------------------------------------------------------
//  `bool` built-in namespace
// ----------------------------------------------------------------------------

/// Builds the `bool` namespace value.
///
/// `from` is wrapped with `map_1`, so it maps over tuples and returns a tuple
/// of booleans. `any` and `all` are plain `BuiltinFn`s because they reduce
/// their whole argument to a single `Bool`.
pub fn create_namespace() -> Value {
    let mut ns = Namespace::new();
    ns.insert("TRUE" , Value::Bool(true));
    ns.insert("FALSE", Value::Bool(false));
    ns.insert("from" , map_1("bool.from", bool_from));
    ns.insert("any"  , Value::Builtin(BuiltinFn::new("bool.any", bool_any)));
    ns.insert("all"  , Value::Builtin(BuiltinFn::new("bool.all", bool_all)));
    Value::Namespace(ns)
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
    use crate::value::Scope;
    use crate::evaluator::eval;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    /// Minimal root scope: the `bool` namespace plus the global boolean
    /// constants. Returns a child scope, mirroring how user code is evaluated.
    fn root_scope() -> Scope {
        let scope = Scope::new();
        scope.set("bool" , create_namespace());
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
            Value::tuple(vec![Value::Bool(true), Value::Bool(false)]),
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

    #[test]
    fn test_promise() {
        let scope = root_scope();
        scope.set("promise", crate::builtins::promise::create_namespace());
        let run_p = |src: &str| eval(&Parser::new(Lexer::new(src)).parse(), &scope);

        // Pending: FALSE
        assert_eq!(run_p("p = promise.create(), bool.from p"), Value::Bool(false));
        assert_eq!(run_p("bool.any(promise.create(), 0)"),     Value::Bool(false));
        assert_eq!(run_p("bool.all(promise.create(), 1)"),     Value::Bool(false));

        // Resolved: TRUE, even when the value is false
        assert_eq!(run_p("promise.resolve p FALSE, bool.from p"), Value::Bool(true));
        assert_eq!(run_p("bool.any(p, 0)"),                       Value::Bool(true));
        assert_eq!(run_p("bool.all(p, 1)"),                       Value::Bool(true));
    }
}
