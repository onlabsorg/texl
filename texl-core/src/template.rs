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
//! See [`value_to_string`] for how each kind of value is rendered.
//!
//! # Example
//!
//! ```ignore
//! let scope = Scope::new();
//! scope.set("name", Value::Text("World".into()));
//! let out = render("Hello <% name %>!", &scope).await;
//! assert_eq!(out, "Hello World!");
//! ```

use swan_core::evaluator::eval;
use swan_core::lexer::Lexer;
use swan_core::parser::Parser;
use swan_core::value::{Scope, Value};
use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};



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


/// Renders `template`, evaluating every `<% ... %>` expression against `scope`.
///
/// For each tag, the expression text is lexed, parsed and evaluated with
/// [`swan_core::evaluator::eval`]. If the result is a promise, it is awaited via
/// [`resolve_promises`]. The final value is converted with [`value_to_string`] and
/// appended to the output. Text outside tags is copied verbatim.
///
/// Evaluation mutates `scope`, so assignments persist across tags and after the call returns.
/// An unclosed `<%` ends processing, and the remaining text is appended literally.
///
/// # Arguments
/// * `template` - the template source text.
/// * `scope` - the scope used to evaluate all expressions.
///
/// # Returns
/// The fully interpolated string.
pub async fn render(template: &str, scope: &Scope) -> String {
    let mut result = String::with_capacity(template.len());
    let mut cursor = 0;

    while let Some(start_idx) = template[cursor..].find("<%") {
        let tag_start = cursor + start_idx;
        let expr_start = tag_start + 2; // skip "<%"

        if let Some(end_idx) = template[expr_start..].find("%>") {
            let expr_end = expr_start + end_idx;
            let tag_end = expr_end + 2; // skip "%>"

            // Append literal text prior to the opening tag
            result.push_str(&template[cursor..tag_start]);

            // Extract expression snippet between delimiters
            let expr_src = &template[expr_start..expr_end];

            // Parse and evaluate expression
            let evaluated_val = {
                let mut parser = Parser::new(Lexer::new(expr_src));
                let ast = parser.parse();
                eval(&ast, scope)
            };

            // If the expression yielded a promise, wait for it to resolve
            let evaluated_val = resolve_promises(evaluated_val).await;

            // Convert evaluated Value into its text representation
            result.push_str(&value_to_string(evaluated_val));

            // Move cursor past the closing delimiter tag
            cursor = tag_end;
        } else {
            // Unclosed `<%` delimiter encountered; append remaining text and finish
            break;
        }
    }

    // Append any trailing string fragment after the final tag
    result.push_str(&template[cursor..]);
    result
}


