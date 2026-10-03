
//! The `text` built-in namespace.
//!
//! Every function is applied element-wise over tuples (via `map_1` / `map_curried`),
//! so `Nothing` maps to `Nothing` and tuples map to tuples. Arguments of the wrong
//! type produce `Value::Undefined`. All positions and lengths count Unicode
//! characters, not bytes.

use crate::value::{BuiltinFn, Scope, Value};
use super::helpers::*;


// ----------------------------------------------------------------------------
//  `text` built-in namespace 
// ----------------------------------------------------------------------------

/// Builds the `text` namespace and returns it as a `Value::Namespace`.
///
/// The namespace scope is a child of `parent`, so root-scope names stay visible inside it.
pub fn create_namespace(parent: &Scope) -> Value {
    let text_scope = parent.new_child();

    // Unary functions: map over each item of the argument.
    text_scope.set("from"     , map_1("text.from"     , text_from));
    text_scope.set("size"     , map_1("text.size"     , text_size));
    text_scope.set("enum"     , map_1("text.enum"     , text_enum));
    text_scope.set("lower"    , map_1("text.lower"    , text_lower));
    text_scope.set("upper"    , map_1("text.upper"    , text_upper));
    text_scope.set("trim"     , map_1("text.trim"     , text_trim));
    text_scope.set("trim_head", map_1("text.trim_head", text_trim_head));
    text_scope.set("trim_tail", map_1("text.trim_tail", text_trim_tail));

    // Curried binary functions: (needle/count/separator) first, then the target text.
    text_scope.set("find"     , map_curried("text.find" , text_find));
    text_scope.set("rfind"    , map_curried("text.rfind", text_rfind));
    text_scope.set("head"     , map_curried("text.head" , text_head));
    text_scope.set("tail"     , map_curried("text.tail" , text_tail));
    text_scope.set("split"    , map_curried("text.split", text_split));
    // `join` uses map_1 (not map_curried) on purpose: the separator is mapped, but the
    // returned function receives the whole tuple of items to join.
    text_scope.set("join"     , map_1("text.join"     , text_join));

    Value::Namespace(text_scope)
}

// ----------------------------------------------------------------------------
//  Conversion / inspection
// ----------------------------------------------------------------------------

/// Converts a scalar to its text form (`TRUE`/`FALSE`, number, `[[List of N]]`, `[[Func]]`, `[[Undefined]]`, `[[Namespace of N]]`). Text passes through unchanged; anything else is Undefined.
fn text_from(val: Value) -> Value {
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
        Value::List(l) => {
            format!("[[List of {}]]", l.len())
        }
        Value::Func{..} | Value::Builtin(_) => {
            "[[Func]]".to_string()
        }
        Value::Undefined(_) => {
            "[[Undefined]]".to_string()
        }
        Value::Namespace(scope) => {
            format!("[[Namespace of {}]]", scope.0.borrow().bindings.len())
        }
        // Bool/Numb/Text/etc. handled above; Nothing and Tuple never reach here (handled by map_1).
        _ => return Value::undef_unary("text.from", val, 0),
    };

    Value::Text(s.into())
}

/// Number of characters in the text.
fn text_size(val: Value) -> Value {
    match val {
        Value::Text(s) => Value::Numb(s.chars().count() as f64),
        other => Value::undef_unary("text.size", other, 0),
    }
}

/// Splits the text into a tuple of single-character texts.
fn text_enum(val: Value) -> Value {
    match val {
        Value::Text(s) => {
            let chars: Vec<Value> = s.chars().map(|c| Value::Text(c.to_string().into())).collect();
            Value::tuple(chars)
        }
        other => Value::undef_unary("text.enum", other, 0),
    }
}

// ----------------------------------------------------------------------------
//  Case and whitespace
// ----------------------------------------------------------------------------

/// Lower-cases the text.
fn text_lower(val: Value) -> Value {
    match val {
        Value::Text(s) => Value::Text(s.to_lowercase().into()),
        other => Value::undef_unary("text.lower", other, 0),
    }
}

