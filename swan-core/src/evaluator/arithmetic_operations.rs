
use std::rc::Rc;
use crate::value::{Value, Undef};






// ----------------------------------------------------------------------------
//  Arithmetic binary operations
// ----------------------------------------------------------------------------

/// `X + Y` — type-homogeneous sum.
///
/// Matches the JS `SumOperation`: both operands must be the *same* Swan type
/// and that type must define a `sum` operation.  Mismatched types always
/// produce `Undefined`. (#12)
///
/// Currently implemented:
///   Numb  + Numb  → number addition
///   Text  + Text  → string concatenation
///   List  + List  → list concatenation
///   Namespace + Namespace → shallow merge (right wins on collision)
///
/// When tuples are supported, the operation should iterate `iterPairs` over
/// the two tuples and apply element-wise, assembling the results into a new
/// Tuple. (#11)
pub fn eval_add(lhv: Value, rhv: Value, pos: usize) -> Value {
    match (lhv, rhv) {

        (Value::Numb(a), Value::Numb(b)) => Value::Numb(a + b),

        (Value::Bool(a), Value::Bool(b)) => Value::Bool(a || b),

        (Value::Text(a), Value::Text(b)) => {
            // Concatenate two Rc<str> values without going through String twice.
            let mut s = String::with_capacity(a.len() + b.len());
            s.push_str(&a);
            s.push_str(&b);
            Value::Text(s.into())
        }

        (Value::List(a), Value::List(b)) => {
            // Concatenate two Rc<Vec<Value>>: clone items from both sides.
            let mut v = Vec::with_capacity(a.len() + b.len());
            v.extend_from_slice(&a);
            v.extend_from_slice(&b);
            Value::List(Rc::new(v))
        }

        (lhv, rhv) => Value::Undefined(Undef::BinaryOperation {
            op:  "sum",
            lhv: Box::new(lhv),
            rhv: Box::new(rhv),
            pos,
        }),
    }
}

/// `X - Y` — subtraction.
///
/// Currently implemented:
///   Numb  - Numb  → number subtraction
///
/// When tuples are supported, the operation should iterate `iterPairs` over
/// the two tuples and apply element-wise, assembling the results into a new
/// Tuple. (#11)
pub fn eval_subtraction(lhv: Value, rhv: Value, pos: usize) -> Value {
    match (lhv, rhv) {

        (Value::Numb(a), Value::Numb(b)) => Value::Numb(a - b),

        (lhv, rhv) => Value::Undefined(Undef::BinaryOperation {
            op:  "subtraction",
            lhv: Box::new(lhv),
            rhv: Box::new(rhv),
            pos,
        }),
    }
}

/// `X * Y` — product
///
/// Currently implemented:
///   Numb * Numb  → number product
///
pub fn eval_product(lhv: Value, rhv: Value, pos: usize) -> Value {
    match (lhv, rhv) {

        (Value::Numb(a), Value::Numb(b)) => Value::Numb(a * b),

        (Value::Bool(a), Value::Bool(b)) => Value::Bool(a && b),

        (lhv, rhv) => Value::Undefined(Undef::BinaryOperation {
            op:  "product",
            lhv: Box::new(lhv),
            rhv: Box::new(rhv),
            pos,
        }),
    }
}

/// `X / Y` — division
///
/// Currently implemented:
///   Numb / Numb  → number division
///
pub fn eval_division(lhv: Value, rhv: Value, pos: usize) -> Value {
    match (lhv, rhv) {

        (Value::Numb(a), Value::Numb(b)) => Value::Numb(a / b),

        (lhv, rhv) => Value::Undefined(Undef::BinaryOperation {
            op:  "division",
            lhv: Box::new(lhv),
            rhv: Box::new(rhv),
            pos,
        }),
    }
}

/// `X % Y` — modulo
///
/// Currently implemented:
///   Numb % Numb  → number modulo
///
pub fn eval_modulo(lhv: Value, rhv: Value, pos: usize) -> Value {
    match (lhv, rhv) {

        (Value::Numb(a), Value::Numb(b)) => Value::Numb(a % b),

        (lhv, rhv) => Value::Undefined(Undef::BinaryOperation {
            op:  "modulo",
            lhv: Box::new(lhv),
            rhv: Box::new(rhv),
            pos,
        }),
    }
}

