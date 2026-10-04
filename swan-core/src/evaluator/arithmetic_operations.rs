//! Arithmetic binary operations: `+`, `-`, `*`, `/`, `%`, `^`.
//!
//! Each function here handles a single pair of non-tuple operands.
//! Tuple broadcasting (element-wise application, `Nothing` padding and
//! normalization) is done by the `map_pairs!` macro in the parent evaluator
//! module, so these functions never see a `Tuple`.
//!
//! Operations are type-homogeneous: unsupported operand types (including a
//! `Nothing` paired with a value) give `Value::Undefined(Undef::BinaryOperation)`.
//! Numeric operations follow IEEE 754 semantics, so there are no numeric errors:
//! division by zero yields `±inf` or `NaN`.

use crate::value::Value;


// ----------------------------------------------------------------------------
//  Arithmetic binary operations
// ----------------------------------------------------------------------------

/// `X + Y` — sum.
///
/// Both operands must be of the same type:
/// - `Numb + Numb`  → arithmetic addition
/// - `Bool + Bool`  → logical OR
/// - `Text + Text`  → concatenation
/// - `List + List`  → concatenation into a new list
///
/// Any other combination (including mixed types) gives `Undefined("sum")`.
pub fn eval_add(lhv: Value, rhv: Value, pos: usize) -> Value {
    match (lhv, rhv) {
        // Numeric addition
        (Value::Numb(a), Value::Numb(b)) => Value::Numb(a + b),

        // Logical OR
        (Value::Bool(a), Value::Bool(b)) => Value::Bool(a || b),

        // Build the result in one allocation, then convert to Rc<str>.
        (Value::Text(a), Value::Text(b)) => {
            let mut s = String::with_capacity(a.len() + b.len());
            s.push_str(&a);
            s.push_str(&b);
            Value::Text(s.into())
        }

        // Items from both lists are cloned into a new list.
        (Value::List(a), Value::List(b)) => {
            let mut v = Vec::with_capacity(a.len() + b.len());
            v.extend_from_slice(&a);
            v.extend_from_slice(&b);
            Value::list(v)
        }

        (lhv, rhv) => Value::undef_binary("sum", lhv, rhv, pos),
    }
}

/// `X - Y` — subtraction.
///
/// - `Numb - Numb`  → arithmetic subtraction
///
/// Any other combination gives `Undefined("subtraction")`.
pub fn eval_subtraction(lhv: Value, rhv: Value, pos: usize) -> Value {
    match (lhv, rhv) {
        (Value::Numb(a), Value::Numb(b)) => Value::Numb(a - b),

        (lhv, rhv) => Value::undef_binary("subtraction", lhv, rhv, pos),
    }
}

/// `X * Y` — product.
///
/// - `Numb * Numb`  → multiplication
/// - `Bool * Bool`  → logical AND
///
/// Any other combination gives `Undefined("product")`.
pub fn eval_product(lhv: Value, rhv: Value, pos: usize) -> Value {
    match (lhv, rhv) {
        (Value::Numb(a), Value::Numb(b)) => Value::Numb(a * b),

        // Logical AND
        (Value::Bool(a), Value::Bool(b)) => Value::Bool(a && b),

        (lhv, rhv) => Value::undef_binary("product", lhv, rhv, pos),
    }
}

/// `X / Y` — division.
///
/// - `Numb / Numb`  → IEEE 754 division (`x / 0` is `±inf`, `0 / 0` is `NaN`)
///
/// Any other combination gives `Undefined("division")`.
pub fn eval_division(lhv: Value, rhv: Value, pos: usize) -> Value {
    match (lhv, rhv) {
        (Value::Numb(a), Value::Numb(b)) => Value::Numb(a / b),

        (lhv, rhv) => Value::undef_binary("division", lhv, rhv, pos),
    }
}

/// `X % Y` — modulo.
///
/// - `Numb % Numb`  → remainder with the sign of the dividend (Rust `%` on `f64`; `x % 0` is `NaN`)
///
/// Any other combination gives `Undefined("modulo")`.
pub fn eval_modulo(lhv: Value, rhv: Value, pos: usize) -> Value {
    match (lhv, rhv) {
        (Value::Numb(a), Value::Numb(b)) => Value::Numb(a % b),

        (lhv, rhv) => Value::undef_binary("modulo", lhv, rhv, pos),
    }
}

/// `X ^ Y` — power.
///
/// - `Numb ^ Numb`  → `a.powf(b)`
///
/// Any other combination gives `Undefined("power")`.
pub fn eval_power(lhv: Value, rhv: Value, pos: usize) -> Value {
    match (lhv, rhv) {
        (Value::Numb(a), Value::Numb(b)) => Value::Numb(a.powf(b)),

        (lhv, rhv) => Value::undef_binary("power", lhv, rhv, pos),
    }
}