/// Upper-cases the text.
fn text_upper(val: Value) -> Value {
    match val {
        Value::Text(s) => Value::Text(s.to_uppercase().into()),
        other => Value::undef_unary("text.upper", other, 0),
    }
}

/// Removes whitespace from both ends of the text.
fn text_trim(val: Value) -> Value {
    match val {
        Value::Text(s) => Value::Text(s.trim().into()),
        other => Value::undef_unary("text.trim", other, 0),
    }
}

/// Removes whitespace from the start of the text.
fn text_trim_head(val: Value) -> Value {
    match val {
        Value::Text(s) => Value::Text(s.trim_start().into()),
        other => Value::undef_unary("text.trim_head", other, 0),
    }
}

/// Removes whitespace from the end of the text.
fn text_trim_tail(val: Value) -> Value {
    match val {
        Value::Text(s) => Value::Text(s.trim_end().into()),
        other => Value::undef_unary("text.trim_tail", other, 0),
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
        (_, other) => Value::undef_unary("text.find", other, 0),
    }
}

/// Character index of the last occurrence of `needle` in `haystack`, or -1 if absent. Only `haystack` is reported in the Undefined result if either argument is not Text.
fn text_rfind(needle: Value, haystack: Value) -> Value {
    match (&needle, haystack) {
        (Value::Text(n), Value::Text(h)) => char_position(&h, h.rfind(n.as_ref())),
        (_, other) => Value::undef_unary("text.rfind", other, 0),
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
        (_, other) => Value::undef_unary("text.head", other, 0),
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
        (_, other) => Value::undef_unary("text.tail", other, 0),
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
        (_, other) => Value::undef_unary("text.split", other, 0),
    }
}

/// Curried: `text.join sep items` joins all Text items with `sep`. Returns Undefined if `sep` is not Text or if any item is not Text.
fn text_join(sep: Value) -> Value {
    Value::Builtin(BuiltinFn::new("text.join", move |items| {
        match &sep {
            Value::Text(s1) => {
                // Validate and collect first, so one non-Text item aborts the whole join.
                let mut strings: Vec<String> = Vec::new();
                for item in items.items() {
                    match item {
                        Value::Text(t) => strings.push(t.to_string()),
                        other => return Value::undef_unary("text.join", other, 0), // `return` exits the whole builtin closure, not just the loop.
                    }
                }
                Value::Text(strings.join(s1.as_ref()).into())
            }
            _ => Value::undef_unary("text.join", items, 0),
        }
    }))
}


