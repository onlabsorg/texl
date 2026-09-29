//! The `numb` built-in namespace.
//!
//! Currently contains a single function:
//!
//! - `numb.from(x)` converts a `Bool` or `Text` (or a `Numb`, unchanged) to a `Numb`.
//!
//! ```text
//! numb.from(TRUE)       -> 1
//! numb.from("42")       -> 42
//! numb.from(TRUE, "42") -> (1, 42)
//! numb.from("abc")      -> undefined
//! ```

// `Undef` is kept in scope because `undef_unary!` may expand to code that refers to it.
use crate::value::{Scope, Undef, Value};
use super::helpers::*;



// ----------------------------------------------------------------------------
//  Namespace construction
// ----------------------------------------------------------------------------

/// Builds the `numb` namespace value.
///
/// The namespace scope is a child of `parent`, so root-scope names stay
/// visible inside it.
pub fn create_namespace(parent: &Scope) -> Value {
    let numb_scope = parent.new_child();
    // Unary function: `map_1` applies it to each item of the argument.
    numb_scope.set("from", map_1("numb.from", numb_from));
    Value::Namespace(numb_scope)
}



// ----------------------------------------------------------------------------
//  numb.from
// ----------------------------------------------------------------------------

/// Converts a single (non-tuple) value to a number.
///
/// - `Numb`  -> returned unchanged.
/// - `Bool`  -> `1.0` for `TRUE`, `0.0` for `FALSE`.
/// - `Text`  -> parsed with Rust's `f64::from_str`. This accepts forms such
///              as `"42"`, `"-3.5"` and `"1e3"`. It does not accept surrounding
///              whitespace, and it also accepts `"inf"` and `"NaN"`.
/// - anything else, or text that fails to parse -> `Undefined`.
///
/// `Nothing` and tuples are handled by `map_1` before this function is
/// called, so `val` is always a single item.
fn numb_from(val: Value) -> Value {
    match &val {
        Value::Numb(n) => Value::Numb(*n),
        Value::Bool(b) => Value::Numb(if *b { 1.0 } else { 0.0 }),
        Value::Text(s) => match s.parse::<f64>() {
            Ok(n)  => Value::Numb(n),
            Err(_) => undef_unary!("numb.from", val), // not a valid number literal
        },
        _ => undef_unary!("numb.from", val),          // unsupported type
    }
}



// ----------------------------------------------------------------------------
//  Unit tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;
    use crate::eval::eval;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    /// Minimal root scope: the `numb` namespace plus the boolean constants.
    /// Returns a child scope, mirroring how user code is evaluated.
    fn root_scope() -> Scope {
        let scope = Scope::new();
        scope.set("numb" , create_namespace(&scope));
        scope.set("TRUE" , Value::Bool(true));
        scope.set("FALSE", Value::Bool(false));
        scope.new_child()
    }

    /// Lexes, parses and evaluates `src` in a fresh test scope.
    fn run(src: &str) -> Value {
        let scope = root_scope();
        let mut parser = Parser::new(Lexer::new(src));
        let ast = parser.parse();
        eval(&ast, &scope)
    }

    #[test]
    fn test_from() {

        // Bool
        assert_eq!(run("numb.from(TRUE)") , Value::Numb(1.0));
        assert_eq!(run("numb.from(FALSE)"), Value::Numb(0.0));

        // Numb
        assert_eq!(run("numb.from(1.5)"), Value::Numb(1.5));

        // Text
        assert_eq!(run(r#"numb.from "42""#)   , Value::Numb(42.0));
        assert_eq!(run(r#"numb.from "-3.5""#) , Value::Numb(-3.5));
        assert_eq!(run(r#"numb.from "1e3""#)  , Value::Numb(1000.0));
        assert!(matches!(run(r#"numb.from " 42""#), Value::Undefined(_)));

        // Invalid Text
        assert!(matches!(run(r#"numb.from "abc""#), Value::Undefined(_)));
        assert!(matches!(run(r#"numb.from """#)   , Value::Undefined(_)));

        // Nothing
        assert_eq!(run("numb.from()"), Value::Nothing);

        // Tuple
        assert_eq!(
            run(r#"numb.from(TRUE, "42")"#),
            Value::Tuple(Rc::new(vec![
                Value::Numb(1.0),
                Value::Numb(42.0),
            ]))
        );

        // Others
        assert!(matches!(run("numb.from []"), Value::Undefined(_)));
    }
}