/// Converts a Swan [`Value`] into its rendered text form.
///
/// | Value                  | Output                                              |
/// |------------------------|-----------------------------------------------------|
/// | `Text`                 | the string itself                                   |
/// | `Numb`                 | Rust `f64` display (`30`, not `30.0`; `1.5`)        |
/// | `Bool`                 | `TRUE` / `FALSE`                                    |
/// | `Nothing`              | empty string                                        |
/// | `Tuple`                | items rendered and joined with `", "`               |
/// | `List`                 | `[[List of N]]`                                     |
/// | `Namespace`            | `[[Namespace of N]]` (N = number of local bindings) |
/// | `Func` / `Builtin`     | `[[Func]]`                                          |
/// | `Undefined`            | `[[Undefined]]`                                     |
/// | `Name`                 | the name text                                       |
/// | `Promise` (resolved)   | the rendered resolved value                         |
/// | `Promise` (pending)    | `[[Pending Promise]]`                               |
///
/// Top-level promises are already awaited by [`render`], so `[[Pending Promise]]` can
/// only appear for promises nested inside other values, such as tuple items.
fn value_to_string(val: Value) -> String {
    match val {
        Value::Text(s) => s.to_string(),
        Value::Numb(n) => n.to_string(),
        Value::Bool(b) => if b { "TRUE".into() } else { "FALSE".into() },
        Value::Nothing => String::new(),
        Value::List(l) => format!("[[List of {}]]", l.len()),
        Value::Namespace(s) => format!("[[Namespace of {}]]", s.0.borrow().bindings.len()),
        Value::Func { .. } | Value::Builtin(_) => "[[Func]]".to_string(),
        Value::Undefined(_) => "[[Undefined]]".to_string(),
        Value::Tuple(items) => {
            let item_strs: Vec<String> = items
                .iter()
                .cloned()
                .map(value_to_string)
                .collect();
            item_strs.join(", ")
        }
        Value::Name(n) => n.to_string(),

        // Non-blocking: read the promise's current value without awaiting it.
        Value::Promise(promise) => {
            let inner_value = promise.0.borrow().value.clone();
            match inner_value {
                Some(resolved_val) => value_to_string(resolved_val),
                None => "[[Pending Promise]]".to_string(),
            }
        }
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
        assert_eq!(block_on(render(input, &scope)), "Hello, world! No template tags here.");
    }

    #[test]
    /// A single tag is replaced by its value.
    fn test_single_expression_interpolation() {
        let scope = create_test_scope();
        let input = "Hello <% name %>!";
        assert_eq!(block_on(render(input, &scope)), "Hello World!");
    }

    #[test]
    /// Multiple tags are evaluated, including arithmetic.
    fn test_multiple_expressions_and_arithmetic() {
        let scope = create_test_scope();
        let input = "Sum: <% a + b %>, Product: <% a * b %>";
        assert_eq!(block_on(render(input, &scope)), "Sum: 30, Product: 200");
    }

    #[test]
    /// Booleans render as TRUE/FALSE and Nothing as an empty string.
    fn test_boolean_and_nothing_formatting() {
        let scope = create_test_scope();
        let input = "Valid: <% is_valid %>, Empty: <% () %>";
        assert_eq!(block_on(render(input, &scope)), "Valid: TRUE, Empty: ");
    }

    #[test]
    /// Lists and namespaces render as placeholder summaries.
    fn test_structured_types_formatting() {
        let scope = create_test_scope();
        let input = "List: <% items %>, Namespace: <% { x: 1, y: 2 } %>";
        assert_eq!(block_on(render(input, &scope)), "List: [[List of 2]], Namespace: [[Namespace of 2]]");
    }

    #[test]
    /// Tuple items are joined with ", ".
    fn test_tuple_formatting() {
        let scope = create_test_scope();
        let input = "Tuple: <% (a, b, name) %>";
        assert_eq!(block_on(render(input, &scope)), "Tuple: 10, 20, World");
    }

    #[test]
    /// An unclosed "<%" causes the remaining text to be copied literally.
    fn test_unclosed_delimiter_tag() {
        let scope = create_test_scope();
        let input = "Hello <% name %, remaining text without closing tag <% a";
        assert_eq!(
            block_on(render(input, &scope)),
            "Hello <% name %, remaining text without closing tag <% a"
        );
    }

    #[test]
    /// Assignments in one tag are visible in later tags.
    fn test_state_mutation_across_tags() {
        let scope = create_test_scope();
        let input = "<% count = 1 %>Count is <% count %>, next is <% count = count + 1, count %>";
        assert_eq!(block_on(render(input, &scope)), "Count is 1, next is 2");
    }

    #[test]
    /// An already-resolved promise renders its value without suspending.
    fn test_already_resolved_promise_is_rendered() {
        let scope = create_test_scope();
        let p = Promise::new();
        p.resolve(Value::Text("done".into()));
        scope.set("p", Value::Promise(p));
        assert_eq!(block_on(render("Result: <% p %>", &scope)), "Result: done");
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
        let result = block_on(fut);
        assert_eq!(result, "Value: 42!");
    }
}
