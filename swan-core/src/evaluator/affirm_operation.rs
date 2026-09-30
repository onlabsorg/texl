

use crate::value::Value;




/// `+X` — identity for ALL Swan value types. (#10)
///
/// In JS this is literally `return this.operand.evaluate(context)` with no
/// type guard.  Restricting it to Numb only was a bug.
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

    fn evaluate(src: &str) -> Value {
        let mut parser = Parser::new(Lexer::new(src));
        let ast = parser.parse();
        let scope = Scope::new();
        scope.set("TRUE", Value::Bool(true));
        scope.set("FALSE", Value::Bool(false));
        eval(&ast, &scope)
    }

    #[test]
    fn affirm() {

        // number
        assert_eq!(evaluate("+2"), Value::Numb(2.0));

        // +X should be a pure identity — it must NOT return Undef for
        // non-numeric types (was the bug described in review item #10).
        let text_val = Value::Text("hello".into());

        // Build a tiny AST manually: AffirmOp wrapping a Text literal.
        let scope = Scope::new();
        let text_leaf = crate::ast::Node::Text(crate::ast::Leaf { pos: 0, val: "hello".into() });
        let affirm_node = crate::ast::Node::AffirmOp(crate::ast::UnaryOp {
            pos: 0,
            opd: Box::new(text_leaf),
        });
        assert_eq!(eval(&affirm_node, &scope), text_val);

        // Nothing
        assert_eq!(evaluate("+()"), Value::Nothing);

        // Tuple
        assert_eq!(
            evaluate("+(1, 2)"), 
            Value::Tuple(Rc::new(vec![
                Value::Numb(1.0),
                Value::Numb(2.0),
            ]))
        );
    }
}