// ----------------------------------------------------------------------------
//  UNIT TESTS
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;
    use crate::evaluator::eval;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn root_scope() -> Scope {
        let scope = Scope::new();
        scope.set("text", create_namespace(&scope));
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
            Value::Tuple(Rc::new(vec![
                Value::Text("21".into()),
                Value::Text("TRUE".into()),
            ])),
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
            Value::Tuple(Rc::new(vec![
                Value::Numb(3.0),
                Value::Numb(5.0),
            ]))
        );

        // Others
        assert!(matches!(run("text.size 123"), Value::Undefined(_)));
    }

    #[test]
    fn text_enum() {

        // Text
        assert_eq!(
            run(r#"text.enum "abc""#),
            Value::Tuple(Rc::new(vec![
                Value::Text("a".into()),
                Value::Text("b".into()),
                Value::Text("c".into()),
            ]))
        );

        // Nothing
        assert_eq!(run("text.enum()"), Value::Nothing);

        // Tuple
        assert_eq!(
            run("text.enum(\"abc\", \"de\")"), 
            Value::Tuple(Rc::new(vec![
                Value::Text("a".into()),
                Value::Text("b".into()),
                Value::Text("c".into()),
                Value::Text("d".into()),
                Value::Text("e".into()),
            ]))
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
            Value::Tuple(Rc::new(vec![
                Value::Text("abc".into()),
                Value::Text("def".into()),
            ]))
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
            Value::Tuple(Rc::new(vec![
                Value::Text("ABC".into()),
                Value::Text("DEF".into()),
            ]))
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
            Value::Tuple(Rc::new(vec![
                Value::Text("abc".into()),
                Value::Text("def".into()),
            ]))
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
            Value::Tuple(Rc::new(vec![
                Value::Text("abc  ".into()),
                Value::Text("def  ".into()),
            ]))
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
            Value::Tuple(Rc::new(vec![
                Value::Text("  abc".into()),
                Value::Text("  def".into()),
            ]))
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
            Value::Tuple(Rc::new(vec![
                Value::Numb(1.0),
                Value::Numb(3.0),
            ]))
        );

        // Tuple - Tuple
        assert_eq!(
            run("text.find (\"b\", \"d\") (\"abcdefabcdef\", \"aaaaabcccd\")"), 
            Value::Tuple(Rc::new(vec![
                Value::Numb(1.0),
                Value::Numb(5.0),
                Value::Numb(3.0),
                Value::Numb(9.0),
            ]))
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
            Value::Tuple(Rc::new(vec![
                Value::Numb(7.0),
                Value::Numb(9.0),
            ]))
        );

        // Tuple - Tuple
        assert_eq!(
            run("text.rfind (\"b\", \"d\") (\"abcdefabcdef\", \"aaaaabcccd\")"), 
            Value::Tuple(Rc::new(vec![
                Value::Numb(7.0),
                Value::Numb(5.0),
                Value::Numb(9.0),
                Value::Numb(9.0),
            ]))
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
            Value::Tuple(Rc::new(vec![
                Value::Text("ab".into()),
                Value::Text("abc".into()),
            ]))
        );

        // Tuple - Tuple
        assert_eq!(
            run("text.head (2, 3) (\"abcd\", \"efgh\")"), 
            Value::Tuple(Rc::new(vec![
                Value::Text("ab".into()),
                Value::Text("ef".into()),
                Value::Text("abc".into()),
                Value::Text("efg".into()),
            ]))
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
            Value::Tuple(Rc::new(vec![
                Value::Text("ef".into()),
                Value::Text("def".into()),
            ]))
        );

        // Tuple - Tuple
        assert_eq!(
            run("text.tail (-2, -3) (\"abcd\", \"efgh\")"), 
            Value::Tuple(Rc::new(vec![
                Value::Text("cd".into()),
                Value::Text("gh".into()),
                Value::Text("bcd".into()),
                Value::Text("fgh".into()),
            ]))
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
            Value::Tuple(Rc::new(vec![
                Value::Text("aa".into()),
                Value::Text("bb".into()),
                Value::Text("cc".into()),
            ]))
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
            Value::Tuple(Rc::new(vec![
                Value::Text("aa".into()),
                Value::Text(":bb".into()),
                Value::Text(":cc".into()),
                Value::Text("aa.".into()),
                Value::Text("bb.".into()),
                Value::Text("cc".into()),
            ]))
        );

        // Tuple - Tuple
        assert_eq!(
            run("text.split (\".\", \":\") (\"aa.:bb.:cc\", \"abc:def\")"), 
            Value::Tuple(Rc::new(vec![
                Value::Text("aa".into()),
                Value::Text(":bb".into()),
                Value::Text(":cc".into()),
                Value::Text("abc:def".into()),
                Value::Text("aa.".into()),
                Value::Text("bb.".into()),
                Value::Text("cc".into()),
                Value::Text("abc".into()),
                Value::Text("def".into()),
            ]))
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
            Value::Tuple(Rc::new(vec![
                Value::Text("aa.bb.cc".into()),
                Value::Text("aa:bb:cc".into()),
            ]))
        );

        // Others
        assert!(matches!(run("text.join 123 345"), Value::Undefined(_)));
        assert!(matches!(run("text.join \"abc\" 123"), Value::Undefined(_)));
        assert!(matches!(run(r#"text.join ":" ("aa", 1, "cc")"#), Value::Undefined(_)));
    }
}

