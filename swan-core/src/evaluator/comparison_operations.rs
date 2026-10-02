

use std::cmp::Ordering;
use crate::value::{Value, Undef};





// ----------------------------------------------------------------------------
//  Comparison operations
// ----------------------------------------------------------------------------

/// `X == Y` — equal-to operation 
pub fn eval_equal_to(lhv: Value, rhv: Value) -> Value {
    Value::Bool(lhv == rhv)
}

/// `X != Y` — not-equal-to operation
pub fn eval_not_equal_to(lhv: Value, rhv: Value) -> Value {
    Value::Bool(lhv != rhv)
}

/// `X > Y` — greater-than operation
pub fn eval_greater_than(lhv: Value, rhv: Value, pos: usize) -> Value {
    if let Some(cmp) = lhv.partial_cmp(&rhv) {
        Value::Bool(matches!(cmp, Ordering::Greater))
    } else {
        Value::Undefined(Undef::BinaryOperation{
            op:  "order",
            lhv: Box::new(lhv),
            rhv: Box::new(rhv),
            pos: pos,
        })
    }
}

/// `X < Y` — less-than operation
pub fn eval_less_than(lhv: Value, rhv: Value, pos: usize) -> Value {
    if let Some(cmp) = lhv.partial_cmp(&rhv) {
        Value::Bool(matches!(cmp, Ordering::Less))
    } else {
        Value::Undefined(Undef::BinaryOperation{
            op:  "order",
            lhv: Box::new(lhv),
            rhv: Box::new(rhv),
            pos: pos,
        })
    }
}

/// `X >= Y` — greater-than-or-equal-to operation
pub fn eval_greater_than_or_equal_to(lhv: Value, rhv: Value, pos: usize) -> Value {

    // >= is evaluated as !<
    match eval_less_than(lhv, rhv, pos) {
        Value::Bool(b) => Value::Bool(!b),
        undef => undef,
    }
}

/// `X <= Y` — less-than-or-equal-to operation
pub fn eval_less_than_or_equal_to(lhv: Value, rhv: Value, pos: usize) -> Value {

    // <= is evaluated as !>
    match eval_greater_than(lhv, rhv, pos) {
        Value::Bool(b) => Value::Bool(!b),
        undef => undef,
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

    fn evaluate_bool(src: &str) -> bool {
        match evaluate(src) {
            Value::Bool(b) => b,
            _ => panic!("Boolean epression expected, found {}", src),
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

        // tuple
        assert_eq!(
            evaluate("(1,2,3) == (1,4,3)"), 
            Value::Tuple(Rc::new(vec![
                Value::Bool(true),
                Value::Bool(false),
                Value::Bool(true),
            ]))
        );
        assert_eq!(
            evaluate("(1,2,3) == (1,2)"), 
            Value::Tuple(Rc::new(vec![
                Value::Bool(true),
                Value::Bool(true),
                Value::Bool(false),
            ]))
        );

        // nothing
        assert_eq!(evaluate("() == ()"), Value::Nothing);
        assert_eq!(evaluate_bool("() == FALSE  "), false);
        assert_eq!(evaluate_bool("() == 1      "), false);
        assert_eq!(evaluate_bool("() == \"abc\""), false);
        assert_eq!(evaluate_bool("() == [1,2,3]"), false);
        assert_eq!(evaluate_bool("() == {x: 10}"), false);

        // undefined
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

        // tuple
        assert_eq!(
            evaluate("(1,2,3) != (1,4,3)"), 
            Value::Tuple(Rc::new(vec![
                Value::Bool(false),
                Value::Bool(true),
                Value::Bool(false),
            ]))
        );
        assert_eq!(
            evaluate("(1,2,3) != (1,2)"), 
            Value::Tuple(Rc::new(vec![
                Value::Bool(false),
                Value::Bool(false),
                Value::Bool(true),
            ]))
        );

        // nothing
        assert_eq!(evaluate("() != ()"), Value::Nothing);
        assert_eq!(evaluate_bool("() != FALSE  "), true);
        assert_eq!(evaluate_bool("() != 1      "), true);
        assert_eq!(evaluate_bool("() != \"abc\""), true);
        assert_eq!(evaluate_bool("() != [1,2,3]"), true);
        assert_eq!(evaluate_bool("() != {x: 10}"), true);

        // undefined
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
        
        // tuple
        assert_eq!(
            evaluate("(1,20,3) > (10,2,3)"), 
            Value::Tuple(Rc::new(vec![
                Value::Bool(false),
                Value::Bool(true),
                Value::Bool(false),
            ]))
        );
        assert_eq!(
            evaluate("(1,20,3) > (10,2)"), 
            Value::Tuple(Rc::new(vec![
                Value::Bool(false),
                Value::Bool(true),
                Value::Bool(true),
            ]))
        );

        // nothing
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

        // undefined order
        assert_eq!(evaluate("TRUE > 1"), Value::Undefined(Undef::BinaryOperation{
            op : "order", 
            lhv: Box::new(Value::Bool(true)), 
            rhv: Box::new(Value::Numb(1.0)), 
            pos: 5
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
        
        // tuple
        assert_eq!(
            evaluate("(1,20,3) < (10,2,3)"), 
            Value::Tuple(Rc::new(vec![
                Value::Bool(true),
                Value::Bool(false),
                Value::Bool(false),
            ]))
        );
        assert_eq!(
            evaluate("(1,20,3) < (10,2)"), 
            Value::Tuple(Rc::new(vec![
                Value::Bool(true),
                Value::Bool(false),
                Value::Bool(false),
            ]))
        );

        // nothing
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

        // undefined order
        assert_eq!(evaluate("TRUE < 1"), Value::Undefined(Undef::BinaryOperation{
            op : "order", 
            lhv: Box::new(Value::Bool(true)), 
            rhv: Box::new(Value::Numb(1.0)), 
            pos: 5
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
        
        // tuple
        assert_eq!(
            evaluate("(1,20,3) >= (10,2,3)"), 
            Value::Tuple(Rc::new(vec![
                Value::Bool(false),
                Value::Bool(true),
                Value::Bool(true),
            ]))
        );
        assert_eq!(
            evaluate("(1,20,3) >= (10,2)"), 
            Value::Tuple(Rc::new(vec![
                Value::Bool(false),
                Value::Bool(true),
                Value::Bool(true),
            ]))
        );

        // nothing
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

        // undefined order
        assert_eq!(evaluate("TRUE >= 1"), Value::Undefined(Undef::BinaryOperation{
            op : "order", 
            lhv: Box::new(Value::Bool(true)), 
            rhv: Box::new(Value::Numb(1.0)), 
            pos: 5
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
        
        // tuple
        assert_eq!(
            evaluate("(1,20,3) <= (10,2,3)"), 
            Value::Tuple(Rc::new(vec![
                Value::Bool(true),
                Value::Bool(false),
                Value::Bool(true),
            ]))
        );
        assert_eq!(
            evaluate("(1,20,3) <= (10,2)"), 
            Value::Tuple(Rc::new(vec![
                Value::Bool(true),
                Value::Bool(false),
                Value::Bool(false),
            ]))
        );

        // nothing
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

        // undefined order
        assert_eq!(evaluate("TRUE <= 1"), Value::Undefined(Undef::BinaryOperation{
            op : "order", 
            lhv: Box::new(Value::Bool(true)), 
            rhv: Box::new(Value::Numb(1.0)), 
            pos: 5
        }));
    }
}