// ----------------------------------------------------------------------------
//  UNIT TESTS
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
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

    /// Asserts that `src` evaluates to `Undefined(BinaryOperation)` with the given parts.
    fn assert_undefined(src: &str, op: &str, lhv: Value, rhv: Value, pos: usize) {
        match evaluate(src) {
            Value::Undefined(Undef::BinaryOperation { op: o, lhv: l, rhv: r, pos: p }) => {
                assert_eq!(o, op);
                assert_eq!(*l, lhv);
                assert_eq!(*r, rhv);
                assert_eq!(p, pos);
            }
            other => panic!("expected Undefined::BinaryOperation for {src:?}, got {other:?}"),
        }
    }

    #[test]
    fn sum() {
        // Nothing against Nothing
        assert_eq!(evaluate("() + ()"), Value::Nothing);

        // Numeric addition
        assert_eq!(evaluate("1 + 2"), Value::Numb(3.0));

        // Logical OR
        assert_eq!(evaluate("TRUE + TRUE"), Value::Bool(true));
        assert_eq!(evaluate("TRUE + FALSE"), Value::Bool(true));
        assert_eq!(evaluate("FALSE + TRUE"), Value::Bool(true));
        assert_eq!(evaluate("FALSE + FALSE"), Value::Bool(false));

        // String concatenation
        assert_eq!(evaluate("\"abc\" + \"def\""), Value::Text("abcdef".into()));
        assert_eq!(evaluate("\"\" + \"a\""), Value::Text("a".into()));

        // List concatenation
        assert_eq!(
            evaluate("[1,2,3] + [4,5,6]"), 
            Value::list(vec![
                Value::Numb(1.0),
                Value::Numb(2.0),
                Value::Numb(3.0),
                Value::Numb(4.0),
                Value::Numb(5.0),
                Value::Numb(6.0)
            ])
        );
        assert_eq!(evaluate("[] + [1]"), Value::list(vec![Value::Numb(1.0)]));

        // Broadcasting over tuples
        assert_eq!(evaluate("(1, 2) + (3, 4)"), Value::tuple(vec![
                    Value::Numb(4.0),
                    Value::Numb(6.0),
                ]));

        // Unsupported operand types
        assert_undefined("1 + \"abc\"", "sum", Value::Numb(1.0), Value::Text("abc".into()), 2);
        assert_undefined("() + 1", "sum", Value::Nothing, Value::Numb(1.0), 3);
        assert_undefined("[1] + \"a\"", "sum", Value::list(vec![Value::Numb(1.0)]), Value::Text("a".into()), 4);
    }

    #[test]
    fn subtraction() {
        // Nothing against Nothing
        assert_eq!(evaluate("() - ()"), Value::Nothing);

        // Numeric subtraction
        assert_eq!(evaluate("7 - 2"), Value::Numb(5.0));

        // Broadcasting over tuples
        assert_eq!(evaluate("(10, 20) - (1, 2)"), Value::tuple(vec![
                    Value::Numb(9.0),
                    Value::Numb(18.0),
                ]));

        // Unsupported operand types
        assert_undefined("1 - \"abc\"", "subtraction", Value::Numb(1.0), Value::Text("abc".into()), 2);
    }

    #[test]
    fn product() {
        // Nothing against Nothing
        assert_eq!(evaluate("() * ()"), Value::Nothing);

        // Numeric multiplication
        assert_eq!(evaluate("7 * 2"), Value::Numb(14.0));

        // Logical AND
        assert_eq!(evaluate("TRUE * TRUE"), Value::Bool(true));
        assert_eq!(evaluate("TRUE * FALSE"), Value::Bool(false));
        assert_eq!(evaluate("FALSE * TRUE"), Value::Bool(false));
        assert_eq!(evaluate("FALSE * FALSE"), Value::Bool(false));

        // Broadcasting over tuples
        assert_eq!(evaluate("(2, 3) * (4, 5)"), Value::tuple(vec![
                    Value::Numb(8.0),
                    Value::Numb(15.0),
                ]));

        // Unsupported operand types
        assert_undefined("1 * \"abc\"", "product", Value::Numb(1.0), Value::Text("abc".into()), 2);
    }

    #[test]
    fn division() {
        // Nothing against Nothing
        assert_eq!(evaluate("() / ()"), Value::Nothing);

        // Numeric division
        assert_eq!(evaluate("7 / 2"), Value::Numb(3.5));
        assert_eq!(evaluate("7 / 0"), Value::Numb(f64::INFINITY));
        assert_eq!(evaluate("(0 - 7) / 0"), Value::Numb(f64::NEG_INFINITY));
        assert!(matches!(evaluate("0 / 0"), Value::Numb(n) if n.is_nan()));

        // Broadcasting over tuples
        assert_eq!(evaluate("(10, 20) / (2, 5)"), Value::tuple(vec![
                    Value::Numb(5.0),
                    Value::Numb(4.0),
                ]));

        // Unsupported operand types
        assert_undefined("1 / \"abc\"", "division", Value::Numb(1.0), Value::Text("abc".into()), 2);
    }

    #[test]
    fn modulo() {
        // Nothing against Nothing
        assert_eq!(evaluate("() % ()"), Value::Nothing);

        // Numeric modulo
        assert_eq!(evaluate("8 % 3"), Value::Numb(2.0));
        assert_eq!(evaluate("(0 - 7) % 3"), Value::Numb(-1.0));
        assert!(matches!(evaluate("5 % 0"), Value::Numb(n) if n.is_nan()));

        // Broadcasting over tuples
        assert_eq!(evaluate("(10, 20) % (3, 7)"), Value::tuple(vec![
                    Value::Numb(1.0),
                    Value::Numb(6.0),
                ]));

        // Unsupported operand types
        assert_undefined("1 % \"abc\"", "modulo", Value::Numb(1.0), Value::Text("abc".into()), 2);
    }

    #[test]
    fn power() {
        // Nothing against Nothing
        assert_eq!(evaluate("() ^ ()"), Value::Nothing);

        // Numeric exponentiation
        assert_eq!(evaluate("2 ^ 3"), Value::Numb(8.0));
        assert_eq!(evaluate("0 ^ 0"), Value::Numb(1.0));


        // Broadcasting over tuples
        assert_eq!(evaluate("(10, 2) ^ (2, 4)"), Value::tuple(vec![
                    Value::Numb(100.0),
                    Value::Numb(16.0),
                ]));

        // Unsupported operand types
        assert_undefined("1 ^ \"abc\"", "power", Value::Numb(1.0), Value::Text("abc".into()), 2);
    }
}
