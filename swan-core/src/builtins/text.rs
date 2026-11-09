
//! The `text` built-in namespace.
//!
//! Every function is applied element-wise over tuples (via `map_1` / `map_curried`),
//! so `Nothing` maps to `Nothing` and tuples map to tuples. Arguments of the wrong
//! type produce `Value::Undefined`. All positions and lengths count Unicode
//! characters, not bytes.

use crate::evaluator::apply;
use crate::value::{BuiltinFn, Namespace, Value};
use super::helpers::*;


// ----------------------------------------------------------------------------
//  `text` built-in namespace 
// ----------------------------------------------------------------------------

/// Builds the `text` namespace and returns it as a `Value::Namespace`.
pub fn create_namespace() -> Value {
    let mut ns = Namespace::new();

    // Unary functions: map over each item of the argument.
    ns.insert("from"     , map_1("text.from"     , to_text));
    ns.insert("size"     , map_1("text.size"     , text_size));
    ns.insert("enum"     , map_1("text.enum"     , text_enum));
    ns.insert("lower"    , map_1("text.lower"    , text_lower));
    ns.insert("upper"    , map_1("text.upper"    , text_upper));
    ns.insert("trim"     , map_1("text.trim"     , text_trim));
    ns.insert("trim_head", map_1("text.trim_head", text_trim_head));
    ns.insert("trim_tail", map_1("text.trim_tail", text_trim_tail));

    // Curried binary functions: (needle/count/separator) first, then the target text.
    ns.insert("find"     , map_curried("text.find" , text_find));
    ns.insert("rfind"    , map_curried("text.rfind", text_rfind));
    ns.insert("head"     , map_curried("text.head" , text_head));
    ns.insert("tail"     , map_curried("text.tail" , text_tail));
    ns.insert("split"    , map_curried("text.split", text_split));
    // `join` uses map_1 (not map_curried) on purpose: the separator is mapped, but the
    // returned function receives the whole tuple of items to join.
    ns.insert("join"     , map_1("text.join"     , text_join));
    // `template` maps over templates; the function it returns maps over namespaces.
    ns.insert("template" , map_1("text.template" , text_template));

    Value::Namespace(ns)
}

// ----------------------------------------------------------------------------
//  Conversion / inspection
// ----------------------------------------------------------------------------

/// The name of the namespace member that gives a namespace its text form.
///
/// A namespace with this member is converted to text by applying the member
/// to the namespace itself; see [`to_text`].
pub const TEXT_HOOK: &str = "__text__";

/// Converts a single value to text, the one conversion shared by every
/// stringifier: Swan's `text.from` (which maps it over tuples) and Texl's
/// template renderer.
///
/// | Value                  | Text                                        |
/// |------------------------|---------------------------------------------|
/// | `Text`                 | the text itself                             |
/// | `Numb`                 | Rust `f64` display (`30`, not `30.0`; `1.5`) |
/// | `Bool`                 | `TRUE` / `FALSE`                            |
/// | `List`                 | `[[List of N]]`                             |
/// | `Namespace` with `__text__` | what `__text__` returns when applied to the namespace |
/// | `Namespace`            | `[[Namespace of N]]` (N = number of members) |
/// | `Func` / `Builtin`     | `[[Func]]`                                  |
/// | `Undefined`            | `[[Undefined]]`                             |
/// | `Name`                 | the name                                    |
///
/// The `__text__` member ([`TEXT_HOOK`]) is applied to the namespace and must
/// return a text. Anything else it returns (a number, a tuple, a promise, an
/// undefined value…) makes the conversion return `Undefined` (operation
/// `__text__`, the namespace in `lhv`, the returned value in `rhv`). The
/// returned text is used as it is: it is not converted again.
///
/// Anything else (`Nothing`, a tuple, a promise) gives `Undefined`: callers
/// handle those themselves (`text.from` maps over tuple items, the renderer
/// joins them and awaits promises). Values made here have no position.
pub fn to_text(val: Value) -> Value {
    let s: String = match &val {
        Value::Bool(b) => {
            if *b { "TRUE".into() } else { "FALSE".into() }
        }
        Value::Numb(n) => {
            n.to_string()
        }
        Value::Text(s) => {
            return Value::Text(s.clone())
        }
        Value::Name(n) => {
            n.to_string()
        }
        Value::List(l) => {
            format!("[[List of {}]]", l.len())
        }
        Value::Func{..} | Value::Builtin(_) => {
            "[[Func]]".to_string()
        }
        Value::Undefined(_) => {
            "[[Undefined]]".to_string()
        }
        Value::Namespace(ns) if ns.contains(TEXT_HOOK) => {
            return match apply(ns.get(TEXT_HOOK), val.clone()) {
                text @ Value::Text(_) => text,
                other => Value::undef_binary(TEXT_HOOK, val, other),
            };
        }
        Value::Namespace(ns) => {
            format!("[[Namespace of {}]]", ns.len())
        }
        Value::Nothing | Value::Tuple(_) | Value::Promise(_) => {
            return Value::undef_unary("text.from", val)
        }
    };

    Value::Text(s.into())
}