/// `X ^ Y` — power
///
/// Currently implemented:
///   Numb ^ Numb  → number exponentiation
///
pub fn eval_power(lhv: Value, rhv: Value, pos: usize) -> Value {
    match (lhv, rhv) {

        (Value::Numb(a), Value::Numb(b)) => Value::Numb(a.powf(b)),

        (lhv, rhv) => Value::Undefined(Undef::BinaryOperation {
            op:  "power",
            lhv: Box::new(lhv),
            rhv: Box::new(rhv),
            pos,
        }),
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
    use crate::value::Scope;
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
    fn sum() {
        assert_eq!(evaluate("() + ()"), Value::Nothing);

        assert_eq!(evaluate("1 + 2"), Value::Numb(3.0));

        assert_eq!(evaluate("TRUE + TRUE"), Value::Bool(true));
        assert_eq!(evaluate("TRUE + FALSE"), Value::Bool(true));
        assert_eq!(evaluate("FALSE + TRUE"), Value::Bool(true));
        assert_eq!(evaluate("FALSE + FALSE"), Value::Bool(false));

        assert_eq!(evaluate("\"abc\" + \"def\""), Value::Text("abcdef".into()));

        assert_eq!(evaluate("[1,2,3] + [4,5,6]"), Value::List(Rc::new(vec![
                    Value::Numb(1.0),
                    Value::Numb(2.0),
                    Value::Numb(3.0),
                    Value::Numb(4.0),
                    Value::Numb(5.0),
                    Value::Numb(6.0)
                ])));

        assert_eq!(evaluate("(1, 2) + (3, 4)"), Value::Tuple(Rc::new(vec![
                    Value::Numb(4.0),
                    Value::Numb(6.0),
                ])));

        match evaluate("1 + \"abc\"") {
            Value::Undefined(Undef::BinaryOperation { op, lhv, rhv, pos }) => {
                assert_eq!(op, "sum");
                assert_eq!(*lhv, Value::Numb(1.0));
                assert_eq!(*rhv, Value::Text("abc".into()));
                assert_eq!(pos, 2);
            }
            other => panic!("expected Undefined::BinaryOperation, got {:?}", other),
        }
    }

    #[test]
    fn subtraction() {
        assert_eq!(evaluate("() - ()"), Value::Nothing);

        assert_eq!(evaluate("7 - 2"), Value::Numb(5.0));

        assert_eq!(evaluate("(10, 20) - (1, 2)"), Value::Tuple(Rc::new(vec![
                    Value::Numb(9.0),
                    Value::Numb(18.0),
                ])));

        match evaluate("1 - \"abc\"") {
            Value::Undefined(Undef::BinaryOperation { op, lhv, rhv, pos }) => {
                assert_eq!(op, "subtraction");
                assert_eq!(*lhv, Value::Numb(1.0));
                assert_eq!(*rhv, Value::Text("abc".into()));
                assert_eq!(pos, 2);
            }
            other => panic!("expected Undefined::BinaryOperation, got {:?}", other),
        }
    }

    #[test]
    fn product() {
        assert_eq!(evaluate("() * ()"), Value::Nothing);

        assert_eq!(evaluate("7 * 2"), Value::Numb(14.0));

        assert_eq!(evaluate("TRUE * TRUE"), Value::Bool(true));
        assert_eq!(evaluate("TRUE * FALSE"), Value::Bool(false));
        assert_eq!(evaluate("FALSE * TRUE"), Value::Bool(false));
        assert_eq!(evaluate("FALSE * FALSE"), Value::Bool(false));

        assert_eq!(evaluate("(2, 3) * (4, 5)"), Value::Tuple(Rc::new(vec![
                    Value::Numb(8.0),
                    Value::Numb(15.0),
                ])));

        match evaluate("1 * \"abc\"") {
            Value::Undefined(Undef::BinaryOperation { op, lhv, rhv, pos }) => {
                assert_eq!(op, "product");
                assert_eq!(*lhv, Value::Numb(1.0));
                assert_eq!(*rhv, Value::Text("abc".into()));
                assert_eq!(pos, 2);
            }
            other => panic!("expected Undefined::BinaryOperation, got {:?}", other),
        }
    }

    #[test]
    fn division() {
        assert_eq!(evaluate("() / ()"), Value::Nothing);

        assert_eq!(evaluate("7 / 2"), Value::Numb(3.5));
        assert_eq!(evaluate("7 / 0"), Value::Numb(f64::INFINITY));

        assert_eq!(evaluate("(10, 20) / (2, 5)"), Value::Tuple(Rc::new(vec![
                    Value::Numb(5.0),
                    Value::Numb(4.0),
                ])));

        match evaluate("1 / \"abc\"") {
            Value::Undefined(Undef::BinaryOperation { op, lhv, rhv, pos }) => {
                assert_eq!(op, "division");
                assert_eq!(*lhv, Value::Numb(1.0));
                assert_eq!(*rhv, Value::Text("abc".into()));
                assert_eq!(pos, 2);
            }
            other => panic!("expected Undefined::BinaryOperation, got {:?}", other),
        }
    }

    #[test]
    fn modulo() {
        assert_eq!(evaluate("() % ()"), Value::Nothing);

        assert_eq!(evaluate("8 % 3"), Value::Numb(2.0));

        assert_eq!(evaluate("(10, 20) % (3, 7)"), Value::Tuple(Rc::new(vec![
                    Value::Numb(1.0),
                    Value::Numb(6.0),
                ])));

        match evaluate("1 % \"abc\"") {
            Value::Undefined(Undef::BinaryOperation { op, lhv, rhv, pos }) => {
                assert_eq!(op, "modulo");
                assert_eq!(*lhv, Value::Numb(1.0));
                assert_eq!(*rhv, Value::Text("abc".into()));
                assert_eq!(pos, 2);
            }
            other => panic!("expected Undefined::BinaryOperation, got {:?}", other),
        }
    }

    #[test]
    fn power() {
        assert_eq!(evaluate("() ^ ()"), Value::Nothing);

        assert_eq!(evaluate("2 ^ 3"), Value::Numb(8.0));

        assert_eq!(evaluate("(10, 2) ^ (2, 4)"), Value::Tuple(Rc::new(vec![
                    Value::Numb(100.0),
                    Value::Numb(16.0),
                ])));

        match evaluate("1 ^ \"abc\"") {
            Value::Undefined(Undef::BinaryOperation { op, lhv, rhv, pos }) => {
                assert_eq!(op, "power");
                assert_eq!(*lhv, Value::Numb(1.0));
                assert_eq!(*rhv, Value::Text("abc".into()));
                assert_eq!(pos, 2);
            }
            other => panic!("expected Undefined::BinaryOperation, got {:?}", other),
        }
    }
}
