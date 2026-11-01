//! Source texts and the source of the code being evaluated.
//!
//! An undefined value records where it was made: a byte offset (`pos`) and
//! the [`Source`] that offset refers to. A host can then find the error in the
//! text and convert the offset to its own unit (UTF-16 units, code points),
//! whichever way the value reached it.
//!
//! The evaluator keeps the source of the code it is running in a
//! thread-local slot, the *current source*:
//!
//! - a host sets it around a top-level evaluation with [`with_source`];
//! - a function captures the current source when it is defined, and its
//!   body runs with that source, so an error raised inside a function
//!   imported from another document points into that document;
//! - a built-in function runs with no source: built-ins report position `0`
//!   and no source, meaning "no position".
//!
//! Undefined values made by [`Value::undef_unary`](crate::value::Value::undef_unary),
//! [`Value::undef_binary`](crate::value::Value::undef_binary) and by the
//! evaluator take the current source.
//!
//! ```
//! use std::rc::Rc;
//! use swan_core::builtins::root_scope;
//! use swan_core::evaluator::eval;
//! use swan_core::lexer::Lexer;
//! use swan_core::parser::Parser;
//! use swan_core::source::{self, Source};
//! use swan_core::value::Value;
//!
//! let text = r#"2 * ("a" - 1)"#;
//! let src = Rc::new(Source::new(Some("doc.texl"), text));
//! let ast = Parser::new(Lexer::new(text)).parse();
//! let val = source::with_source(Some(src), || eval(&ast, &root_scope()));
//! let Value::Undefined(undef) = val else { panic!() };
//! assert_eq!(undef.pos(), Some(2));
//! assert_eq!(undef.cause().unwrap().pos(), Some(9));
//! assert_eq!(undef.cause().unwrap().describe(), r#"cannot subtract the number `1` from the text `"a"`"#);
//! assert_eq!(undef.source().unwrap().uri.as_deref(), Some("doc.texl"));
//! ```

use std::cell::RefCell;
use std::rc::Rc;


/// A source text, such as a document, and its URI if it has one.
///
/// Positions in undefined values are byte offsets in `text`.
#[derive(Debug, PartialEq, Eq)]
pub struct Source {
    /// The URI of the document, if known.
    pub uri: Option<Rc<str>>,
    /// The whole source text.
    pub text: Rc<str>,
}

impl Source {
    /// Creates a source from its URI (if any) and text.
    pub fn new(uri: Option<&str>, text: &str) -> Self {
        Self { uri: uri.map(Rc::from), text: Rc::from(text) }
    }

    /// Converts a byte offset in `text` to UTF-16 code units (JavaScript
    /// string indices). An offset past the end or inside a character is
    /// clamped to the previous character boundary.
    pub fn utf16_offset(&self, byte: usize) -> usize {
        utf16_offset(&self.text, byte)
    }

    /// Converts a byte offset in `text` to code points (Python string
    /// indices). An offset past the end or inside a character is clamped to
    /// the previous character boundary.
    pub fn char_offset(&self, byte: usize) -> usize {
        char_offset(&self.text, byte)
    }
}

impl From<&str> for Source {
    /// A source with no URI.
    fn from(text: &str) -> Self { Source::new(None, text) }
}

impl From<String> for Source {
    /// A source with no URI.
    fn from(text: String) -> Self { Source { uri: None, text: Rc::from(text) } }
}

impl From<&String> for Source {
    /// A source with no URI.
    fn from(text: &String) -> Self { Source::new(None, text) }
}

/// The prefix of `text` up to byte `byte`, clamped to a character boundary.
fn prefix(text: &str, byte: usize) -> &str {
    let mut end = byte.min(text.len());
    while !text.is_char_boundary(end) { end -= 1; }
    &text[..end]
}

/// Converts byte offset `byte` in `text` to UTF-16 code units.
pub fn utf16_offset(text: &str, byte: usize) -> usize {
    prefix(text, byte).encode_utf16().count()
}

/// Converts byte offset `byte` in `text` to code points.
pub fn char_offset(text: &str, byte: usize) -> usize {
    prefix(text, byte).chars().count()
}


thread_local! {
    static CURRENT: RefCell<Option<Rc<Source>>> = const { RefCell::new(None) };
}

/// The source of the code being evaluated, if any.
pub fn current() -> Option<Rc<Source>> {
    CURRENT.with(|c| c.borrow().clone())
}

/// Runs `f` with `source` as the current source, then restores the previous
/// one (also if `f` panics).
pub fn with_source<R>(source: Option<Rc<Source>>, f: impl FnOnce() -> R) -> R {
    struct Restore(Option<Rc<Source>>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let prev = self.0.take();
            CURRENT.with(|c| *c.borrow_mut() = prev);
        }
    }
    let prev = CURRENT.with(|c| std::mem::replace(&mut *c.borrow_mut(), source));
    let _restore = Restore(prev);
    f()
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_offsets() {
        let s = Source::new(None, "è°x😀y");
        assert_eq!(s.char_offset(0), 0);
        assert_eq!(s.char_offset(2), 1);   // after è
        assert_eq!(s.char_offset(5), 3);   // after x
        assert_eq!(s.utf16_offset(9), 5);  // after 😀 (2 units)
        assert_eq!(s.char_offset(9), 4);
        assert_eq!(s.char_offset(1), 0);   // inside è: clamped
        assert_eq!(s.char_offset(100), 5); // past the end
    }

    #[test]
    fn test_with_source_restores() {
        let a = Rc::new(Source::new(Some("a"), ""));
        let b = Rc::new(Source::new(Some("b"), ""));
        assert!(current().is_none());
        with_source(Some(a.clone()), || {
            assert_eq!(current(), Some(a.clone()));
            with_source(Some(b.clone()), || assert_eq!(current(), Some(b.clone())));
            with_source(None, || assert!(current().is_none()));
            assert_eq!(current(), Some(a.clone()));
        });
        assert!(current().is_none());
    }
}
