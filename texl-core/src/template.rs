//! Template rendering for Texl.
//!
//! A template is plain text containing Swan expressions delimited by `<%` and `%>`.
//! [`render`] replaces each `<% expr %>` tag with the text form of the value that `expr`
//! evaluates to, and copies all other text through unchanged.
//!
//! # Syntax
//!
//! - A tag starts at `<%` and ends at the *first* following `%>`. Tags cannot be nested,
//!   and the sequence `%>` cannot appear inside an expression.
//! - If a `<%` has no matching `%>`, rendering stops at that point and the rest of the
//!   template, including the stray `<%`, is copied through literally.
//!
//! # Evaluation model
//!
//! - Tags are evaluated strictly in order, all against the same [`Scope`]. Assignments made
//!   in one tag are therefore visible in later tags
//!   (e.g. `<% count = 1 %>Count is <% count %>`).
//! - If an expression yields a [`Value::Promise`], rendering suspends until the promise
//!   resolves (repeating if the result is itself a promise) and then renders the resolved
//!   value. This is why [`render`] is `async`.
//! - The future is single-threaded: it holds `Rc` values and is not `Send`.
//!
//! # Output format
//!
//! | Value                  | Output                                              |
//! |------------------------|-----------------------------------------------------|
//! | `Text`                 | the text itself                                     |
//! | `Numb`                 | Rust `f64` display (`30`, not `30.0`; `1.5`)        |
//! | `Bool`                 | `TRUE` / `FALSE`                                    |
//! | `Nothing`              | empty text                                          |
//! | `Tuple`                | the items, rendered and joined with `", "`          |
//! | `List`                 | `[[List of N]]`                                     |
//! | `Namespace` with `__text__` | what `__text__` returns, applied to the namespace (`[[Undefined]]` if not a text) |
//! | `Namespace`            | `[[Namespace of N]]` (N = number of members) |
//! | `Func`                 | `[[Func]]`                                          |
//! | `Undefined`            | `[[Undefined]]`                                     |
//! | `Promise` (resolved)   | the rendered resolved value                         |
//! | `Promise` (pending)    | `[[Pending Promise]]`                               |
//!
//! Promises returned by a tag are awaited, so `[[Pending Promise]]` only
//! appears for promises nested inside other values, such as tuple items.
//!
//! # Example
//!
//! ```
//! use swan_core::builtins::root_scope;
//! use swan_core::value::Value;
//! use texl_core::template::render;
//!
//! let scope = root_scope();
//! scope.set("name", Value::Text("World".into()));
//! let out = block_on(render("Hello <% name %>!", &scope));
//! assert_eq!(out.text, "Hello World!");
//! assert_eq!((out.tags[0].start, out.tags[0].end), (6, 16));
//! assert_eq!((out.tags[0].out_start, out.tags[0].out_end), (6, 11));
//! # use std::{future::Future, pin::pin, task::{Context, Poll, Waker}};
//! # fn block_on<F: Future>(fut: F) -> F::Output {
//! #     let mut cx = Context::from_waker(Waker::noop());
//! #     let mut fut = pin!(fut);
//! #     loop {
//! #         texl_core::helpers::run_pending();
//! #         if let Poll::Ready(v) = fut.as_mut().poll(&mut cx) { return v; }
//! #     }
//! # }
//! ```

use swan_core::builtins::to_text;
use swan_core::evaluator::eval;
use swan_core::lexer::Lexer;
use swan_core::parser::Parser;
use swan_core::expression::{self, Expression};
use swan_core::value::{Namespace, Scope, Value};
use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};
use crate::document::{Document, TagOrigin};



/// State shared between a [`WaitForValue`] future and the promise callback that feeds it.
///
/// - `value`: set by the callback when the promise resolves; taken by the future on `poll`.
/// - `waker`: the most recent waker registered by `poll`; used by the callback to wake the task.
struct Shared {
    value: Option<Value>,
    waker: Option<Waker>,
}

