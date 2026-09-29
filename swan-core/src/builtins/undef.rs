
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