/// Converts any value to the text a Texl tag renders for it.
///
/// Single values go through [`to_text`]; this function adds what `to_text`
/// leaves to its callers, and never fails:
///
/// | Value                  | Text                                          |
/// |------------------------|-----------------------------------------------|
/// | `Nothing`              | empty text                                    |
/// | `Tuple`                | items converted and joined with `", "`        |
/// | `Promise` (resolved)   | the text of the resolved value                |
/// | `Promise` (pending)    | `[[Pending Promise]]`                         |
/// | anything `to_text` cannot convert | `[[Undefined]]`                    |
///
/// Used by Texl's template renderer and by `text.template`, so a placeholder
/// shows a value the way a tag does. A pending promise is not awaited.
pub fn to_rendered_text(val: Value) -> String {
    match val {
        Value::Nothing => String::new(),
        Value::Tuple(items) => {
            let item_strs: Vec<String> = items.iter().cloned().map(to_rendered_text).collect();
            item_strs.join(", ")
        }
        // Non-blocking: read the promise's current value without awaiting it.
        Value::Promise(promise) => {
            let inner_value = promise.0.borrow().value.clone();
            match inner_value {
                Some(resolved_val) => to_rendered_text(resolved_val),
                None => "[[Pending Promise]]".to_string(),
            }
        }
        other => match to_text(other) {
            Value::Text(s) => s.to_string(),
            _ => "[[Undefined]]".to_string(),
        },
    }
}

/// Number of characters in the text.
fn text_size(val: Value) -> Value {
    match val {
        Value::Text(s) => Value::Numb(s.chars().count() as f64),
        other => Value::undef_unary("text.size", other),
    }
}

/// Splits the text into a tuple of single-character texts.
fn text_enum(val: Value) -> Value {
    match val {
        Value::Text(s) => {
            let chars: Vec<Value> = s.chars().map(|c| Value::Text(c.to_string().into())).collect();
            Value::tuple(chars)
        }
        other => Value::undef_unary("text.enum", other),
    }
}

// ----------------------------------------------------------------------------
//  Case and whitespace
// ----------------------------------------------------------------------------

/// Lower-cases the text.
fn text_lower(val: Value) -> Value {
    match val {
        Value::Text(s) => Value::Text(s.to_lowercase().into()),
        other => Value::undef_unary("text.lower", other),
    }
}

/// Upper-cases the text.
fn text_upper(val: Value) -> Value {
    match val {
        Value::Text(s) => Value::Text(s.to_uppercase().into()),
        other => Value::undef_unary("text.upper", other),
    }
}

/// Removes whitespace from both ends of the text.
fn text_trim(val: Value) -> Value {
    match val {
        Value::Text(s) => Value::Text(s.trim().into()),
        other => Value::undef_unary("text.trim", other),
    }
}

/// Removes whitespace from the start of the text.
fn text_trim_head(val: Value) -> Value {
    match val {
        Value::Text(s) => Value::Text(s.trim_start().into()),
        other => Value::undef_unary("text.trim_head", other),
    }
}

/// Removes whitespace from the end of the text.
fn text_trim_tail(val: Value) -> Value {
    match val {
        Value::Text(s) => Value::Text(s.trim_end().into()),
        other => Value::undef_unary("text.trim_tail", other),
    }
}

// ----------------------------------------------------------------------------
//  Searching
// ----------------------------------------------------------------------------

/// Converts an optional byte offset within `haystack` into a character index
/// (`Value::Numb`), or `-1` when there was no match.
fn char_position(haystack: &str, byte_pos: Option<usize>) -> Value {
    match byte_pos {
        Some(b) => Value::Numb(haystack[..b].chars().count() as f64),
        None => Value::Numb(-1.0),
    }
}

/// Character index of the first occurrence of `needle` in `haystack`, or -1 if absent. Only `haystack` is reported in the Undefined result if either argument is not Text.
fn text_find(needle: Value, haystack: Value) -> Value {
    match (&needle, haystack) {
        (Value::Text(n), Value::Text(h)) => char_position(&h, h.find(n.as_ref())),
        (_, other) => Value::undef_unary("text.find", other),
    }
}