/// Future that completes once the promise callback has stored a value in the shared state.
///
/// Each `poll` either takes the stored value (`Ready`) or registers the current waker
/// and returns `Pending`.
struct WaitForValue(Rc<RefCell<Shared>>);

impl Future for WaitForValue {
    type Output = Value;

    /// Returns `Ready` with the value if the callback has already stored one;
    /// otherwise stores a clone of `cx`'s waker and returns `Pending`.
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Value> {
        let mut shared = self.0.borrow_mut();
        if let Some(v) = shared.value.take() {
            Poll::Ready(v)
        } else {
            shared.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}



/// Waits for `val` to become a non-promise value and returns it.
///
/// If `val` is a [`Value::Promise`], a callback is registered with `Promise::then`
/// (which runs immediately if the promise is already resolved) that stores the resolved
/// value and wakes the awaiting task. This repeats while the resolved value is itself a
/// promise. Any non-promise value is returned unchanged without suspending.
///
/// Promises never reject in Swan, so this function cannot fail.
async fn resolve_promises(mut val: Value) -> Value {
    while let Value::Promise(p) = &val {
        let p = p.clone();
        let shared = Rc::new(RefCell::new(Shared { value: None, waker: None }));
        let cb_shared = shared.clone();

        // `then` runs the callback immediately if the promise is already resolved.
        p.then(move |v| {
            let waker = {
                let mut s = cb_shared.borrow_mut();
                s.value = Some(v);
                s.waker.take()
            }; // borrow released before waking
            if let Some(w) = waker {
                w.wake();
            }
            Value::Nothing
        });

        let resolved = WaitForValue(shared).await;
        val = resolved;
    }
    val
}



/// One tag of a rendered template.
///
/// Positions are byte offsets: `start` and `end` in the template,
/// `out_start` and `out_end` in the rendered text.
#[derive(Debug, Clone, PartialEq)]
pub struct Tag {
    /// Where the tag starts in the template (at `<%`).
    pub start: usize,
    /// Where the tag ends in the template (after `%>`).
    pub end: usize,
    /// Where the text of the tag starts in the rendered text.
    pub out_start: usize,
    /// Where the text of the tag ends in the rendered text.
    pub out_end: usize,
    /// The value of the tag, the one that was rendered: if the expression
    /// returned a promise, its resolved value. Promises nested in the value
    /// (for example tuple items) are not awaited, as in the rendered text.
    pub value: Value,
}

impl Tag {
    /// The tag as a Swan namespace `{start, end, out_start, out_end, value}`.
    pub fn to_namespace(&self) -> Namespace {
        [
            ("start", Value::Numb(self.start as f64)),
            ("end", Value::Numb(self.end as f64)),
            ("out_start", Value::Numb(self.out_start as f64)),
            ("out_end", Value::Numb(self.out_end as f64)),
            ("value", self.value.clone()),
        ].into_iter().collect()
    }
}



/// The result of [`render`].
#[derive(Debug, Clone)]
pub struct Rendering {
    /// The template that was rendered. Undefined values made by its tags
    /// refer to it through their [`TagOrigin`] (see [`crate::document`]).
    pub document: Rc<Document>,
    /// The rendered text.
    pub text: String,
    /// The bindings of the scope when the rendering ended: a snapshot,
    /// [`Scope::namespace`]. It holds the names bound by the template and any
    /// names the scope had before.
    pub data: Namespace,
    /// The tags of the template, in source order, including the tags that
    /// render nothing (such as assignments).
    pub tags: Vec<Tag>,
}

impl Rendering {
    /// The rendering as a Swan namespace `{text, data, tags}`, with tag
    /// positions converted by `convert(text, byte_offset)` (see
    /// [`crate::engine::convert_tag_positions`]); pass `|_, b| b` to keep
    /// byte offsets.
    pub fn to_namespace(&self, convert: impl Fn(&str, usize) -> usize) -> Namespace {
        let tags = self.tags.iter().map(|tag| {
            let tag = Tag {
                start: convert(&self.document.text, tag.start),
                end: convert(&self.document.text, tag.end),
                out_start: convert(&self.text, tag.out_start),
                out_end: convert(&self.text, tag.out_end),
                value: tag.value.clone(),
            };
            Value::Namespace(tag.to_namespace())
        }).collect();
        [
            ("text", Value::Text(self.text.as_str().into())),
            ("data", Value::Namespace(self.data.clone())),
            ("tags", Value::list(tags)),
        ].into_iter().collect()
    }
}



/// Renders `document`, evaluating every `<% ... %>` expression against `scope`.
///
/// `document` is the template text (`&str` or `String`, with no URI) or a
/// [`Document`] (with the URI of the document, if it has one).
///
/// For each tag, the expression text is lexed, parsed and evaluated with
/// [`swan_core::evaluator::eval`], as a Swan [`Expression`] whose origin is
/// a [`TagOrigin`]: positions in undefined values are byte offsets in the
/// expression, and [`crate::document::locate`] turns them into offsets in
/// the template. If the result is a promise, it is awaited. The final value is converted to
/// text (see the [module documentation](self)) and appended to the output.
/// Text outside tags is copied verbatim.
///
/// Evaluation mutates `scope`, so assignments persist across tags and after the call returns.
/// An unclosed `<%` ends processing, and the remaining text is appended literally.
///
/// Returns the [`Rendering`]: the text, the data (a snapshot of the scope
/// bindings when the rendering ends) and the tags.
pub async fn render(document: impl Into<Document>, scope: &Scope) -> Rendering {
    let document = Rc::new(document.into());
    let template: &str = &document.text;
    let mut text = String::with_capacity(template.len());
    let mut tags = Vec::new();
    let mut cursor = 0;

    while let Some(start_idx) = template[cursor..].find("<%") {
        let tag_start = cursor + start_idx;
        let expr_start = tag_start + 2; // skip "<%"

        if let Some(end_idx) = template[expr_start..].find("%>") {
            let expr_end = expr_start + end_idx;
            let tag_end = expr_end + 2; // skip "%>"

            // Append literal text prior to the opening tag
            text.push_str(&template[cursor..tag_start]);

            // Parse and evaluate the expression; its origin locates it in the template
            let expr_src = &template[expr_start..expr_end];
            let origin = TagOrigin { document: document.clone(), tag_start, expr_start };
            let expr = Rc::new(Expression::new(expr_src, Some(Rc::new(origin))));
            let ast = Parser::new(Lexer::new(expr_src)).parse();
            let value = expression::with_expression(Some(expr), || eval(&ast, scope));

            // If the expression yielded a promise, wait for it to resolve
            let value = resolve_promises(value).await;

            // Convert evaluated Value into its text representation
            let out_start = text.len();
            text.push_str(&value_to_string(value.clone()));
            tags.push(Tag { start: tag_start, end: tag_end, out_start, out_end: text.len(), value });

            // Move cursor past the closing delimiter tag
            cursor = tag_end;
        } else {
            // Unclosed `<%` delimiter encountered; append remaining text and finish
            break;
        }
    }

    // Append any trailing string fragment after the final tag
    text.push_str(&template[cursor..]);
    Rendering { text, data: scope.namespace(), tags, document }
}



/// Converts a Swan [`Value`] into its rendered text form.
///
/// Single values go through [`swan_core::builtins::to_text`], the conversion
/// shared with Swan's `text.from` (including the `__text__` member of
/// namespaces); a value it cannot convert renders as `[[Undefined]]`. This
/// function adds only what is specific to templates:
///
/// | Value                  | Output                                              |
/// |------------------------|-----------------------------------------------------|
/// | `Nothing`              | empty string                                        |
/// | `Tuple`                | items rendered and joined with `", "`               |
/// | `Promise` (resolved)   | the rendered resolved value                         |
/// | `Promise` (pending)    | `[[Pending Promise]]`                               |
///
/// Top-level promises are already awaited by [`render`], so `[[Pending Promise]]` can
/// only appear for promises nested inside other values, such as tuple items.
fn value_to_string(val: Value) -> String {
    match val {
        Value::Nothing => String::new(),
        Value::Tuple(items) => {
            let item_strs: Vec<String> = items
                .iter()
                .cloned()
                .map(value_to_string)
                .collect();
            item_strs.join(", ")
        }

        // Non-blocking: read the promise's current value without awaiting it.
        Value::Promise(promise) => {
            let inner_value = promise.0.borrow().value.clone();
            match inner_value {
                Some(resolved_val) => value_to_string(resolved_val),
                None => "[[Pending Promise]]".to_string(),
            }
        }

        other => match to_text(other) {
            Value::Text(s) => s.to_string(),
            _ => "[[Undefined]]".to_string(),
        },
    }
}



#[cfg(test)]
/// Unit tests for template rendering, including promise handling.
mod tests {
    use super::*;
    use swan_core::value::{Scope, Value};
    use swan_core::value::Promise;
    use std::pin::pin;
    use std::sync::Arc;
    use std::task::Wake;

    /// Builds a child scope containing `name` ("World"), `a` (10), `b` (20),
    /// `is_valid` (TRUE) and `items` (a two-element list).
    fn create_test_scope() -> Scope {
        let scope = Scope::new();
        scope.set("name", Value::Text("World".into()));
        scope.set("a", Value::Numb(10.0));
        scope.set("b", Value::Numb(20.0));
        scope.set("is_valid", Value::Bool(true));
        scope.set("items", Value::list(vec![Value::Numb(1.0), Value::Numb(2.0)]));
        scope.new_child()
    }

    /// Waker that does nothing; sufficient because `block_on` simply re-polls in a loop.
    struct NoopWake;
    impl Wake for NoopWake {
        fn wake(self: Arc<Self>) {}
    }

    /// Minimal busy-polling executor that drives `fut` to completion on the current thread.
    fn block_on<F: Future>(fut: F) -> F::Output {
        let waker = Waker::from(Arc::new(NoopWake));
        let mut cx = Context::from_waker(&waker);
        let mut fut = pin!(fut);
        loop {
            if let Poll::Ready(v) = fut.as_mut().poll(&mut cx) {
                return v;
            }
        }
    }

    #[test]
    /// Text without tags is returned unchanged.
    fn test_plain_text_without_expressions() {
        let scope = create_test_scope();
        let input = "Hello, world! No template tags here.";
        assert_eq!(block_on(render(input, &scope)).text, "Hello, world! No template tags here.");
    }

    #[test]
    /// A single tag is replaced by its value.
    fn test_single_expression_interpolation() {
        let scope = create_test_scope();
        let input = "Hello <% name %>!";
        assert_eq!(block_on(render(input, &scope)).text, "Hello World!");
    }

    #[test]
    /// Multiple tags are evaluated, including arithmetic.
    fn test_multiple_expressions_and_arithmetic() {
        let scope = create_test_scope();
        let input = "Sum: <% a + b %>, Product: <% a * b %>";
        assert_eq!(block_on(render(input, &scope)).text, "Sum: 30, Product: 200");
    }

    #[test]
    /// Booleans render as TRUE/FALSE and Nothing as an empty string.
    fn test_boolean_and_nothing_formatting() {
        let scope = create_test_scope();
        let input = "Valid: <% is_valid %>, Empty: <% () %>";
        assert_eq!(block_on(render(input, &scope)).text, "Valid: TRUE, Empty: ");
    }

    #[test]
    /// Lists and namespaces render as placeholder summaries.
    fn test_structured_types_formatting() {
        let scope = create_test_scope();
        let input = "List: <% items %>, Namespace: <% { x: 1, y: 2 } %>";
        assert_eq!(block_on(render(input, &scope)).text, "List: [[List of 2]], Namespace: [[Namespace of 2]]");
    }

    #[test]
    /// A namespace with a `__text__` member renders as the text it returns
    /// when applied to the namespace, and stays usable as data.
    fn test_namespace_text_hook() {
        let scope = swan_core::builtins::root_scope();
        let input = "<% t: {v: 3, __text__: s -> \"<b>\" + text.from(s.v) + \"</b>\"} %> <% t.v + 1 %>";
        assert_eq!(block_on(render(input, &scope)).text, "<b>3</b> 4");

        // Nested in a tuple
        let input = "<% u = {__text__: s -> \"U\"}, (u, 1, u) %>";
        assert_eq!(block_on(render(input, &scope)).text, "U, 1, U");

        // A hook that does not return a text renders as undefined
        let input = "<% {__text__: s -> 1} %>|<% {__text__: \"x\"} %>|<% {__text__: s -> (\"a\", \"b\")} %>";
        assert_eq!(block_on(render(input, &scope)).text, "[[Undefined]]|[[Undefined]]|[[Undefined]]");
    }

    #[test]
    /// Tuple items are joined with ", ".
    fn test_tuple_formatting() {
        let scope = create_test_scope();
        let input = "Tuple: <% (a, b, name) %>";
        assert_eq!(block_on(render(input, &scope)).text, "Tuple: 10, 20, World");
    }

    #[test]
    /// An unclosed "<%" causes the remaining text to be copied literally.
    fn test_unclosed_delimiter_tag() {
        let scope = create_test_scope();
        let input = "Hello <% name %, remaining text without closing tag <% a";
        assert_eq!(
            block_on(render(input, &scope)).text,
            "Hello <% name %, remaining text without closing tag <% a"
        );
    }

    #[test]
    /// Assignments in one tag are visible in later tags.
    fn test_state_mutation_across_tags() {
        let scope = create_test_scope();
        let input = "<% count = 1 %>Count is <% count %>, next is <% count = count + 1, count %>";
        assert_eq!(block_on(render(input, &scope)).text, "Count is 1, next is 2");
    }

    #[test]
    /// An already-resolved promise renders its value without suspending.
    fn test_already_resolved_promise_is_rendered() {
        let scope = create_test_scope();
        let p = Promise::new();
        p.resolve(Value::Text("done".into()));
        scope.set("p", Value::Promise(p));
        assert_eq!(block_on(render("Result: <% p %>", &scope)).text, "Result: done");
    }

    #[test]
    /// A pending promise suspends rendering until it resolves, then renders its value.
    fn test_pending_promise_is_awaited_then_rendered() {
        let scope = create_test_scope();
        let p = Promise::new();
        scope.set("p", Value::Promise(p.clone()));

        let waker = Waker::from(Arc::new(NoopWake));
        let mut cx = Context::from_waker(&waker);
        let mut fut = pin!(render("Value: <% p %>!", &scope));

        // 1. Poll the future once. It should be pending and register the waker.
        assert!(fut.as_mut().poll(&mut cx).is_pending());

        // 2. Resolve the promise. This should wake the registered waker.
        p.resolve(Value::Numb(42.0));

        // 3. Poll again until it's ready. The `block_on` utility is perfect for this.
        let result = block_on(fut).text;
        assert_eq!(result, "Value: 42!");
    }

    #[test]
    /// A stray token makes the whole tag undefined; nothing after it runs.
    fn test_stray_token_in_tag() {
        let scope = create_test_scope();
        let input = "<% x = 1 ) y = 2 %>|<% x %>|<% 2 + 1e3 ) 1 %>";
        assert_eq!(block_on(render(input, &scope)).text, "[[Undefined]]||[[Undefined]]");
    }

    #[test]
    /// An error that reaches a tag through a promise is shown, not dropped.
    fn test_undefined_through_a_promise() {
        let scope = create_test_scope();
        let rendering = block_on(render(r#"<% 1 ~> (x -> x - "a") %>|<% (1 - "a") ~> (x -> x) %>"#, &scope));
        assert_eq!(rendering.text, "[[Undefined]]|[[Undefined]]");
        let Value::Undefined(u) = &rendering.tags[0].value else { panic!() };
        assert_eq!(u.describe(), "cannot subtract the text `\"a\"` from the number `1`");
    }
}
