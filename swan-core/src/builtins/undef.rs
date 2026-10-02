
//! The `undef` built-in namespace.
//!
//! Provides helpers for explicitly constructing `Value::Undefined` values
//! from user code.
//!
//! | Member       | Description                                              |
//! |--------------|----------------------------------------------------------|
//! | `undef.from` | Wraps any value into `Undefined(Undef::Value(<value>))`. |

use crate::value::{BuiltinFn, Scope, Undef, Value};
use super::helpers::*;



// ----------------------------------------------------------------------------
//  `undef` built-in namespace
// ----------------------------------------------------------------------------

/// Builds the `undef` namespace as a child of `parent`.
///
/// The child scope keeps the parent's bindings visible from inside the
/// namespace. It currently contains only `from`.
pub fn create_namespace(parent: &Scope) -> Value {
    let undef_scope = parent.new_child();
    undef_scope.set("from"   , Value::Builtin(BuiltinFn::new("undef.from"   , undef_from)));
    undef_scope.set("inspect", Value::Builtin(BuiltinFn::new("undef.inspect", undef_inspect)));
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


/// `undef.inspect U` — converts an `Undefined` value into a namespace containing
/// its details (`type`, `pos`, etc.), converting any nested `Undefined` values
/// recursively into namespaces.
///
/// Returns `Undefined` if the argument is not a `Value::Undefined`.
fn undef_inspect(val: Value) -> Value {
    match val {
        Value::Undefined(undef) => inspect_undef(&undef),
        other => undef_unary!("undef.inspect", other),
    }
}

/// Helper function to convert an `Undef` enum into a `Value::Namespace`,
/// recursively processing any nested `Value` operands.
fn inspect_undef(undef: &Undef) -> Value {
    let scope = Scope::new();

    match undef {
        Undef::UnaryOperation { op, val, pos } => {
            scope.set("type", Value::Text("UnaryOperation".into()));
            scope.set("op", Value::Text((*op).into()));
            scope.set("val", inspect_value(val));
            scope.set("pos", Value::Numb(*pos as f64));
        }
        Undef::BinaryOperation { op, lhv, rhv, pos } => {
            scope.set("type", Value::Text("BinaryOperation".into()));
            scope.set("op", Value::Text((*op).into()));
            scope.set("lhv", inspect_value(lhv));
            scope.set("rhv", inspect_value(rhv));
            scope.set("pos", Value::Numb(*pos as f64));
        }
        Undef::Termination { expected, pos } => {
            scope.set("type", Value::Text("Termination".into()));
            scope.set("expected", Value::Text(expected.to_string().into()));
            scope.set("pos", Value::Numb(*pos as f64));
        }
        Undef::Token { token, pos } => {
            scope.set("type", Value::Text("Token".into()));
            scope.set("token", Value::Text(token.as_str().into()));
            scope.set("pos", Value::Numb(*pos as f64));
        }
        Undef::Name { pos } => {
            scope.set("type", Value::Text("Name".into()));
            scope.set("pos", Value::Numb(*pos as f64));
        }
        Undef::Value(val) => {
            scope.set("type", Value::Text("Value".into()));
            scope.set("val", inspect_value(val));
        }
    }

    Value::Namespace(scope)
}

/// Recursively processes a `Value`. If it is a `Value::Undefined`, it converts it
/// to a namespace via `inspect_undef`. If it is a `Value::Tuple`, it recursively
/// converts any contained `Undefined` items. Other values remain unchanged.
fn inspect_value(val: &Value) -> Value {
    match val {
        Value::Undefined(u) => inspect_undef(u),
        Value::Tuple(items) => {
            let mapped: Vec<Value> = items.iter().map(inspect_value).collect();
            Value::Tuple(std::rc::Rc::new(mapped))
        }
        other => other.clone(),
    }
}


// ----------------------------------------------------------------------------
//  UNIT TESTS
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::rc::Rc;
    use super::*;
    use crate::evaluator::eval;
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

    #[test]
    fn test_undef_inspect() {
        // Inspecting Undef::Value
        assert_eq!(
            run("info = undef.inspect(undef.from 42), info.type, info.val"),
            Value::Tuple(Rc::new(vec![
                Value::Text("Value".into()),
                Value::Numb(42.0),
            ]))
        );

        // Inspecting an invalid operation (e.g. UnaryOperation)
        assert_eq!(
            run("info = undef.inspect(- \"abc\"), info.type, info.op, info.val, info.pos"),
            Value::Tuple(Rc::new(vec![
                Value::Text("UnaryOperation".into()),
                Value::Text("negation".into()),
                Value::Text("abc".into()),
                Value::Numb(21.0),
            ]))
        );

        // Recursive inspection of nested Undefined values
        assert_eq!(
            run("info = undef.inspect(undef.from(undef.from 42)), info.val.type, info.val.val"),
            Value::Tuple(Rc::new(vec![
                Value::Text("Value".into()),
                Value::Numb(42.0),
            ]))
        );

        // Recursive inspection inside Tuples (destructure the tuple to reach its items)
        assert_eq!(
            run("info = undef.inspect(undef.from(1, undef.from 2)), (a, b) = info.val, b.type, b.val"),
            Value::Tuple(Rc::new(vec![
                Value::Text("Value".into()),
                Value::Numb(2.0),
            ]))
        );

        // Non-undefined argument yields Undefined
        assert!(matches!(run("undef.inspect 123"), Value::Undefined(_)));
    }
}
