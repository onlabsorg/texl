//! Expressions being evaluated, and where they come from.
//!
//! An undefined value records where it was made: a byte offset (`pos`) in
//! the text of the [`Expression`] that made it. An expression may carry an
//! [`Origin`], set by the host and opaque to Swan, that tells where the text
//! comes from (a file, a part of a document, …); the host gets its own origin
//! back by downcasting and uses it to place the error in its own terms.
//!
//! The evaluator keeps the expression it is running in a thread-local slot,
//! the *current expression*:
//!
//! - a host sets it around a top-level evaluation with [`with_expression`];
//! - a function captures the current expression when it is defined, and its
//!   body runs with it, so an error raised inside a function points into the
//!   expression that defined it, wherever the function is called from;
//! - a built-in function runs with no expression: it makes undefined values
//!   with no position, and the evaluator gives them the position of the call
//!   in the current expression when the built-in returns them;
//! - code compiled by the built-in `parse` runs as an expression of its own,
//!   with no origin.
//!
//! Undefined values made by the evaluator (and by
//! [`Value::undef_unary_at`](crate::value::Value::undef_unary_at),
//! [`Value::undef_binary_at`](crate::value::Value::undef_binary_at)) take
//! the current expression.
//!
//! ```
//! use std::any::Any;
//! use std::rc::Rc;
//! use swan_core::builtins::root_scope;
//! use swan_core::evaluator::eval;
//! use swan_core::expression::{self, Expression};
//! use swan_core::lexer::Lexer;
//! use swan_core::parser::Parser;
//! use swan_core::value::Value;
//!
//! let text = r#"2 * ("a" - 1)"#;
//! let expr = Rc::new(Expression::new(text, Some(Rc::new("doc.swan".to_string()))));
//! let ast = Parser::new(Lexer::new(text)).parse();
//! let val = expression::with_expression(Some(expr), || eval(&ast, &root_scope()));
//! let Value::Undefined(undef) = val else { panic!() };
//! assert_eq!(undef.pos(), Some(2));
//! assert_eq!(undef.cause().unwrap().pos(), Some(9));
//! assert_eq!(undef.cause().unwrap().describe(), r#"cannot subtract the number `1` from the text `"a"`"#);
//!
//! // The host gets its origin back by downcasting.
//! let origin = undef.expression().unwrap().origin.as_deref().unwrap();
//! assert_eq!(origin.to_string(), "doc.swan");
//! let name: &String = (origin as &dyn Any).downcast_ref().unwrap();
//! assert_eq!(name, "doc.swan");
//! ```

use std::any::Any;
use std::cell::RefCell;
use std::fmt::{self, Debug, Display};
use std::rc::Rc;


/// Where an expression comes from, as the host knows it.
///
/// Swan only displays an origin (`undef.inspect` shows it as text); the host
/// defines its type, with whatever information it needs, and gets it back by
/// downcasting (`origin as &dyn Any`). Every type that is `Any + Display` is
/// an origin, so a plain `String` will do.
///
/// An origin must not hold Swan values (scopes, functions, promises): the
/// cycle collector cannot see inside it, so it would never free them.
pub trait Origin: Any + Display {}

impl<T: Any + Display> Origin for T {}


/// The text of an expression and, if the host gave one, its origin.
///
/// Positions in undefined values are byte offsets in `text`.
pub struct Expression {
    /// The text of the expression.
    pub text: Rc<str>,
    /// Where the text comes from, if the host said so.
    pub origin: Option<Rc<dyn Origin>>,
}

impl Expression {
    /// Creates an expression from its text and origin (if any).
    pub fn new(text: &str, origin: Option<Rc<dyn Origin>>) -> Self {
        Self { text: Rc::from(text), origin }
    }
}

impl Debug for Expression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Expression")
            .field("text", &self.text)
            .field("origin", &self.origin.as_ref().map(|o| o.to_string()))
            .finish()
    }
}

impl From<&str> for Expression {
    /// An expression with no origin.
    fn from(text: &str) -> Self { Expression::new(text, None) }
}

impl From<String> for Expression {
    /// An expression with no origin.
    fn from(text: String) -> Self { Expression { text: Rc::from(text), origin: None } }
}


thread_local! {
    static CURRENT: RefCell<Option<Rc<Expression>>> = const { RefCell::new(None) };
}

/// The expression being evaluated, if any.
pub fn current() -> Option<Rc<Expression>> {
    CURRENT.with(|c| c.borrow().clone())
}

/// Runs `f` with `expression` as the current expression, then restores the
/// previous one (also if `f` panics).
pub fn with_expression<R>(expression: Option<Rc<Expression>>, f: impl FnOnce() -> R) -> R {
    struct Restore(Option<Rc<Expression>>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let prev = self.0.take();
            CURRENT.with(|c| *c.borrow_mut() = prev);
        }
    }
    let prev = CURRENT.with(|c| std::mem::replace(&mut *c.borrow_mut(), expression));
    let _restore = Restore(prev);
    f()
}


#[cfg(test)]
mod tests {
    use super::*;

    fn same(a: &Option<Rc<Expression>>, b: &Rc<Expression>) -> bool {
        a.as_ref().is_some_and(|a| Rc::ptr_eq(a, b))
    }

    #[test]
    fn test_with_expression_restores() {
        let a = Rc::new(Expression::from("a"));
        let b = Rc::new(Expression::from("b"));
        assert!(current().is_none());
        with_expression(Some(a.clone()), || {
            assert!(same(&current(), &a));
            with_expression(Some(b.clone()), || assert!(same(&current(), &b)));
            with_expression(None, || assert!(current().is_none()));
            assert!(same(&current(), &a));
        });
        assert!(current().is_none());
    }

    #[test]
    fn test_origin_downcast() {
        struct Place { line: usize }
        impl Display for Place {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "line {}", self.line) }
        }
        let expr = Expression::new("1", Some(Rc::new(Place { line: 7 })));
        let origin = expr.origin.as_deref().unwrap();
        assert_eq!(origin.to_string(), "line 7");
        let place: &Place = (origin as &dyn Any).downcast_ref().unwrap();
        assert_eq!(place.line, 7);
        assert!((origin as &dyn Any).downcast_ref::<String>().is_none());
    }
}
