



use std::rc::Rc;
use crate::value::{Scope, Value};
use super::pairing_operation;










// ----------------------------------------------------------------------------
//  ASSIGNMENT OPERATION
// ----------------------------------------------------------------------------

/// `X = Y` - assignment operation
pub fn eval(scope: &Scope, names: &Value, values: &Value) {
    let mut names_iter = names.items();
    let mut values_iter = values.items();
    let mut last_name: Rc<str> = "__uname__".into();
    loop {
        let name = names_iter.next();
        let value = values_iter.next();
        match (name, value) {

            (Some(Value::Name(name)), Some(value)) => {
                scope.set(name.as_ref(), value);
                last_name = name;
            }

            (Some(Value::Name(name)), None) => {
                scope.set(name.as_ref(), Value::Nothing);
                last_name = name;
            }

            (None, Some(value)) => {
                let prev_value = scope.get(last_name.as_ref());
                scope.set(last_name.as_ref(), pairing_operation::eval(prev_value, value));
            }

            (None, None) => {
                break;
            }

            _ => {
                scope.set("__uname__", Value::Nothing);
            }
        }
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
    fn assignment() {
        // `x = 5` evaluates to Nothing; the binding is only visible through
        // a subsequent lookup. Because `=` binds tighter than `,`, this
        // parses as `(x = 5), x`, letting us observe the stored value.
        assert_eq!(evaluate("x = 5, x"), Value::Numb(5.0));

        // Assignment itself always yields Nothing.
        assert_eq!(evaluate("x = 5"), Value::Nothing);

        // Destructuring assignment: `(a, b) = (1, 2)` binds a=1, b=2.
        assert_eq!(
            evaluate("(a, b) = (1, 2), a, b"),
            Value::Tuple(Rc::new(vec![Value::Numb(1.0), Value::Numb(2.0)]))
        );

        // Re-assigning an existing name overwrites the previous value.
        assert_eq!(evaluate("x = 1, x = 2, x"), Value::Numb(2.0));

        // Invalid left-hand names.
        assert_eq!(evaluate("(x, 12) = (1, 2)"), Value::Undefined(Undef::BinaryOperation{
            op: "assignment",
            lhv: Box::new(Value::Undefined(Undef::Name{pos:4})),
            rhv: Box::new(Value::Nothing),
            pos: 8,
        }));
    }
}
