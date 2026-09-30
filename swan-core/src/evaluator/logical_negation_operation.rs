

use crate::value::Value;



/// `-X` — negation.
///
/// Currently only defined for `Numb`. When tuples are supported the operation
/// should be mapped element-wise over all items. (#11)
pub fn eval(val: Value) -> Value {
    Value::Bool(!bool::from(val))
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
    fn logical_negation() {

        // Bool
        assert_eq!(evaluate("!TRUE"), Value::Bool(false));
        assert_eq!(evaluate("!FALSE"), Value::Bool(true));

        // Numb
        assert_eq!(evaluate("!1"), Value::Bool(false));
        assert_eq!(evaluate("!0"), Value::Bool(true));
        assert_eq!(evaluate("!!0"), Value::Bool(false));
        assert_eq!(evaluate("!!1"), Value::Bool(true));

        // List
        assert_eq!(evaluate("![]"), Value::Bool(true));
        assert_eq!(evaluate("![1]"), Value::Bool(false));

        // Namespace
        assert_eq!(evaluate("!{}"), Value::Bool(true));
        assert_eq!(evaluate("!{a:1}"), Value::Bool(false));

        // Func
        assert_eq!(evaluate("!(x->2*x)"), Value::Bool(false));

        // Nothing
        assert_eq!(evaluate("!()"), Value::Nothing);

        // Tuple
        assert_eq!(
            evaluate("!(TRUE, 0)"),
            Value::Tuple(Rc::new(vec![
                Value::Bool(false),
                Value::Bool(true),
            ]))
        );
    }
}
