
//! The `undef` built-in namespace.
//!
//! Provides helpers for explicitly constructing `Value::Undefined` values
//! from user code.
//!
//! | Member       | Description                                              |
//! |--------------|----------------------------------------------------------|
//! | `undef.from` | Wraps any value into `Undefined(Undef::Value(<value>))`. |

use crate::value::{BuiltinFn, Namespace, Undef, Value};



// ----------------------------------------------------------------------------
//  `undef` built-in namespace
// ----------------------------------------------------------------------------

/// Builds the `undef` namespace value, with `from` and `inspect`.
pub fn create_namespace() -> Value {
    let mut ns = Namespace::new();
    ns.insert("from"   , Value::Builtin(BuiltinFn::new("undef.from"   , undef_from)));
    ns.insert("inspect", Value::Builtin(BuiltinFn::new("undef.inspect", undef_inspect)));
    Value::Namespace(ns)
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
        other => Value::undef_unary("undef.inspect", other, 0),
    }
}

/// Helper function to convert an `Undef` enum into a `Value::Namespace`,
/// recursively processing any nested `Value` operands.
fn inspect_undef(undef: &Undef) -> Value {
    let mut ns = Namespace::new();

    match undef {
        Undef::UnaryOperation { op, val, pos } => {
            ns.insert("type", Value::Text("UnaryOperation".into()));
            ns.insert("op", Value::Text((*op).into()));
            ns.insert("val", inspect_value(val));
            ns.insert("pos", Value::Numb(*pos as f64));
        }
        Undef::BinaryOperation { op, lhv, rhv, pos } => {
            ns.insert("type", Value::Text("BinaryOperation".into()));
            ns.insert("op", Value::Text((*op).into()));
            ns.insert("lhv", inspect_value(lhv));
            ns.insert("rhv", inspect_value(rhv));
            ns.insert("pos", Value::Numb(*pos as f64));
        }
        Undef::Termination { expected, pos } => {
            ns.insert("type", Value::Text("Termination".into()));
            ns.insert("expected", Value::Text(expected.to_string().into()));
            ns.insert("pos", Value::Numb(*pos as f64));
        }
        Undef::Token { token, pos } => {
            ns.insert("type", Value::Text("Token".into()));
            ns.insert("token", Value::Text(token.as_str().into()));
            ns.insert("pos", Value::Numb(*pos as f64));
        }
        Undef::Name { pos } => {
            ns.insert("type", Value::Text("Name".into()));
            ns.insert("pos", Value::Numb(*pos as f64));
        }
        Undef::Value(val) => {
            ns.insert("type", Value::Text("Value".into()));
            ns.insert("val", inspect_value(val));
        }
    }

    Value::Namespace(ns)
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
    use super::*;
    use crate::value::Scope;
    use crate::evaluator::eval;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn root_scope() -> Scope {
        let scope = Scope::new();
        scope.set("undef", create_namespace());
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
            Value::tuple(vec![
                Value::Text("Value".into()),
                Value::Numb(42.0),
            ])
        );

        // Inspecting an invalid operation (e.g. UnaryOperation)
        assert_eq!(
            run("info = undef.inspect(- \"abc\"), info.type, info.op, info.val, info.pos"),
            Value::tuple(vec![
                Value::Text("UnaryOperation".into()),
                Value::Text("negation".into()),
                Value::Text("abc".into()),
                Value::Numb(21.0),
            ])
        );

        // Recursive inspection of nested Undefined values
        assert_eq!(
            run("info = undef.inspect(undef.from(undef.from 42)), info.val.type, info.val.val"),
            Value::tuple(vec![
                Value::Text("Value".into()),
                Value::Numb(42.0),
            ])
        );

        // Recursive inspection inside Tuples (destructure the tuple to reach its items)
        assert_eq!(
            run("info = undef.inspect(undef.from(1, undef.from 2)), (a, b) = info.val, b.type, b.val"),
            Value::tuple(vec![
                Value::Text("Value".into()),
                Value::Numb(2.0),
            ])
        );

        // Non-undefined argument yields Undefined
        assert!(matches!(run("undef.inspect 123"), Value::Undefined(_)));
    }
}
