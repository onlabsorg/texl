//! Unary lifting operation: `$X`.
//!
//! Lifting resolves a name starting from the *outermost* scope of the current
//! scope chain instead of the innermost one. It reads a binding that has been
//! shadowed by a nearer scope (see [`Scope::lift`]).
//!
//! The operand of `$` is not evaluated as an expression. The evaluator turns it
//! into a name, a tuple of names or `Nothing` with `eval_names`.
//! Tuple and `Nothing` operands are broadcast by `map_values!` in the parent
//! module, so this module only handles a single item at a time.

use crate::value::{Scope, Value};

/// Lifts a single name out of `scope`.
///
/// - `name`: the operand, already resolved to a [`Value`] by `eval_names`.
/// - `scope`: the scope in which the `$` expression is evaluated.
/// - `pos`: source position of the `$` operator, used for error reporting.
///
/// Returns:
/// - for `Value::Name(n)`: the value of `n` found by [`Scope::lift`];
/// - for anything else (typically an `Undefined(Undef::Name)` produced by
///   `eval_names` for an invalid operand such as `$12`): an
///   `Undefined(Undef::UnaryOperation)` with `op: "lifting"`, which wraps the
///   offending value.
pub fn eval(name: Value, scope: &Scope, pos: usize) -> Value {
    match name {
        // A valid name: look it up starting from the outermost scope.
        Value::Name(n) => scope.lift(&n),
        // Not a name: report it as an undefined lifting operation.
        other => Value::undef_unary("lifting", other, pos),
    }
}



// ----------------------------------------------------------------------------
//  UNIT TESTS
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::rc::Rc;
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;
    use crate::evaluator::eval;
    use crate::value::Undef;

    fn evaluate(src: &str) -> Value {
        let mut parser = Parser::new(Lexer::new(src));
        let ast = parser.parse();
        let scope = Scope::new();
        scope.set("TRUE", Value::Bool(true));
        scope.set("FALSE", Value::Bool(false));
        eval(&ast, &scope)
    }

    #[test]
    fn lifting() {
        // Name
        assert_eq!(evaluate("{x=1}.{x=2}.x"), Value::Numb(2.0));
        assert_eq!(evaluate("{x=1}.{x=2}.$x"), Value::Numb(1.0));

        // Nothing
        assert_eq!(evaluate("{x=1}.{x=2}.$()"), Value::Nothing);

        // Tuple
        assert_eq!(
            evaluate("{x=1}.{x=2,y=3}.$(x,y)"),
            Value::Tuple(Rc::new(vec![
                Value::Numb(1.0),
                Value::Numb(3.0),
            ]))
        );

        // Invalid operand: not a name, so the result is an Undefined lifting operation
        // that wraps the Undefined name reported by `eval_names`.
        match evaluate("$12") {
            Value::Undefined(Undef::UnaryOperation { op, val, pos }) => {
                assert_eq!(op, "lifting");
                assert!(matches!(*val, Value::Undefined(Undef::Name { .. })));
                assert_eq!(pos, 0);
            }
            other => panic!("expected Undefined::UnaryOperation, got {:?}", other),
        }
    }
}
