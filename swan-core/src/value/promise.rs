//! Single-assignment promises for Swan.
//!
//! A [`Promise`] is a shared, mutable cell that is either *pending* or
//! *resolved* with a [`Value`]. Callbacks registered with [`Promise::then`]
//! (or [`Promise::then_apply`]) run when the promise resolves and produce a
//! new child promise holding the callback's result, so promises form a chain.
//!
//! Semantics:
//! - **Resolve once.** The first call to [`Promise::resolve`] wins; later calls
//!   are ignored.
//! - **Never rejects.** There is no failure state. Resolving with
//!   `Value::Undefined(_)` stores `Value::Nothing` instead.
//! - **Synchronous dispatch.** Callbacks run immediately on the caller's stack,
//!   either inside `resolve` (if registered while pending) or inside `then`
//!   (if the promise is already resolved). There is no event loop or scheduler
//!   in this module.
//! - **Shared identity.** Cloning a `Promise` clones the `Rc` handle, not the
//!   state; all clones observe the same resolution.
//! - **No flattening.** If a callback returns a `Value::Promise`, the child
//!   resolves to that promise value; it is not unwrapped.

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;
use crate::value::Value;
use crate::evaluator::eval_apply;

/// A callback queued on a pending promise.
///
/// It receives the parent's resolved value and returns the value that will
/// resolve the corresponding child promise.
#[derive(Clone)]
pub enum PromiseCallback {
    /// A Rust closure, registered with [`Promise::then`]. The cycle collector
    /// ([`crate::gc`]) cannot see the values it captures.
    Native(Rc<dyn Fn(Value) -> Value>),
    /// A Swan callable and a source position, registered with
    /// [`Promise::then_apply`]. The cycle collector traces the callable.
    Apply(Value, usize),
}

impl PromiseCallback {
    fn call(&self, v: Value) -> Value {
        match self {
            PromiseCallback::Native(f) => f(v),
            PromiseCallback::Apply(f, pos) => eval_apply(f.clone(), v, *pos),
        }
    }
}

/// The shared state of a [`Promise`], kept behind `Rc<RefCell<..>>`.
#[derive(Clone)]
pub struct PromiseData {
    /// `None` while pending; `Some(v)` once resolved. Set at most once and
    /// never `Value::Undefined`.
    pub value: Option<Value>,

    /// Callbacks waiting for resolution, each paired with the child
    /// `Value::Promise` to resolve with that callback's result. Always empty
    /// once the promise is resolved (drained by [`Promise::resolve`]).
    pub children: Vec<(PromiseCallback, Value)>,
}

/// Prints the resolved value (if any) and only the *number* of queued
/// children, since callbacks are opaque closures and cannot be displayed.
impl fmt::Debug for PromiseData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PromiseData")
            .field("value", &self.value)
            .field("children_count", &self.children.len())
            .finish()
    }
}

/// A single-assignment value that may not be available yet.
///
/// A promise is *pending* until [`Promise::resolve`] gives it a value, and
/// *resolved* afterwards. Callbacks registered with [`Promise::then`] run when
/// it resolves.
///
/// - **Resolve once.** The first call to `resolve` wins; later calls are
///   ignored.
/// - **Never rejects.** There is no failure state. Resolving with
///   [`Value::Undefined`] stores [`Value::Nothing`] instead.
/// - **Synchronous dispatch.** Callbacks run on the caller's stack, inside
///   `resolve` (if registered while pending) or inside `then` (if already
///   resolved). There is no scheduler in this crate.
/// - **Shared identity.** `Promise` is a cheap handle: clones observe the same
///   resolution.
/// - **No flattening.** If a callback returns a `Value::Promise`, the child
///   promise resolves to that promise; it is not unwrapped.
///
/// The inner `Rc<RefCell<PromiseData>>` is public so that hosts can inspect
/// the state; prefer `then` and `resolve` to change it.
///
/// # Examples
///
/// ```
/// use swan_core::value::{Promise, Value};
///
/// let p = Promise::new();
/// let child = p.then(|v| match v {
///     Value::Numb(n) => Value::Numb(n + 1.0),
///     other => other,
/// });
/// p.resolve(Value::Numb(41.0));
///
/// if let Value::Promise(child) = child {
///     assert_eq!(child.0.borrow().value, Some(Value::Numb(42.0)));
/// }
/// ```
#[derive(Clone, Debug)]
pub struct Promise(pub Rc<RefCell<PromiseData>>);

impl Promise {
    /// Creates a new pending promise with no value and no queued callbacks.
    pub fn new() -> Self {
        let promise = Self(Rc::new(RefCell::new(PromiseData {
            value: None,
            children: Vec::new(),
        })));
        crate::gc::track_promise(&promise);
        promise
    }

    /// Registers a Rust callback and returns a child promise (as `Value::Promise`)
    /// that resolves to the callback's result.
    ///
    /// - If `self` is already resolved, `f` is called immediately with a clone of
    ///   the value and the returned child is already resolved.
    /// - If `self` is pending, `f` is queued and runs when `self` is resolved.
    ///
    /// `f` is invoked at most once. If `f` returns `Value::Undefined`, the child
    /// resolves to `Value::Nothing` (see [`Promise::resolve`]).
    pub fn then(&self, f: impl Fn(Value) -> Value + 'static) -> Value {
        self.chain(PromiseCallback::Native(Rc::new(f)))
    }

    /// Like [`Promise::then`], but takes a Swan callable instead of a Rust closure.
    ///
    /// The callable `f` is applied to the resolved value via
    /// [`eval_apply`], with `pos` as the source position used in any resulting
    /// `Undefined` (for example, when `f` is not callable). Such an `Undefined`
    /// result resolves the child to `Value::Nothing`.
    pub fn then_apply(&self, f: Value, pos: usize) -> Value {
        self.chain(PromiseCallback::Apply(f, pos))
    }

    /// Runs `callback` now if resolved, or queues it; returns the child promise.
    fn chain(&self, callback: PromiseCallback) -> Value {
        let child = Promise::new();
        let child_val = Value::Promise(child.clone());

        let resolved = self.0.borrow().value.clone();
        match resolved {
            Some(v) => child.resolve(callback.call(v)),
            None => self.0.borrow_mut().children.push((callback, child_val.clone())),
        }

        child_val
    }

    /// Resolves this promise with `v` and runs all queued callbacks.
    ///
    /// - Only the first call has any effect; if the promise is already resolved,
    ///   `v` is ignored and nothing runs.
    /// - `Value::Undefined(_)` is replaced by `Value::Nothing`, so a promise never
    ///   holds an undefined value.
    /// - Each queued callback is called in registration order with a clone of the
    ///   resolved value, and its result resolves the associated child promise.
    ///   Resolution therefore cascades down the chain synchronously.
    /// - The internal borrow is released before callbacks run, so callbacks may
    ///   safely call `then` on this promise (they will see it already resolved).
    pub fn resolve(&self, v: Value) {
        let mut inner = self.0.borrow_mut();

        // Promises resolve only once
        if inner.value.is_some() {
            return;
        }

        // A swan promise never rejects; undefined values resolve to Nothing
        let resolve_val = if matches!(v, Value::Undefined(_)) {
            Value::Nothing
        } else {
            v
        };

        inner.value = Some(resolve_val.clone());

        // Drain children to drop mutable borrow on inner before invoking closures
        let children = std::mem::take(&mut inner.children);
        drop(inner);

        for (f, child_val) in children {
            let res = f.call(resolve_val.clone());
            if let Value::Promise(child) = child_val {
                child.resolve(res);
            }
        }
    }
}
