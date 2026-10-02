//! Comparison operations: `==`, `!=`, `>`, `<`, `>=`, `<=`.
//!
//! Every function here works on a single pair of (non-broadcast) values.
//! Element-wise tuple broadcasting is done by the caller (`map_pairs!` in
//! `evaluator.rs`), so these functions never need to handle tuples themselves.
//!
//! - `==` and `!=` are total: they always return a `Value::Bool`.
//! - The four ordering operators rely on `Value::partial_cmp`. When the two
//!   values have no defined order (e.g. `TRUE > 1`, or a NaN number), they return
//!   `Undefined(Undef::BinaryOperation { op: "order", .. })`.
//! - `Nothing` is equal only to `Nothing` and is less than any other value.
use std::cmp::Ordering;
use crate::value::{Value, Undef};



// ----------------------------------------------------------------------------
//  Comparison operations
// ----------------------------------------------------------------------------

/// `X == Y` — equal-to operation.
///
/// Uses structural `Value` equality; values of different types are simply
/// not equal. Never returns `Undefined`.
pub fn eval_equal_to(lhv: Value, rhv: Value) -> Value {
    Value::Bool(lhv == rhv)
}

/// `X != Y` — not-equal-to operation. The exact negation of `X == Y`.
pub fn eval_not_equal_to(lhv: Value, rhv: Value) -> Value {
    Value::Bool(lhv != rhv)
}

/// Shared implementation of the four ordering operators.
///
/// Compares `lhv` with `rhv` using `Value::partial_cmp` and converts the
/// resulting [`Ordering`] into a `Bool` with `test`. If the values are not
/// comparable, returns `Undefined(BinaryOperation)` with `op: "order"`.
fn compare(lhv: Value, rhv: Value, pos: usize, test: fn(Ordering) -> bool) -> Value {
    match lhv.partial_cmp(&rhv) {
        Some(ordering) => Value::Bool(test(ordering)),
        None => Value::Undefined(Undef::BinaryOperation {
            op:  "order",
            lhv: Box::new(lhv),
            rhv: Box::new(rhv),
            pos,
        }),
    }
}

/// `X > Y` — greater-than operation.
///
/// Returns `Undefined(order)` if `X` and `Y` have no defined order.
pub fn eval_greater_than(lhv: Value, rhv: Value, pos: usize) -> Value {
    compare(lhv, rhv, pos, Ordering::is_gt)
}

/// `X < Y` — less-than operation.
///
/// Returns `Undefined(order)` if `X` and `Y` have no defined order.
pub fn eval_less_than(lhv: Value, rhv: Value, pos: usize) -> Value {
    compare(lhv, rhv, pos, Ordering::is_lt)
}

/// `X >= Y` — greater-than-or-equal-to operation.
///
/// Returns `Undefined(order)` if `X` and `Y` have no defined order.
pub fn eval_greater_than_or_equal_to(lhv: Value, rhv: Value, pos: usize) -> Value {
    compare(lhv, rhv, pos, Ordering::is_ge)
}