/// Character index of the last occurrence of `needle` in `haystack`, or -1 if absent. Only `haystack` is reported in the Undefined result if either argument is not Text.
fn text_rfind(needle: Value, haystack: Value) -> Value {
    match (&needle, haystack) {
        (Value::Text(n), Value::Text(h)) => char_position(&h, h.rfind(n.as_ref())),
        (_, other) => Value::undef_unary("text.rfind", other),
    }
}

// ----------------------------------------------------------------------------
//  Slicing
// ----------------------------------------------------------------------------

/// Resolves a signed character count `n` against a text of `len` characters
/// into a boundary index in `0..=len`.
/// - `n >= 0`: `n`, clamped to `len` (counted from the start)
/// - `n < 0`:  `len + n`, clamped to `0` (counted back from the end)
fn boundary_index(n: f64, len: usize) -> usize {
    let (len, n) = (len as i64, n as i64);
    if n >= 0 { n.min(len) as usize } else { (len + n).max(0) as usize }
}

/// First `n` characters; a negative `n` drops the last `|n|` characters instead.
fn text_head(n: Value, s: Value) -> Value {
    match (&n, s) {
        (Value::Numb(n_val), Value::Text(s_val)) => {
            let chars: Vec<char> = s_val.chars().collect();
            let end = boundary_index(*n_val, chars.len());
            let result: String = chars[..end].iter().collect();
            Value::Text(result.into())
        }
        (_, other) => Value::undef_unary("text.head", other),
    }
}

/// Text from character index `n` onward; a negative `n` keeps only the last `|n|` characters.
fn text_tail(n: Value, s: Value) -> Value {
    match (&n, s) {
        (Value::Numb(n_val), Value::Text(s_val)) => {
            let chars: Vec<char> = s_val.chars().collect();
            let start = boundary_index(*n_val, chars.len());
            let result: String = chars[start..].iter().collect();
            Value::Text(result.into())
        }
        (_, other) => Value::undef_unary("text.tail", other),
    }
}

// ----------------------------------------------------------------------------
//  Split / join
// ----------------------------------------------------------------------------

/// Splits `target` on every occurrence of `sep`, returning a tuple of parts (a single Text if `sep` does not occur).
fn text_split(sep: Value, target: Value) -> Value {
    match (&sep, target) {
        (Value::Text(s1), Value::Text(s2)) => {
            let parts: Vec<Value> = s2
                .split(s1.as_ref())
                .map(|p| Value::Text(p.into()))
                .collect();
            Value::tuple(parts)
        }
        (_, other) => Value::undef_unary("text.split", other),
    }
}

/// Curried: `text.join sep items` joins all Text items with `sep`. Returns Undefined if `sep` is not Text or if any item is not Text.
fn text_join(sep: Value) -> Value {
    Value::Builtin(BuiltinFn::capturing("text.join", vec![sep], |captured, items| {
        match &captured[0] {
            Value::Text(s1) => {
                // Validate and collect first, so one non-Text item aborts the whole join.
                let mut strings: Vec<String> = Vec::new();
                for item in items.items() {
                    match item {
                        Value::Text(t) => strings.push(t.to_string()),
                        other => return Value::undef_unary("text.join", other), // `return` exits the whole builtin closure, not just the loop.
                    }
                }
                Value::Text(strings.join(s1.as_ref()).into())
            }
            _ => Value::undef_unary("text.join", items),
        }
    }))
}


// ----------------------------------------------------------------------------
//  Templates
// ----------------------------------------------------------------------------

/// A piece of a parsed `text.template` template.
#[derive(Debug, PartialEq)]
enum Segment {
    /// Text copied to the output as it is.
    Literal(String),
    /// A `{name}` placeholder.
    Name(String),
}

/// Splits a template into literal text and `{name}` placeholders.
///
/// - `{name}`, where `name` is a Swan name (a letter or `_`, then letters,
///   digits or `_`), is a placeholder.
/// - `{{` stands for `{` and `}}` for `}`.
/// - Any other `{` or `}` is literal text, so a template never fails to parse.
fn parse_template(template: &str) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut literal = String::new();
    let mut rest = template;

    while let Some(ch) = rest.chars().next() {
        if let Some(after) = rest.strip_prefix("{{") {
            literal.push('{');
            rest = after;
        } else if let Some(after) = rest.strip_prefix("}}") {
            literal.push('}');
            rest = after;
        } else if let Some((name, after)) = placeholder(rest) {
            if !literal.is_empty() {
                segments.push(Segment::Literal(std::mem::take(&mut literal)));
            }
            segments.push(Segment::Name(name.to_string()));
            rest = after;
        } else {
            literal.push(ch);
            rest = &rest[ch.len_utf8()..];
        }
    }
    if !literal.is_empty() {
        segments.push(Segment::Literal(literal));
    }
    segments
}

