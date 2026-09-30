
use std::rc::Rc;
use crate::value::Value;





/// `X , Y` - pairing operation
pub fn eval(lhv: Value, rhv: Value) -> Value {
    let mut vec: Vec<_> = lhv.items().chain(rhv.items()).collect();
    match vec.len() {
        0 => Value::Nothing,
        1 => vec.pop().unwrap(),
        _ => Value::Tuple(Rc::new(vec)),
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
    fn pairing() {
        assert_eq!(evaluate("7 , 2 , 9"), Value::Tuple(Rc::new(vec![
                    Value::Numb(7.0), 
                    Value::Numb(2.0),
                    Value::Numb(9.0)
        ])));

        assert_eq!(evaluate("7 , (2,9)"), Value::Tuple(Rc::new(vec![
                    Value::Numb(7.0), 
                    Value::Numb(2.0),
                    Value::Numb(9.0)
        ])));

        assert_eq!(evaluate("(7,2) , 9"), Value::Tuple(Rc::new(vec![
                    Value::Numb(7.0), 
                    Value::Numb(2.0),
                    Value::Numb(9.0)
        ])));

        assert_eq!(evaluate("(7,2) , (9,3)"), Value::Tuple(Rc::new(vec![
                    Value::Numb(7.0), 
                    Value::Numb(2.0),
                    Value::Numb(9.0),
                    Value::Numb(3.0)
        ])));

        assert_eq!(evaluate("7 , 2 , () , 9"), Value::Tuple(Rc::new(vec![
                    Value::Numb(7.0), 
                    Value::Numb(2.0),
                    Value::Numb(9.0)
        ])));

        assert_eq!(evaluate("() , ()"), Value::Nothing);
        assert_eq!(evaluate("1 , ()"), Value::Numb(1.0));
        assert_eq!(evaluate("() , 1"), Value::Numb(1.0));
    }
}