/// `X <= Y` — less-than-or-equal-to operation.
///
/// Returns `Undefined(order)` if `X` and `Y` have no defined order.
pub fn eval_less_than_or_equal_to(lhv: Value, rhv: Value, pos: usize) -> Value {
    compare(lhv, rhv, pos, Ordering::is_le)
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

    /// Parses and evaluates `src` in a scope where TRUE and FALSE are defined.
    fn evaluate(src: &str) -> Value {
        let mut parser = Parser::new(Lexer::new(src));
        let ast = parser.parse();
        let scope = Scope::new();
        scope.set("TRUE", Value::Bool(true));
        scope.set("FALSE", Value::Bool(false));
        eval(&ast, &scope)
    }

    /// Evaluates `src` and returns the resulting bool; panics if it is not a Bool.
    fn evaluate_bool(src: &str) -> bool {
        match evaluate(src) {
            Value::Bool(b) => b,
            other => panic!("Boolean expression expected for {src:?}, found {other:?}"),
        }
    }

    #[test]
    fn equal_to() {
        // bool
        assert_eq!(evaluate_bool("TRUE == TRUE")  , true);
        assert_eq!(evaluate_bool("FALSE == FALSE"), true);
        assert_eq!(evaluate_bool("TRUE == FALSE") , false);

        // numb
        assert_eq!(evaluate_bool("1 == 1"), true);
        assert_eq!(evaluate_bool("1 == 2"), false);

        // text
        assert_eq!(evaluate_bool("\"abc\" == \"abc\""), true);
        assert_eq!(evaluate_bool("\"abc\" == \"xy\"") , false);

        // list
        assert_eq!(evaluate_bool("[1,2,3] == [1,2,3]"), true);
        assert_eq!(evaluate_bool("[1,2,3] == [1,2]")  , false);

        // namespace
        assert_eq!(evaluate_bool("{x:1,y:2} == {x:1,y:2}"), true);
        assert_eq!(evaluate_bool("{x:1,y:2} == {x:1,z:2}"), false);

        // tuple: the operation is broadcast item by item; the result is a tuple of bools
        assert_eq!(
            evaluate("(1,2,3) == (1,4,3)"), 
            Value::Tuple(Rc::new(vec![
                Value::Bool(true),
                Value::Bool(false),
                Value::Bool(true),
            ]))
        );
        // the shorter tuple is padded with Nothing
        assert_eq!(
            evaluate("(1,2,3) == (1,2)"), 
            Value::Tuple(Rc::new(vec![
                Value::Bool(true),
                Value::Bool(true),
                Value::Bool(false),
            ]))
        );

        // nothing
        // Nothing vs Nothing gives Nothing; Nothing vs a non-tuple value is compared as is
        assert_eq!(evaluate("() == ()"), Value::Nothing);
        assert_eq!(evaluate_bool("() == FALSE  "), false);
        assert_eq!(evaluate_bool("() == 1      "), false);
        assert_eq!(evaluate_bool("() == \"abc\""), false);
        assert_eq!(evaluate_bool("() == [1,2,3]"), false);
        assert_eq!(evaluate_bool("() == {x: 10}"), false);

        // Undefined values are compared by content (operation and operands, not position); unclosed-bracket errors are Undefined too
        assert_eq!(evaluate_bool("(-[]) == (-[])"), true);
        assert_eq!(evaluate_bool("(-[]) == (-{})"), false);
        assert_eq!(evaluate_bool("([]^2) == ([]^2)"), true);
        assert_eq!(evaluate_bool("([]^2) == ({}^2)"), false);
        assert_eq!(evaluate_bool("(@) == (@)"), true);
        assert_eq!(evaluate_bool("(~) == (@)"), false);
        assert_eq!(evaluate("(1,2"), evaluate("(1,2 "));
    }

    #[test]
    fn not_equal_to() {
        // bool
        assert_eq!(evaluate_bool("TRUE != TRUE"), false);
        assert_eq!(evaluate_bool("FALSE != FALSE"), false);
        assert_eq!(evaluate_bool("TRUE != FALSE"), true);

        // numb
        assert_eq!(evaluate_bool("1 != 1"), false);
        assert_eq!(evaluate_bool("1 != 2"), true);

        // text
        assert_eq!(evaluate_bool("\"abc\" != \"abc\""), false);
        assert_eq!(evaluate_bool("\"abc\" != \"xy\"") , true);

        // list
        assert_eq!(evaluate_bool("[1,2,3] != [1,2,3]"), false);
        assert_eq!(evaluate_bool("[1,2,3] != [1,2]")  , true);

        // namespace
        assert_eq!(evaluate_bool("{x:1,y:2} != {x:1,y:2}"), false);
        assert_eq!(evaluate_bool("{x:1,y:2} != {x:1,z:2}"), true);

        // tuple: the operation is broadcast item by item; the result is a tuple of bools
        assert_eq!(
            evaluate("(1,2,3) != (1,4,3)"), 
            Value::Tuple(Rc::new(vec![
                Value::Bool(false),
                Value::Bool(true),
                Value::Bool(false),
            ]))
        );
        // the shorter tuple is padded with Nothing
        assert_eq!(
            evaluate("(1,2,3) != (1,2)"), 
            Value::Tuple(Rc::new(vec![
                Value::Bool(false),
                Value::Bool(false),
                Value::Bool(true),
            ]))
        );

        // nothing
        // Nothing vs Nothing gives Nothing; Nothing vs a non-tuple value is compared as is
        assert_eq!(evaluate("() != ()"), Value::Nothing);
        assert_eq!(evaluate_bool("() != FALSE  "), true);
        assert_eq!(evaluate_bool("() != 1      "), true);
        assert_eq!(evaluate_bool("() != \"abc\""), true);
        assert_eq!(evaluate_bool("() != [1,2,3]"), true);
        assert_eq!(evaluate_bool("() != {x: 10}"), true);

        // Undefined values are compared by content (operation and operands, not position); unclosed-bracket errors are Undefined too
        assert_eq!(evaluate_bool("(-[]) != (-[])"), false);
        assert_eq!(evaluate_bool("(-[]) != (-{})"), true);
        assert_eq!(evaluate_bool("([]^2) != ([]^2)"), false);
        assert_eq!(evaluate_bool("([]^2) != ({}^2)"), true);
        assert_eq!(evaluate_bool("(@) != (@)"), false);
        assert_eq!(evaluate_bool("(~) != (@)"), true);
        assert_ne!(evaluate("(1,2"), evaluate("[1,2"));
    }

    #[test]
    fn greater_than() {
        // bool
        assert_eq!(evaluate_bool("TRUE > TRUE  "), false);
        assert_eq!(evaluate_bool("FALSE > FALSE"), false);
        assert_eq!(evaluate_bool("TRUE > FALSE "), true);
        assert_eq!(evaluate_bool("FALSE > TRUE "), false);

        // numb
        assert_eq!(evaluate_bool("1 > 1"), false);
        assert_eq!(evaluate_bool("1 > 2"), false);
        assert_eq!(evaluate_bool("2 > 1"), true);
        assert_eq!(evaluate_bool("2 > 2"), false);

        // text 
        assert_eq!(evaluate_bool("\"abc\" > \"abc\""), false);
        assert_eq!(evaluate_bool("\"abc\" > \"xyz\""), false);
        assert_eq!(evaluate_bool("\"xyz\" > \"abc\""), true);

        // list
        assert_eq!(evaluate_bool("[1,2,3] > [1,2,3]"), false);
        assert_eq!(evaluate_bool("[1,2,3] > [1,3,4]"), false);
        assert_eq!(evaluate_bool("[1,3,4] > [1,2,3]"), true);
        assert_eq!(evaluate_bool("[1,2  ] > [1,2,3]"), false);
        assert_eq!(evaluate_bool("[1,2,3] > [1,2  ]"), true);
        
        // tuple: the operation is broadcast item by item; the result is a tuple of bools
        assert_eq!(
            evaluate("(1,20,3) > (10,2,3)"), 
            Value::Tuple(Rc::new(vec![
                Value::Bool(false),
                Value::Bool(true),
                Value::Bool(false),
            ]))
        );
        // the shorter tuple is padded with Nothing
        assert_eq!(
            evaluate("(1,20,3) > (10,2)"), 
            Value::Tuple(Rc::new(vec![
                Value::Bool(false),
                Value::Bool(true),
                Value::Bool(true),
            ]))
        );

        // nothing
        // Nothing vs Nothing gives Nothing; Nothing vs a non-tuple value is compared as is (Nothing is less than every other value)
        assert_eq!(evaluate("() > ()"), Value::Nothing);
        assert_eq!(evaluate_bool("() > FALSE  "), false);
        assert_eq!(evaluate_bool("() > 1      "), false);
        assert_eq!(evaluate_bool("() > \"abc\""), false);
        assert_eq!(evaluate_bool("() > [1,2,3]"), false);
        assert_eq!(evaluate_bool("() > {x: 10}"), false);
        assert_eq!(evaluate_bool("FALSE   > ()"), true);
        assert_eq!(evaluate_bool("1       > ()"), true);
        assert_eq!(evaluate_bool("\"abc\" > ()"), true);
        assert_eq!(evaluate_bool("[1,2,3] > ()"), true);
        assert_eq!(evaluate_bool("{x: 10} > ()"), true);

        // values of unrelated types have no order: Undefined(order) carrying both operands and the operator position (byte offset 5)
        assert_eq!(evaluate("TRUE > 1"), Value::Undefined(Undef::BinaryOperation{
            op:  "order", 
            lhv: Box::new(Value::Bool(true)), 
            rhv: Box::new(Value::Numb(1.0)), 
            pos: 5,
        }));
    }

    #[test]
    fn less_than() {
        // bool
        assert_eq!(evaluate_bool("TRUE < TRUE  "), false);
        assert_eq!(evaluate_bool("FALSE < FALSE"), false);
        assert_eq!(evaluate_bool("TRUE < FALSE "), false);
        assert_eq!(evaluate_bool("FALSE < TRUE "), true);

        // numb
        assert_eq!(evaluate_bool("1 < 1"), false);
        assert_eq!(evaluate_bool("1 < 2"), true);
        assert_eq!(evaluate_bool("2 < 1"), false);
        assert_eq!(evaluate_bool("2 < 2"), false);

        // text 
        assert_eq!(evaluate_bool("\"abc\" < \"abc\""), false);
        assert_eq!(evaluate_bool("\"abc\" < \"xyz\""), true);
        assert_eq!(evaluate_bool("\"xyz\" < \"abc\""), false);

        // list
        assert_eq!(evaluate_bool("[1,2,3] < [1,2,3]"), false);
        assert_eq!(evaluate_bool("[1,2,3] < [1,3,4]"), true);
        assert_eq!(evaluate_bool("[1,3,4] < [1,2,3]"), false);
        assert_eq!(evaluate_bool("[1,2  ] < [1,2,3]"), true);
        assert_eq!(evaluate_bool("[1,2,3] < [1,2  ]"), false);
        
        // tuple: the operation is broadcast item by item; the result is a tuple of bools
        assert_eq!(
            evaluate("(1,20,3) < (10,2,3)"), 
            Value::Tuple(Rc::new(vec![
                Value::Bool(true),
                Value::Bool(false),
                Value::Bool(false),
            ]))
        );
        // the shorter tuple is padded with Nothing
        assert_eq!(
            evaluate("(1,20,3) < (10,2)"), 
            Value::Tuple(Rc::new(vec![
                Value::Bool(true),
                Value::Bool(false),
                Value::Bool(false),
            ]))
        );

        // nothing
        // Nothing vs Nothing gives Nothing; Nothing vs a non-tuple value is compared as is (Nothing is less than every other value)
        assert_eq!(evaluate("() < ()"), Value::Nothing);
        assert_eq!(evaluate_bool("() < FALSE  "), true);
        assert_eq!(evaluate_bool("() < 1      "), true);
        assert_eq!(evaluate_bool("() < \"abc\""), true);
        assert_eq!(evaluate_bool("() < [1,2,3]"), true);
        assert_eq!(evaluate_bool("() < {x: 10}"), true);
        assert_eq!(evaluate_bool("FALSE   < ()"), false);
        assert_eq!(evaluate_bool("1       < ()"), false);
        assert_eq!(evaluate_bool("\"abc\" < ()"), false);
        assert_eq!(evaluate_bool("[1,2,3] < ()"), false);
        assert_eq!(evaluate_bool("{x: 10} < ()"), false);

        // values of unrelated types have no order: Undefined(order) carrying both operands and the operator position (byte offset 5)
        assert_eq!(evaluate("TRUE < 1"), Value::Undefined(Undef::BinaryOperation{
            op:  "order", 
            lhv: Box::new(Value::Bool(true)), 
            rhv: Box::new(Value::Numb(1.0)), 
            pos: 5,
        }));
    }

    #[test]
    fn greater_than_or_equal_to() {
        // bool
        assert_eq!(evaluate_bool("TRUE >= TRUE  "), true);
        assert_eq!(evaluate_bool("FALSE >= FALSE"), true);
        assert_eq!(evaluate_bool("TRUE >= FALSE "), true);
        assert_eq!(evaluate_bool("FALSE >= TRUE "), false);

        // numb
        assert_eq!(evaluate_bool("1 >= 1"), true);
        assert_eq!(evaluate_bool("1 >= 2"), false);
        assert_eq!(evaluate_bool("2 >= 1"), true);
        assert_eq!(evaluate_bool("2 >= 2"), true);

        // text 
        assert_eq!(evaluate_bool("\"abc\" >= \"abc\""), true);
        assert_eq!(evaluate_bool("\"abc\" >= \"xyz\""), false);
        assert_eq!(evaluate_bool("\"xyz\" >= \"abc\""), true);

        // list
        assert_eq!(evaluate_bool("[1,2,3] >= [1,2,3]"), true);
        assert_eq!(evaluate_bool("[1,2,3] >= [1,3,4]"), false);
        assert_eq!(evaluate_bool("[1,3,4] >= [1,2,3]"), true);
        assert_eq!(evaluate_bool("[1,2  ] >= [1,2,3]"), false);
        assert_eq!(evaluate_bool("[1,2,3] >= [1,2  ]"), true);
        
        // tuple: the operation is broadcast item by item; the result is a tuple of bools
        assert_eq!(
            evaluate("(1,20,3) >= (10,2,3)"), 
            Value::Tuple(Rc::new(vec![
                Value::Bool(false),
                Value::Bool(true),
                Value::Bool(true),
            ]))
        );
        // the shorter tuple is padded with Nothing
        assert_eq!(
            evaluate("(1,20,3) >= (10,2)"), 
            Value::Tuple(Rc::new(vec![
                Value::Bool(false),
                Value::Bool(true),
                Value::Bool(true),
            ]))
        );

        // nothing
        // Nothing vs Nothing gives Nothing; Nothing vs a non-tuple value is compared as is (Nothing is less than every other value)
        assert_eq!(evaluate("() >= ()"), Value::Nothing);
        assert_eq!(evaluate_bool("() >= FALSE  "), false);
        assert_eq!(evaluate_bool("() >= 1      "), false);
        assert_eq!(evaluate_bool("() >= \"abc\""), false);
        assert_eq!(evaluate_bool("() >= [1,2,3]"), false);
        assert_eq!(evaluate_bool("() >= {x: 10}"), false);
        assert_eq!(evaluate_bool("FALSE   >= ()"), true);
        assert_eq!(evaluate_bool("1       >= ()"), true);
        assert_eq!(evaluate_bool("\"abc\" >= ()"), true);
        assert_eq!(evaluate_bool("[1,2,3] >= ()"), true);
        assert_eq!(evaluate_bool("{x: 10} >= ()"), true);

        // values of unrelated types have no order: Undefined(order) carrying both operands and the operator position (byte offset 5)
        assert_eq!(evaluate("TRUE >= 1"), Value::Undefined(Undef::BinaryOperation{
            op:  "order", 
            lhv: Box::new(Value::Bool(true)), 
            rhv: Box::new(Value::Numb(1.0)), 
            pos: 5,
        }));
    }

    #[test]
    fn less_than_or_equal_to() {
        // bool
        assert_eq!(evaluate_bool("TRUE <= TRUE  "), true);
        assert_eq!(evaluate_bool("FALSE <= FALSE"), true);
        assert_eq!(evaluate_bool("TRUE <= FALSE "), false);
        assert_eq!(evaluate_bool("FALSE <= TRUE "), true);

        // numb
        assert_eq!(evaluate_bool("1 <= 1"), true);
        assert_eq!(evaluate_bool("1 <= 2"), true);
        assert_eq!(evaluate_bool("2 <= 1"), false);
        assert_eq!(evaluate_bool("2 <= 2"), true);

        // text 
        assert_eq!(evaluate_bool("\"abc\" <= \"abc\""), true);
        assert_eq!(evaluate_bool("\"abc\" <= \"xyz\""), true);
        assert_eq!(evaluate_bool("\"xyz\" <= \"abc\""), false);

        // list
        assert_eq!(evaluate_bool("[1,2,3] <= [1,2,3]"), true);
        assert_eq!(evaluate_bool("[1,2,3] <= [1,3,4]"), true);
        assert_eq!(evaluate_bool("[1,3,4] <= [1,2,3]"), false);
        assert_eq!(evaluate_bool("[1,2  ] <= [1,2,3]"), true);
        assert_eq!(evaluate_bool("[1,2,3] <= [1,2  ]"), false);
        
        // tuple: the operation is broadcast item by item; the result is a tuple of bools
        assert_eq!(
            evaluate("(1,20,3) <= (10,2,3)"), 
            Value::Tuple(Rc::new(vec![
                Value::Bool(true),
                Value::Bool(false),
                Value::Bool(true),
            ]))
        );
        // the shorter tuple is padded with Nothing
        assert_eq!(
            evaluate("(1,20,3) <= (10,2)"), 
            Value::Tuple(Rc::new(vec![
                Value::Bool(true),
                Value::Bool(false),
                Value::Bool(false),
            ]))
        );

        // nothing
        // Nothing vs Nothing gives Nothing; Nothing vs a non-tuple value is compared as is (Nothing is less than every other value)
        assert_eq!(evaluate("() <= ()"), Value::Nothing);
        assert_eq!(evaluate_bool("() <= FALSE  "), true);
        assert_eq!(evaluate_bool("() <= 1      "), true);
        assert_eq!(evaluate_bool("() <= \"abc\""), true);
        assert_eq!(evaluate_bool("() <= [1,2,3]"), true);
        assert_eq!(evaluate_bool("() <= {x: 10}"), true);
        assert_eq!(evaluate_bool("FALSE   <= ()"), false);
        assert_eq!(evaluate_bool("1       <= ()"), false);
        assert_eq!(evaluate_bool("\"abc\" <= ()"), false);
        assert_eq!(evaluate_bool("[1,2,3] <= ()"), false);
        assert_eq!(evaluate_bool("{x: 10} <= ()"), false);

        // values of unrelated types have no order: Undefined(order) carrying both operands and the operator position (byte offset 5)
        assert_eq!(evaluate("TRUE <= 1"), Value::Undefined(Undef::BinaryOperation{
            op:  "order", 
            lhv: Box::new(Value::Bool(true)), 
            rhv: Box::new(Value::Numb(1.0)), 
            pos: 5,
        }));
    }
}

