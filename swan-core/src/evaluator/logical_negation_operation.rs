//! Unary logical negation (`!X`).
//!
//! The operation converts its operand to a boolean using the language's
//! truthiness rules (`bool::from(Value)`) and returns the opposite `Bool`.
//!
//! Broadcasting over `Nothing` and tuples is **not** handled here: the
//! evaluator wraps this function in `map_values!`, so `eval` only ever sees
//! a single, non-tuple, non-`Nothing` value.

use crate::value::Value;

/// `!X` — logical negation.
///
/// Defined for every value: the operand is converted to a boolean using the
/// truthiness rules implemented by `bool::from(Value)`, and the opposite
/// `Bool` is returned. This operation never produces an `Undefined`.
///
/// Tuple and `Nothing` operands are handled by the caller (`map_values!` in
/// the evaluator), which applies this function item by item.
pub fn eval(val: Value) -> Value {
    Value::Bool(!bool::from(val))
}


// ----------------------------------------------------------------------------
//  UNIT TESTS
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;
    use crate::lexer::Lexer;
    use crate::parser::Parser;
    use crate::value::Scope;
    use crate::evaluator::eval;

    /// Parses and evaluates `src` in a fresh scope that defines `TRUE` and `FALSE`.
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

        // Text
        assert_eq!(evaluate("!\"abc\""), Value::Bool(false));
        assert_eq!(evaluate("!\"\""), Value::Bool(true));

        // List
        assert_eq!(evaluate("![]"), Value::Bool(true));
        assert_eq!(evaluate("![1]"), Value::Bool(false));

        // Namespace
        assert_eq!(evaluate("!{}"), Value::Bool(true));
        assert_eq!(evaluate("!{a:1}"), Value::Bool(false));

        // Func
        assert_eq!(evaluate("!(x->2*x)"), Value::Bool(false));

        // Nothing: passes through `map_values!` unchanged
        assert_eq!(evaluate("!()"), Value::Nothing);

        // Tuple: negated item by item
        assert_eq!(
            evaluate("!(TRUE, 0)"),
            Value::Tuple(Rc::new(vec![
                Value::Bool(false),
                Value::Bool(true),
            ]))
        );

        // Nested tuple: flattened, then negated item by item
        assert_eq!(
            evaluate("!(0, (1, 0))"),
            Value::Tuple(Rc::new(vec![
                Value::Bool(true),
                Value::Bool(false),
                Value::Bool(true),
            ]))
        );
    }
}
