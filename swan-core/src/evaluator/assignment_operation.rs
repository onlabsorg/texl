

//! Binding of values to names, used by assignment (`=`), labelling (`:`) and
//! function application (parameter binding).
//!
//! The caller is responsible for validating that `names` contains only
//! `Value::Name` items (see `eval_names` in the parent module).

use std::rc::Rc;
use crate::value::{Scope, Value};
use super::pairing_operation;

// ----------------------------------------------------------------------------
//  ASSIGNMENT OPERATION
// ----------------------------------------------------------------------------

/// `X = Y` - binds the names in `names` to the items in `values`.
///
/// Both operands are walked item by item (tuples are flattened, `Nothing`
/// yields no items). Bindings are always created in `scope` itself, never in
/// a parent scope.
///
/// - Name and value available: the name is bound to the value.
/// - More names than values: the surplus names are bound to `Nothing`.
/// - More values than names: each surplus value is paired (`,`) onto the value
///   of the last bound name, so `(a, b) = (1, 2, 3)` gives `a = 1`, `b = (2, 3)`.
///   If there are no names at all, the surplus values are discarded.
pub fn eval(scope: &Scope, names: &Value, values: &Value) {
    let mut names_iter = names.items();
    let mut values_iter = values.items();

    // The most recently bound name together with the value it currently holds.
    let mut last: Option<(Rc<str>, Value)> = None;

    loop {
        match (names_iter.next(), values_iter.next()) {

            // A name, with or without a matching value.
            (Some(Value::Name(name)), value) => {
                let value = value.unwrap_or(Value::Nothing);
                scope.set(name.as_ref(), value.clone());
                last = Some((name, value));
            }

            // Surplus value: append it to the value of the last bound name.
            (None, Some(value)) => {
                if let Some((name, prev)) = last.take() {
                    let paired = pairing_operation::eval(prev, value);
                    scope.set(name.as_ref(), paired.clone());
                    last = Some((name, paired));
                }
                // No name to attach to: the value is discarded.
            }

            // Both sequences are exhausted.
            (None, None) => break,

            // A non-name item among the names. Callers validate names beforehand,
            // so this is not expected; ignore it (its value is consumed).
            (Some(_), _) => {}
        }
    }
}

// ----------------------------------------------------------------------------
//  UNIT TESTS
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
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
        // --- Basic binding ---
        // `x = 5` evaluates to Nothing; the binding is only visible through
        // a subsequent lookup. Because `=` binds tighter than `,`, this
        // parses as `(x = 5), x`.
        assert_eq!(evaluate("x = 5, x"), Value::Numb(5.0));
        assert_eq!(evaluate("x = 5"), Value::Nothing);

        // Re-assigning an existing name overwrites the previous value.
        assert_eq!(evaluate("x = 1, x = 2, x"), Value::Numb(2.0));

        // --- Destructuring ---
        assert_eq!(
            evaluate("(a, b) = (1, 2), a, b"),
            Value::tuple(vec![Value::Numb(1.0), Value::Numb(2.0)])
        );

        // --- More names than values: surplus names become Nothing ---
        assert_eq!(evaluate("(a, b) = 1, a"), Value::Numb(1.0));
        assert_eq!(evaluate("(a, b) = 1, b"), Value::Nothing);
        assert_eq!(evaluate("x = 1, (x, y) = (), x"), Value::Nothing);

        // --- More values than names: surplus values are paired onto the last name ---
        assert_eq!(
            evaluate("a = (1, 2), a"),
            Value::tuple(vec![Value::Numb(1.0), Value::Numb(2.0)])
        );
        assert_eq!(
            evaluate("(a, b) = (1, 2, 3), b"),
            Value::tuple(vec![Value::Numb(2.0), Value::Numb(3.0)])
        );
        assert_eq!(evaluate("(a, b) = (1, 2, 3), a"), Value::Numb(1.0));

        // No names at all: values are discarded and nothing is bound.
        assert_eq!(evaluate("() = 5"), Value::Nothing);
        assert_eq!(evaluate("() = 5, __uname__"), Value::Nothing);

        // --- Scope: bindings are created in the given scope only ---
        let parent = Scope::new();
        parent.set("x", Value::Numb(1.0));
        let child = parent.new_child();
        super::eval(&child, &Value::Name("x".into()), &Value::Numb(2.0));
        assert_eq!(child.get("x"), Value::Numb(2.0));
        assert_eq!(parent.get("x"), Value::Numb(1.0));

        // --- Invalid left-hand names (reported by the evaluator before binding) ---
        assert_eq!(
            evaluate("(x, 12) = (1, 2)"), 
            Value::undef_binary("assignment", Value::Undefined(Undef::Name{pos:4}), Value::Nothing, 8),
        );
    }
}
