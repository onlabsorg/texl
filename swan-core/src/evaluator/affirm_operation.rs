//! Unary affirm operation: `+X`.
//!
//! Affirm is the identity operation: it returns its operand untouched, whatever
//! its type (number, text, bool, list, namespace, function, tuple, `Nothing`,
//! `Undefined`, ...). In particular it does not require a number, and it does
//! not map over tuples or normalize them (unlike negation), because the operand
//! is passed through as a whole.

use crate::value::Value;


/// `+X` — returns `val` unchanged.
///
/// This is deliberately type-agnostic: no type check is done, so it never
/// produces an `Undefined` of its own. An `Undefined` operand is simply passed
/// through, consistent with Swan's "errors are values" semantics.
pub fn eval(val: Value) -> Value {
    val
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

    /// Parses and evaluates `src` in a scope that defines `TRUE` and `FALSE`.
    fn evaluate(src: &str) -> Value {
        let mut parser = Parser::new(Lexer::new(src));
        let ast = parser.parse();
        let scope = Scope::new();
        scope.set("TRUE", Value::Bool(true));
        scope.set("FALSE", Value::Bool(false));
        eval(&ast, &scope)
    }

    /// `+X` is the identity for every kind of value.
    #[test]
    fn affirm() {
        // Number
        assert_eq!(evaluate("+2"), Value::Numb(2.0));

        // Text: must not become Undefined (it is not restricted to numbers)
        assert_eq!(evaluate("+\"hello\""), Value::Text("hello".into()));

        // Bool
        assert_eq!(evaluate("+TRUE"), Value::Bool(true));
        assert_eq!(evaluate("+FALSE"), Value::Bool(false));

        // List
        assert_eq!(
            evaluate("+[1, 2]"),
            Value::List(Rc::new(vec![Value::Numb(1.0), Value::Numb(2.0)]))
        );

        // Nothing
        assert_eq!(evaluate("+()"), Value::Nothing);

        // Tuple: returned as is
        assert_eq!(
            evaluate("+(1, 2)"),
            Value::tuple(vec![Value::Numb(1.0), Value::Numb(2.0)])
        );
    }
}
