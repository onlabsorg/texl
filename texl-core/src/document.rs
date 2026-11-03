//! Documents, and where undefined values come from in them.
//!
//! [`template::render`](crate::template::render) evaluates every tag as its
//! own Swan [`Expression`], whose origin is a [`TagOrigin`]: the
//! [`Document`] being rendered (its URI and text) and where the tag is in
//! it. Swan positions are byte offsets in the expression; [`locate`] turns
//! them back into a URI and an offset in the whole document, whichever way
//! the value reached the host (another tag, `data`, a function imported
//! from another document).
//!
//! [`members`] is the list of details of an undefined value in document
//! terms (`pos` in the document, `uri`): `undef.inspect` in a Texl scope
//! gives it, and the bindings use it with `pos` converted to their unit by
//! [`utf16_offset`] (JavaScript) or [`char_offset`] (Python).

use std::fmt;
use std::rc::Rc;
use swan_core::expression::{Expression, Origin};
use swan_core::value::{BuiltinFn, Namespace, Undef, Value};


/// A template source: its text and the URI of the document, if it has one.
#[derive(Debug, PartialEq, Eq)]
pub struct Document {
    /// The URI of the document, if known.
    pub uri: Option<Rc<str>>,
    /// The whole template text.
    pub text: Rc<str>,
}

impl Document {
    /// Creates a document from its URI (if any) and text.
    pub fn new(uri: Option<&str>, text: &str) -> Self {
        Self { uri: uri.map(Rc::from), text: Rc::from(text) }
    }
}

impl From<&str> for Document {
    /// A document with no URI.
    fn from(text: &str) -> Self { Document::new(None, text) }
}

impl From<String> for Document {
    /// A document with no URI.
    fn from(text: String) -> Self { Document { uri: None, text: Rc::from(text) } }
}

impl From<&String> for Document {
    /// A document with no URI.
    fn from(text: &String) -> Self { Document::new(None, text) }
}


/// The origin of the expression of a tag: its document and where it is.
///
/// Displayed as `<uri>::<tag_start>` (`::<tag_start>` with no URI).
#[derive(Debug)]
pub struct TagOrigin {
    /// The document the tag is in, with the text that was rendered.
    pub document: Rc<Document>,
    /// Where the tag starts in the document (at `<%`): the `start` of its
    /// [`Tag`](crate::template::Tag).
    pub tag_start: usize,
    /// Where the expression starts in the document (after `<%`): Swan
    /// positions in the expression are offsets from here.
    pub expr_start: usize,
}

impl fmt::Display for TagOrigin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}::{}", self.document.uri.as_deref().unwrap_or(""), self.tag_start)
    }
}

impl TagOrigin {
    /// The origin of `expr`, if it is the expression of a tag.
    pub fn of(expr: &Expression) -> Option<&TagOrigin> {
        let origin: &dyn Origin = expr.origin.as_deref()?;
        (origin as &dyn std::any::Any).downcast_ref()
    }
}


/// Where an undefined value was made: see [`locate`].
#[derive(Debug, Clone, PartialEq)]
pub struct Location {
    /// The URI of the document, if known.
    pub uri: Option<Rc<str>>,
    /// The text `pos` refers to: the whole document for a value made by a
    /// tag, otherwise the expression (such as code run by `parse`).
    pub text: Rc<str>,
    /// The byte offset in `text` where the value was made.
    pub pos: usize,
    /// The start of the tag that made the value, for a value made by a tag.
    pub tag_start: Option<usize>,
}

/// Where `undef` was made, or `None` if it has no position (values made by
/// `undef.from`, or by a built-in function and not returned through a call).
pub fn locate(undef: &Undef) -> Option<Location> {
    let pos = undef.pos()?;
    let expr = undef.expression()?;
    Some(match TagOrigin::of(expr) {
        Some(tag) => Location {
            uri: tag.document.uri.clone(),
            text: tag.document.text.clone(),
            pos: tag.expr_start + pos,
            tag_start: Some(tag.tag_start),
        },
        None => Location { uri: None, text: expr.text.clone(), pos, tag_start: None },
    })
}

