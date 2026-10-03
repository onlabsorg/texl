

//! Unary negation operation: `-X`.
//!
//! This module only handles the negation of a *single* value. Broadcasting over
//! `Nothing` and tuples is done by the caller (`evaluator::eval`) through the
//! `map_values!` macro, so `-()` gives `Nothing` and `-(1, 2)` gives `(-1, -2)`
//! without any tuple logic here.

use crate::value::Value;


/// `-X` — negates a single value.
///
/// - `Numb(n)` gives `Numb(-n)`.
/// - Any other value gives `Undefined(UnaryOperation { op: "negation", .. })`,
///   carrying the offending operand and the position `pos` of the operator.
///
/// `Nothing` and `Tuple` operands are expected to be handled by the caller
/// (see the module docs). If they are passed here directly they are treated like
/// any other non-numeric value and produce `Undefined`.
pub fn eval(val: Value, pos: usize) -> Value {
    match val {
        Value::Numb(n) => Value::Numb(-n),
        other => Value::undef_unary("negation", other, pos),
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
    use crate::value::{Scope, Undef};
    use crate::evaluator::eval;

    fn evaluate(src: &str) -> Value {
        let mut parser = Parser::new(Lexer::new(src));
        let ast = parser.parse();
        let scope = Scope::new();
        scope.set("TRUE", Value::Bool(true));
        scope.set("FALSE", Value::Bool(false));
        eval(&ast, &scope)
    }

    #[test]
    fn negate() {

        // Number
        assert_eq!(evaluate("-2"), Value::Numb(-2.0));
        assert_eq!(evaluate("-(-2)"), Value::Numb(2.0));

        // Nothing: passes through as Nothing
        assert_eq!(evaluate("-()"), Value::Nothing);

        // Tuple: negated item by item
        assert_eq!(evaluate("-(2, 3, 4)"), Value::Tuple(Rc::new(vec![
            Value::Numb(-2.0),
            Value::Numb(-3.0),
            Value::Numb(-4.0)
        ])));

        // Tuple with a non-numeric item: only that item becomes Undefined
        assert_eq!(evaluate("-(1, \"a\")"), Value::Tuple(Rc::new(vec![
            Value::Numb(-1.0),
            Value::undef_unary("negation", Value::Text("a".into()), 0),
        ])));

        // Other types: Undefined, carrying the operator, the operand and the position.
        // (`Undef`'s PartialEq ignores `pos`, so it is checked explicitly.)
        for (src, operand) in [
            ("-\"abc\"", Value::Text("abc".into())),
            ("-TRUE",    Value::Bool(true)),
            ("-[]",      Value::List(Rc::new(vec![]))),
        ] {
            match evaluate(src) {
                Value::Undefined(Undef::UnaryOperation { op, val, pos }) => {
                    assert_eq!(op, "negation");
                    assert_eq!(*val, operand);
                    assert_eq!(pos, 0);
                }
                other => panic!("expected Undefined::UnaryOperation for {src:?}, got {other:?}"),
            }
        }
    }
}