/// If `s` starts with a `{name}` placeholder, returns the name and the text
/// after the closing `}`.
fn placeholder(s: &str) -> Option<(&str, &str)> {
    let body = s.strip_prefix('{')?;
    let end = body.find('}')?;
    let name = &body[..end];
    let mut chars = name.chars();
    let first = chars.next()?;
    let is_name = (first == '_' || first.is_alphabetic())
        && chars.all(|c| c == '_' || c.is_alphanumeric());
    is_name.then(|| (name, &body[end + 1..]))
}

/// Curried: `text.template tpl ns` replaces each `{name}` placeholder of `tpl`
/// with the text of `ns.name`, converted as a Texl tag renders it
/// ([`to_rendered_text`]): a missing name is `Nothing`, so it gives an empty
/// text. The template is parsed once, when `text.template` is applied to it.
/// The returned function maps over a tuple of namespaces; anything else that
/// is not a namespace gives `Undefined`.
fn text_template(template: Value) -> Value {
    let segments = match &template {
        Value::Text(t) => std::rc::Rc::new(parse_template(t)),
        _ => return Value::undef_unary("text.template", template),
    };
    // The segments hold no Swan values, so the cycle collector need not see them.
    Value::Builtin(BuiltinFn::new("text.template", move |arg| {
        let segments = segments.clone();
        arg.map(move |item| match item {
            Value::Namespace(ns) => {
                let mut out = String::new();
                for segment in segments.iter() {
                    match segment {
                        Segment::Literal(s) => out.push_str(s),
                        Segment::Name(n) => out.push_str(&to_rendered_text(ns.get(n))),
                    }
                }
                Value::Text(out.into())
            }
            other => Value::undef_unary("text.template", other),
        })
    }))
}


