

use crate::value::{Scope, Value, Undef};



pub fn eval(name: Value, scope: &Scope, pos: usize) -> Value {
    match name {
        Value::Name(n) => scope.lift(&n),
        other => Value::Undefined(Undef::UnaryOperation {
            op:  "lifting",
            val: Box::new(other),
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
    }
}
