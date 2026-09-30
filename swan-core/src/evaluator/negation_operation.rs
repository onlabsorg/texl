

use crate::value::{Value, Undef};



/// `-X` — negation.
///
/// Currently only defined for `Numb`. When tuples are supported the operation
/// should be mapped element-wise over all items. (#11)
pub fn eval(val: Value, pos: usize) -> Value {
    match val {
        Value::Numb(n) => Value::Numb(-n),
        other => Value::Undefined(Undef::UnaryOperation {
            op:  "negation",
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
    fn negate() {
        
        // Number
        assert_eq!(evaluate("-2"), Value::Numb(-2.0));

        // Nothing
        assert_eq!(evaluate("-()"), Value::Nothing);

        // Tuple
        assert_eq!(evaluate("-(2, 3, 4)"), Value::Tuple(Rc::new(vec![
            Value::Numb(-2.0),
            Value::Numb(-3.0),
            Value::Numb(-4.0)
        ])));

        // Others
        assert!(matches!(evaluate("-[]"), Value::Undefined(_)));
    }
}
