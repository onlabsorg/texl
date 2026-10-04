//! Pairing operation: `X , Y`.
//!
//! Pairing builds tuples. Both operands are flattened with [`Value::items`],
//! concatenated, and the result is normalized:
//!
//! - no items       -> `Nothing`
//! - exactly 1 item -> that item itself (there are no 1-tuples)
//! - 2 or more      -> a flat `Tuple`
//!
//! As a consequence, `Nothing` operands vanish, nested tuples are flattened,
//! and the operation is associative. Lists, namespaces and the other
//! non-tuple values are treated as single items and are *not* unpacked.
//!
//! This operation never fails, so it never returns `Undefined`.

use crate::value::Value;



/// `X , Y` - pairing operation.
///
/// Concatenates the flattened items of `lhv` and `rhv` into a single value:
/// `Nothing` if there are no items, the bare item if there is exactly one,
/// otherwise a flat `Tuple`.
pub fn eval(lhv: Value, rhv: Value) -> Value {
    // `items()` flattens nested tuples and skips `Nothing`, so chaining the two
    // iterators gives the full flat sequence of items.
    let vec: Vec<_> = lhv.items().chain(rhv.items()).collect();

    // Normalize the result (same rules as `Value::normalize`).
    Value::tuple(vec)
}



// ----------------------------------------------------------------------------
//  UNIT TESTS
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
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
    fn pairing() {
        // Two single values build a tuple; chaining is flat.
        assert_eq!(evaluate("7 , 2"), Value::tuple(vec![
            Value::Numb(7.0),
            Value::Numb(2.0),
        ]));
        assert_eq!(evaluate("7 , 2 , 9"), Value::tuple(vec![
            Value::Numb(7.0),
            Value::Numb(2.0),
            Value::Numb(9.0),
        ]));

        // A tuple on either side, or on both sides, is flattened into the result.
        assert_eq!(evaluate("7 , (2,9)"), Value::tuple(vec![
            Value::Numb(7.0),
            Value::Numb(2.0),
            Value::Numb(9.0),
        ]));
        assert_eq!(evaluate("(7,2) , 9"), Value::tuple(vec![
            Value::Numb(7.0),
            Value::Numb(2.0),
            Value::Numb(9.0),
        ]));
        assert_eq!(evaluate("(7,2) , (9,3)"), Value::tuple(vec![
            Value::Numb(7.0),
            Value::Numb(2.0),
            Value::Numb(9.0),
            Value::Numb(3.0),
        ]));

        // Explicit grouping does not change the result (associativity / flattening).
        assert_eq!(evaluate("7 , (2 , (9 , 3))"), evaluate("((7 , 2) , 9) , 3"));

        // `Nothing` operands are dropped.
        assert_eq!(evaluate("7 , 2 , () , 9"), Value::tuple(vec![
            Value::Numb(7.0),
            Value::Numb(2.0),
            Value::Numb(9.0),
        ]));

        // A single remaining item is returned as is, not as a 1-tuple.
        assert_eq!(evaluate("1 , ()"), Value::Numb(1.0));
        assert_eq!(evaluate("() , 1"), Value::Numb(1.0));

        // No items at all gives `Nothing`.
        assert_eq!(evaluate("() , ()"), Value::Nothing);

        // Lists are single items: they are paired, not unpacked.
        assert_eq!(evaluate("[1,2] , 3"), Value::tuple(vec![
            Value::list(vec![Value::Numb(1.0), Value::Numb(2.0)]),
            Value::Numb(3.0),
        ]));

        // Values of different types can be paired.
        assert_eq!(evaluate("TRUE , \"a\""), Value::tuple(vec![
            Value::Bool(true),
            Value::Text("a".into()),
        ]));
    }
}