/// The details of `undef` in document terms: [`Undef::members`] with `pos`
/// a byte offset in the document and `uri` the document's URI (when it has
/// one) instead of Swan's `origin`. A value made by an expression whose
/// origin is not a tag keeps `pos` in the expression and its `origin`.
pub fn members(undef: &Undef) -> Vec<(&'static str, Value)> {
    let location = locate(undef);
    let in_tag = location.as_ref().is_some_and(|l| l.tag_start.is_some());
    let mut members = Vec::new();
    for (name, value) in undef.members() {
        match name {
            "pos" => {
                let pos = location.as_ref().map_or(value, |l| Value::Numb(l.pos as f64));
                members.push(("pos", pos));
            }
            "origin" if in_tag => {}
            _ => members.push((name, value)),
        }
    }
    if let Some(uri) = location.and_then(|l| l.uri) {
        members.push(("uri", Value::Text(uri.as_ref().into())));
    }
    members
}


/// The `undef` namespace of Texl scopes: Swan's `undef` (`swan_undef`) with
/// `inspect` giving [`members`], so positions are in the document.
pub(crate) fn undef_namespace(swan_undef: Value) -> Value {
    let mut ns = match swan_undef {
        Value::Namespace(ns) => ns,
        _ => Namespace::new(),
    };
    ns.insert("inspect", Value::Builtin(BuiltinFn::new("undef.inspect", |val| match val {
        Value::Undefined(undef) => inspect(&undef),
        other => Value::undef_unary("undef.inspect", other),
    })));
    Value::Namespace(ns)
}

/// `undef` as a namespace of its [`members`], nested undefined values
/// (operands and tuple items) inspected too.
fn inspect(undef: &Undef) -> Value {
    fn value(val: &Value) -> Value {
        match val {
            Value::Undefined(u) => inspect(u),
            Value::Tuple(items) => Value::Tuple(Rc::new(items.iter().map(value).collect())),
            other => other.clone(),
        }
    }
    Value::Namespace(members(undef).into_iter().map(|(name, val)| (name, value(&val))).collect())
}


/// The prefix of `text` up to byte `byte`, clamped to a character boundary.
fn prefix(text: &str, byte: usize) -> &str {
    let mut end = byte.min(text.len());
    while !text.is_char_boundary(end) { end -= 1; }
    &text[..end]
}

/// Converts byte offset `byte` in `text` to UTF-16 code units (JavaScript
/// string indices). An offset past the end or inside a character is clamped
/// to the previous character boundary.
pub fn utf16_offset(text: &str, byte: usize) -> usize {
    prefix(text, byte).encode_utf16().count()
}

/// Converts byte offset `byte` in `text` to code points (Python string
/// indices). An offset past the end or inside a character is clamped to the
/// previous character boundary.
pub fn char_offset(text: &str, byte: usize) -> usize {
    prefix(text, byte).chars().count()
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_offsets() {
        let s = "è°x😀y";
        assert_eq!(char_offset(s, 0), 0);
        assert_eq!(char_offset(s, 2), 1);   // after è
        assert_eq!(char_offset(s, 5), 3);   // after x
        assert_eq!(utf16_offset(s, 9), 5);  // after 😀 (2 units)
        assert_eq!(char_offset(s, 9), 4);
        assert_eq!(char_offset(s, 1), 0);   // inside è: clamped
        assert_eq!(char_offset(s, 100), 5); // past the end
    }

    #[test]
    fn test_tag_origin_display() {
        let document = Rc::new(Document::new(Some("/a.texl"), "x<% 1 %>"));
        let origin = TagOrigin { document, tag_start: 1, expr_start: 3 };
        assert_eq!(origin.to_string(), "/a.texl::1");
        let origin = TagOrigin { document: Rc::new(Document::from("<% 1 %>")), tag_start: 0, expr_start: 2 };
        assert_eq!(origin.to_string(), "::0");
    }
}