// ----------------------------------------------------------------------------
//  UNIT TESTS
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::{Scope, Undef};
    use crate::evaluator::eval;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn root_scope() -> Scope {
        let scope = Scope::new();
        scope.set("text", create_namespace());
        scope.set("TRUE", Value::Bool(true));
        scope.set("FALSE", Value::Bool(false));
        scope.new_child()
    }

    fn run(src: &str) -> Value {
        let scope = root_scope();
        let mut parser = Parser::new(Lexer::new(src));
        let ast = parser.parse();
        eval(&ast, &scope)
    }

    #[test]
    fn text_from_hook() {
        // The hook is applied to the namespace itself
        assert_eq!(run("text.from{n: 2, __text__: self -> \"n=\" + text.from(self.n * 2)}"), Value::Text("n=4".into()));

        // Mapped over tuples like any other value
        assert_eq!(
            run("t = {__text__: s -> \"T\"}, text.from(t, 1, t)"),
            Value::tuple(vec![Value::Text("T".into()), Value::Text("1".into()), Value::Text("T".into())]),
        );

        // A hook that does not return a text gives Undefined
        for src in [
            "text.from{__text__: s -> 1}",
            "text.from{__text__: s -> (\"a\", \"b\")}",
            "text.from{__text__: s -> []-[]}",
            "text.from{__text__: \"abc\"}",
            "text.from{__text__: ()}",
        ] {
            match run(src) {
                Value::Undefined(u) => {
                    assert!(matches!(&u, Undef::BinaryOperation { op: "__text__", lhv, .. } if matches!(**lhv, Value::Namespace(_))), "{src}");
                    assert!(u.describe().starts_with("the `__text__` member of a namespace must return a text"), "{src}");
                    // text.from places the error at its call
                    assert_eq!(u.pos(), Some(9), "{src}");
                }
                other => panic!("{src}: expected Undefined, got {other:?}"),
            }
        }

        // The cause of a failed hook is reachable
        match run("text.from{__text__: s -> []-[]}") {
            Value::Undefined(u) => assert!(u.cause().is_some()),
            other => panic!("expected Undefined, got {other:?}"),
        }
    }

    #[test]
    fn text_template() {
        let text = |s: &str| Value::Text(s.into());

        // The example of the manual
        assert_eq!(
            run(r#"tfunc = text.template "My car is {color} and it is from {year}.", tfunc {color: "white", year: 2012}"#),
            text("My car is white and it is from 2012."),
        );

        // Values are converted as a tag renders them
        assert_eq!(run(r#"text.template "{b}|{n}|{l}|{s}" {b: TRUE, n: 1.5, l: [1, 2], s: {x: 1}}"#), text("TRUE|1.5|[[List of 2]]|[[Namespace of 1]]"));
        assert_eq!(run(r#"text.template "[{t}]" {t: (1, "a", 2)}"#), text("[1, a, 2]"));
        assert_eq!(run(r#"text.template "[{u}]" {u: [] - []}"#), text("[[[Undefined]]]"));
        assert_eq!(run(r#"text.template "{m}" {m: {__text__: s -> "M"}}"#), text("M"));

        // A missing name is Nothing: an empty text
        assert_eq!(run(r#"text.template "[{colour}]" {color: "red"}"#), text("[]"));

        // A name may repeat; names follow Swan's rules
        assert_eq!(run(r#"text.template "{a}{a}{_b2}{è}" {a: 1, _b2: 2, è: 3}"#), text("1123"));

        // `{{` and `}}` are braces; other braces are literal text
        assert_eq!(run(r#"text.template "{{a}} {a} }} {{" {a: 1}"#), text("{a} 1 } {"));
        assert_eq!(run(r#"text.template "{ a } {1a} {a.b} {} {a" {a: 1}"#), text("{ a } {1a} {a.b} {} {a"));
        assert_eq!(run(r#"text.template "{{{a}}}" {a: 1}"#), text("{1}"));

        // No placeholders, empty template
        assert_eq!(run(r#"text.template "plain" {}"#), text("plain"));
        assert_eq!(run(r#"text.template "" {a: 1}"#), text(""));

        // Mapped over tuples of templates and of namespaces
        assert_eq!(
            run(r#"text.template ("<{a}>", "[{a}]") {a: 1}"#),
            Value::tuple(vec![text("<1>"), text("[1]")]),
        );
        assert_eq!(
            run(r#"text.template "<{a}>" ({a: 1}, {a: 2})"#),
            Value::tuple(vec![text("<1>"), text("<2>")]),
        );
        assert_eq!(run(r#"text.template "<{a}>" ()"#), Value::Nothing);
        assert_eq!(run("text.template ()"), Value::Nothing);

        // Wrong arguments give Undefined, placed at the call
        for (src, desc, pos) in [
            ("text.template 1", "`text.template` is not defined for the number `1`", 14),
            (r#"text.template "{a}" [1]"#, "`text.template` is not defined for a list", 20),
            (r#"t = text.template "{a}", t "x""#, r#"`text.template` is not defined for the text `"x"`"#, 27),
        ] {
            match run(src) {
                Value::Undefined(u) => {
                    assert_eq!(u.describe(), desc, "{src}");
                    assert_eq!(u.pos(), Some(pos), "{src}");
                }
                other => panic!("{src}: expected Undefined, got {other:?}"),
            }
        }
    }

    #[test]
    fn parse_template_segments() {
        use Segment::*;
        assert_eq!(
            parse_template("a{x}b{{c}}{y}"),
            vec![Literal("a".into()), Name("x".into()), Literal("b{c}".into()), Name("y".into())],
        );
        assert_eq!(parse_template(""), vec![]);
    }

    #[test]
    fn rendered_text() {
        assert_eq!(to_rendered_text(Value::Nothing), "");
        assert_eq!(to_rendered_text(Value::tuple(vec![Value::Numb(1.0), Value::Text("a".into())])), "1, a");
        let pending = crate::value::Promise::new();
        assert_eq!(to_rendered_text(Value::Promise(pending)), "[[Pending Promise]]");
    }

    #[test]
    fn text_from() {

        // Number
        assert_eq!(run("text.from 42"), Value::Text("42".into()));

        // Bool
        assert_eq!(run("text.from(TRUE)"), Value::Text("TRUE".into()));
        assert_eq!(run("text.from(FALSE)"), Value::Text("FALSE".into()));

        // Text
        assert_eq!(run("text.from \"abc\""), Value::Text("abc".into()));

        // List
        assert_eq!(run("text.from[10,20,30,40]"), Value::Text("[[List of 4]]".into()));
        
        // Namespace
        assert_eq!(run("text.from{a:1, b:2, c:3}"), Value::Text("[[Namespace of 3]]".into()));

        // Func
        assert_eq!(run("text.from(x -> 2*x)"), Value::Text("[[Func]]".into()));
        assert_eq!(run("text.from(text.from)"), Value::Text("[[Func]]".into()));

        // Undefined
        assert_eq!(run("text.from([] - [])"), Value::Text("[[Undefined]]".into()));

        // Nothing
        assert_eq!(run("text.from()"), Value::Nothing);

        // Tuple
        assert_eq!(
            run("text.from(21, TRUE)"),
            Value::tuple(vec![
                Value::Text("21".into()),
                Value::Text("TRUE".into()),
            ]),
        );
    }

    #[test]
    fn text_size() {

        // Text
        assert_eq!(run("text.size \"abc\""), Value::Numb(3.0));
        assert_eq!(run("text.size \"\"   "), Value::Numb(0.0));

        // Nothing
        assert_eq!(run("text.size()"), Value::Nothing);

        // Tuple
        assert_eq!(
            run("text.size(\"abc\", \"defgh\")"), 
            Value::tuple(vec![
                Value::Numb(3.0),
                Value::Numb(5.0),
            ])
        );

        // Others
        assert!(matches!(run("text.size 123"), Value::Undefined(_)));
    }

    #[test]
    fn text_enum() {

        // Text
        assert_eq!(
            run(r#"text.enum "abc""#),
            Value::tuple(vec![
                Value::Text("a".into()),
                Value::Text("b".into()),
                Value::Text("c".into()),
            ])
        );

        // Nothing
        assert_eq!(run("text.enum()"), Value::Nothing);

        // Tuple
        assert_eq!(
            run("text.enum(\"abc\", \"de\")"), 
            Value::tuple(vec![
                Value::Text("a".into()),
                Value::Text("b".into()),
                Value::Text("c".into()),
                Value::Text("d".into()),
                Value::Text("e".into()),
            ])
        );

        // Others
        assert!(matches!(run("text.enum 123"), Value::Undefined(_)));
    }

    #[test]
    fn text_lower() {

        // Text
        assert_eq!(run("text.lower \"ABC\""), Value::Text("abc".into()));
        assert_eq!(run("text.lower \"aBc\""), Value::Text("abc".into()));
        assert_eq!(run("text.lower \"abc\""), Value::Text("abc".into()));

        // Nothing
        assert_eq!(run("text.lower()"), Value::Nothing);

        // Tuple
        assert_eq!(
            run("text.lower(\"ABC\", \"DEF\")"), 
            Value::tuple(vec![
                Value::Text("abc".into()),
                Value::Text("def".into()),
            ])
        );

        // Others
        assert!(matches!(run("text.lower 123"), Value::Undefined(_)));
    }

    #[test]
    fn text_upper() {

        // Text
        assert_eq!(run("text.upper \"ABC\""), Value::Text("ABC".into()));
        assert_eq!(run("text.upper \"aBc\""), Value::Text("ABC".into()));
        assert_eq!(run("text.upper \"abc\""), Value::Text("ABC".into()));

        // Nothing
        assert_eq!(run("text.upper()"), Value::Nothing);

        // Tuple
        assert_eq!(
            run("text.upper(\"abc\", \"def\")"), 
            Value::tuple(vec![
                Value::Text("ABC".into()),
                Value::Text("DEF".into()),
            ])
        );

        // Others
        assert!(matches!(run("text.upper 123"), Value::Undefined(_)));
    }

    #[test]
    fn text_trim() {

        // Text
        assert_eq!(run("text.trim \"  abc  \""), Value::Text("abc".into()));
        assert_eq!(run("text.trim \"    abc\""), Value::Text("abc".into()));
        assert_eq!(run("text.trim \"abc    \""), Value::Text("abc".into()));
        assert_eq!(run("text.trim \"abc\""), Value::Text("abc".into()));

        // Nothing
        assert_eq!(run("text.trim()"), Value::Nothing);

        // Tuple
        assert_eq!(
            run("text.trim(\"  abc  \", \"  def  \")"), 
            Value::tuple(vec![
                Value::Text("abc".into()),
                Value::Text("def".into()),
            ])
        );

        // Others
        assert!(matches!(run("text.trim 123"), Value::Undefined(_)));
    }

    #[test]
    fn text_trim_head() {

        // Text
        assert_eq!(run("text.trim_head \"  abc  \""), Value::Text("abc  ".into()));
        assert_eq!(run("text.trim_head \"    abc\""), Value::Text("abc".into()));
        assert_eq!(run("text.trim_head \"abc    \""), Value::Text("abc    ".into()));
        assert_eq!(run("text.trim_head \"abc\""), Value::Text("abc".into()));

        // Nothing
        assert_eq!(run("text.trim_head()"), Value::Nothing);

        // Tuple
        assert_eq!(
            run("text.trim_head(\"  abc  \", \"  def  \")"), 
            Value::tuple(vec![
                Value::Text("abc  ".into()),
                Value::Text("def  ".into()),
            ])
        );

        // Others
        assert!(matches!(run("text.trim_head 123"), Value::Undefined(_)));
    }

    #[test]
    fn text_trim_tail() {

        // Text
        assert_eq!(run("text.trim_tail \"  abc  \""), Value::Text("  abc".into()));
        assert_eq!(run("text.trim_tail \"    abc\""), Value::Text("    abc".into()));
        assert_eq!(run("text.trim_tail \"abc    \""), Value::Text("abc".into()));
        assert_eq!(run("text.trim_tail \"abc\""), Value::Text("abc".into()));

        // Nothing
        assert_eq!(run("text.trim_tail()"), Value::Nothing);

        // Tuple
        assert_eq!(
            run("text.trim_tail(\"  abc  \", \"  def  \")"), 
            Value::tuple(vec![
                Value::Text("  abc".into()),
                Value::Text("  def".into()),
            ])
        );

        // Others
        assert!(matches!(run("text.trim_tail 123"), Value::Undefined(_)));
    }

    #[test]
    fn text_find() {

        // Text
        assert_eq!(run(r#"text.find "bc" "abcdefabcd""#), Value::Numb(1.0));
        assert_eq!(run(r#"text.find "xyz" "abcdefabcd""#), Value::Numb(-1.0));

        // Nothing
        assert_eq!(run("text.find ()"), Value::Nothing);
        assert_eq!(run("text.find () \"abc\""), Value::Nothing);
        assert_eq!(run("text.find () ()"), Value::Nothing);

        // Tuple - Text
        assert_eq!(
            run("text.find (\"b\", \"d\") \"abcdefabcdef\""), 
            Value::tuple(vec![
                Value::Numb(1.0),
                Value::Numb(3.0),
            ])
        );

        // Tuple - Tuple
        assert_eq!(
            run("text.find (\"b\", \"d\") (\"abcdefabcdef\", \"aaaaabcccd\")"), 
            Value::tuple(vec![
                Value::Numb(1.0),
                Value::Numb(5.0),
                Value::Numb(3.0),
                Value::Numb(9.0),
            ])
        );

        // Others
        assert!(matches!(run("text.find 123 \"abc\""), Value::Undefined(_)));
        assert!(matches!(run("text.find \"abc\" 123"), Value::Undefined(_)));
    }

    #[test]
    fn text_rfind() {

        // Text - Text
        assert_eq!(run(r#"text.rfind "bc" "abcdefabcd""#), Value::Numb(7.0));
        assert_eq!(run(r#"text.rfind "xyz" "abcdefabcd""#), Value::Numb(-1.0));

        // Nothing
        assert_eq!(run("text.rfind ()"), Value::Nothing);
        assert_eq!(run("text.rfind () \"abc\""), Value::Nothing);
        assert_eq!(run("text.rfind () ()"), Value::Nothing);

        // Tuple - Text
        assert_eq!(
            run("text.rfind (\"b\", \"d\") \"abcdefabcdef\""), 
            Value::tuple(vec![
                Value::Numb(7.0),
                Value::Numb(9.0),
            ])
        );

        // Tuple - Tuple
        assert_eq!(
            run("text.rfind (\"b\", \"d\") (\"abcdefabcdef\", \"aaaaabcccd\")"), 
            Value::tuple(vec![
                Value::Numb(7.0),
                Value::Numb(5.0),
                Value::Numb(9.0),
                Value::Numb(9.0),
            ])
        );

        // Others
        assert!(matches!(run("text.rfind 123 \"abc\""), Value::Undefined(_)));
        assert!(matches!(run("text.rfind \"abc\" 123"), Value::Undefined(_)));
    }

    #[test]
    fn text_head() {

        // Numb - Text
        assert_eq!(run("text.head 3 \"abcdef\""), Value::Text("abc".into()));
        assert_eq!(run("text.head 0 \"abcdef\""), Value::Text("".into()));
        assert_eq!(run("text.head(-2) \"abcdef\""), Value::Text("abcd".into()));
        assert_eq!(run("text.head 99 \"abc\""), Value::Text("abc".into()));
        assert_eq!(run("text.head(-99) \"abc\""), Value::Text("".into()));

        // Nothing
        assert_eq!(run("text.head ()"), Value::Nothing);
        assert_eq!(run("text.head () \"abc\""), Value::Nothing);
        assert_eq!(run("text.head () ()"), Value::Nothing);

        // Tuple - Text
        assert_eq!(
            run("text.head (2, 3) \"abcdef\""), 
            Value::tuple(vec![
                Value::Text("ab".into()),
                Value::Text("abc".into()),
            ])
        );

        // Tuple - Tuple
        assert_eq!(
            run("text.head (2, 3) (\"abcd\", \"efgh\")"), 
            Value::tuple(vec![
                Value::Text("ab".into()),
                Value::Text("ef".into()),
                Value::Text("abc".into()),
                Value::Text("efg".into()),
            ])
        );

        // Others
        assert!(matches!(run("text.head 123 456"), Value::Undefined(_)));
        assert!(matches!(run("text.head \"abc\" 123"), Value::Undefined(_)));
    }

    #[test]
    fn text_tail() {

        // Numb - Text
        assert_eq!(run("text.tail 3 \"abcdef\""), Value::Text("def".into()));
        assert_eq!(run("text.tail 0 \"abcdef\""), Value::Text("abcdef".into()));
        assert_eq!(run("text.tail(-2) \"abcdef\""), Value::Text("ef".into()));
        assert_eq!(run("text.tail 99 \"abc\""), Value::Text("".into()));
        assert_eq!(run("text.tail(-99) \"abc\""), Value::Text("abc".into()));

        // Nothing
        assert_eq!(run("text.tail ()"), Value::Nothing);
        assert_eq!(run("text.tail () \"abc\""), Value::Nothing);
        assert_eq!(run("text.tail () ()"), Value::Nothing);

        // Tuple - Text
        assert_eq!(
            run("text.tail (-2, -3) \"abcdef\""), 
            Value::tuple(vec![
                Value::Text("ef".into()),
                Value::Text("def".into()),
            ])
        );

        // Tuple - Tuple
        assert_eq!(
            run("text.tail (-2, -3) (\"abcd\", \"efgh\")"), 
            Value::tuple(vec![
                Value::Text("cd".into()),
                Value::Text("gh".into()),
                Value::Text("bcd".into()),
                Value::Text("fgh".into()),
            ])
        );

        // Others
        assert!(matches!(run("text.tail 123 456"), Value::Undefined(_)));
        assert!(matches!(run("text.tail \"abc\" 123"), Value::Undefined(_)));
    }

    #[test]
    fn text_split() {

        // Text - Text
        assert_eq!(
            run("text.split \":\" \"aa:bb:cc\""),
            Value::tuple(vec![
                Value::Text("aa".into()),
                Value::Text("bb".into()),
                Value::Text("cc".into()),
            ])
        );
        assert_eq!(
            run("text.split \":\" \"aabbcc\""),
            Value::Text("aabbcc".into()),
        );

        // Nothing
        assert_eq!(run("text.split ()"), Value::Nothing);
        assert_eq!(run("text.split () \"abc\""), Value::Nothing);
        assert_eq!(run("text.split () ()"), Value::Nothing);

        // Tuple - Text
        assert_eq!(
            run("text.split (\".\", \":\") \"aa.:bb.:cc\""), 
            Value::tuple(vec![
                Value::Text("aa".into()),
                Value::Text(":bb".into()),
                Value::Text(":cc".into()),
                Value::Text("aa.".into()),
                Value::Text("bb.".into()),
                Value::Text("cc".into()),
            ])
        );

        // Tuple - Tuple
        assert_eq!(
            run("text.split (\".\", \":\") (\"aa.:bb.:cc\", \"abc:def\")"), 
            Value::tuple(vec![
                Value::Text("aa".into()),
                Value::Text(":bb".into()),
                Value::Text(":cc".into()),
                Value::Text("abc:def".into()),
                Value::Text("aa.".into()),
                Value::Text("bb.".into()),
                Value::Text("cc".into()),
                Value::Text("abc".into()),
                Value::Text("def".into()),
            ])
        );

        // Others
        assert!(matches!(run("text.split 123 456"), Value::Undefined(_)));
        assert!(matches!(run("text.split \"abc\" 123"), Value::Undefined(_)));
    }

    #[test]
    fn text_join() {

        // Text - Tuple
        assert_eq!(run(r#"text.join ":" ("aa", "bb", "cc")"#), Value::Text("aa:bb:cc".into()));
        assert_eq!(run(r#"text.join ":" "abc""#), Value::Text("abc".into()));

        // Nothing
        assert_eq!(run("text.join ()"), Value::Nothing);
        assert_eq!(run("text.join () \"abc\""), Value::Nothing);
        assert_eq!(run("text.join () ()"), Value::Nothing);

        // Tuple - Tuple
        assert_eq!(
            run(r#"text.join (".", ":") ("aa", "bb", "cc")"#), 
            Value::tuple(vec![
                Value::Text("aa.bb.cc".into()),
                Value::Text("aa:bb:cc".into()),
            ])
        );

        // Others
        assert!(matches!(run("text.join 123 345"), Value::Undefined(_)));
        assert!(matches!(run("text.join \"abc\" 123"), Value::Undefined(_)));
        assert!(matches!(run(r#"text.join ":" ("aa", 1, "cc")"#), Value::Undefined(_)));
    }
}

