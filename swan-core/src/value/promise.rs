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

/// A Rust callback queued on a pending promise.
///
/// It receives the parent's resolved value and returns the value that will
/// resolve the corresponding child promise.
pub type PromiseCallback = Rc<dyn Fn(Value) -> Value>;

/// Internal state of a [`Promise`], kept behind `Rc<RefCell<..>>`.
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

/// A cheaply clonable handle to a shared promise.
///
/// See the [module documentation](self) for semantics. The inner
/// `Rc<RefCell<PromiseData>>` is public so other `value` code can inspect the
/// state; prefer [`Promise::then`] and [`Promise::resolve`] for mutation.
#[derive(Clone, Debug)]
pub struct Promise(pub Rc<RefCell<PromiseData>>);

impl Promise {
    /// Creates a new pending promise with no value and no queued callbacks.
    pub fn new() -> Self {
        Self(Rc::new(RefCell::new(PromiseData {
            value: None,
            children: Vec::new(),
        })))
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
    ///
    /// # Panics
    /// When `self` is already resolved, `f` runs while `self`'s internal `RefCell`
    /// is mutably borrowed. A callback that re-enters `self` (e.g. calls
    /// `self.then(..)` or `self.resolve(..)` on the same promise) will panic with
    /// a borrow error.
    pub fn then(&self, f: impl Fn(Value) -> Value + 'static) -> Value {
        let callback: PromiseCallback = Rc::new(f);
        let child = Promise::new();
        let child_val = Value::Promise(child.clone());

        let mut inner = self.0.borrow_mut();

        if let Some(ref v) = inner.value {
            // Parent is already resolved: compute and resolve child immediately
            let res = callback(v.clone());
            child.resolve(res);
        } else {
            // Parent is pending: queue callback and child promise
            inner.children.push((callback, child_val.clone()));
        }

        child_val
    }

    /// Like [`Promise::then`], but takes a Swan callable instead of a Rust closure.
    ///
    /// The callable `f` is applied to the resolved value via
    /// [`eval_apply`], with `pos` as the source position used in any resulting
    /// `Undefined` (for example, when `f` is not callable). Such an `Undefined`
    /// result resolves the child to `Value::Nothing`.
    pub fn then_apply(&self, f: Value, pos: usize) -> Value {
        self.then(move |v| eval_apply(f.clone(), v, pos))
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
            let res = f(resolve_val.clone());
            if let Value::Promise(child) = child_val {
                child.resolve(res);
            }
        }
    }
}
