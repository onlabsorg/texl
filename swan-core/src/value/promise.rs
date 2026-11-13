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
//! - **Never rejects.** There is no failure state. An undefined value is a
//!   value like any other: resolving with `Value::Undefined(_)` stores it.
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
use crate::expression::{self, Expression};

/// A callback queued on a pending promise.
///
/// It receives the parent's resolved value and returns the value that will
/// resolve the corresponding child promise.
#[derive(Clone)]
pub enum PromiseCallback {
    /// A Rust closure, registered with [`Promise::then`]. The cycle collector
    /// ([`crate::gc`]) cannot see the values it captures.
    Native(Rc<dyn Fn(Value) -> Value>),
    /// A Swan callable, and the source position and expression of the `~>`
    /// that registered it with [`Promise::then_apply`]. The callable runs with
    /// that expression as the current one, whenever the promise resolves. The
    /// cycle collector traces the callable.
    Apply(Value, usize, Option<Rc<Expression>>),
}

impl PromiseCallback {
    fn call(&self, v: Value) -> Value {
        match self {
            PromiseCallback::Native(f) => f(v),
            // The `~>` is where this evaluation enters Swan (see `expression::entry`)
            PromiseCallback::Apply(f, pos, expression) => expression::with_entry(expression.clone(), || {
                expression::with_expression(expression.clone(), || eval_apply(f.clone(), v, *pos))
            }),
        }
    }
}

/// The shared state of a [`Promise`], kept behind `Rc<RefCell<..>>`.
#[derive(Clone)]
pub struct PromiseData {
    /// `None` while pending; `Some(v)` once resolved. Set at most once.
    pub value: Option<Value>,

    /// The position, in an expression, of the first call that returned this
    /// promise while it was pending, set by [`Promise::place_undefined`]: the
    /// value the promise resolves to gets it with [`Value::place_undefined`].
    pub placement: Option<(usize, Option<Rc<Expression>>)>,

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
            .field("placement", &self.placement.as_ref().map(|(pos, _)| pos))
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
///   [`Value::Undefined`] stores it, as any other value.
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
            placement: None,
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
    /// `f` is invoked at most once. Whatever `f` returns resolves the child,
    /// `Value::Undefined` included (see [`Promise::resolve`]).
    pub fn then(&self, f: impl Fn(Value) -> Value + 'static) -> Value {
        self.chain(PromiseCallback::Native(Rc::new(f)))
    }

    /// Like [`Promise::then`], but takes a Swan callable instead of a Rust closure.
    ///
    /// The callable `f` is applied to the resolved value via
    /// [`eval_apply`], with `pos` (in the current expression, captured now) as
    /// the source position used in any resulting `Undefined` (for example,
    /// when `f` is not callable). Such an `Undefined` resolves the child
    /// like any other value (T9).
    pub fn then_apply(&self, f: Value, pos: usize) -> Value {
        self.chain(PromiseCallback::Apply(f, pos, expression::current()))
    }

    /// Gives the value this promise resolves to the position `pos` in
    /// `expression`, for its undefined values with no position (see
    /// [`Value::place_undefined`]), if it is pending and has no position yet.
    /// The evaluator calls it on the promises built-ins return: the first
    /// call that returns a pending promise is where its value comes from.
    /// The promise is not copied, so a built-in that returns the same
    /// promise twice still returns one promise.
    pub fn place_undefined(&self, pos: usize, expression: &Option<Rc<Expression>>) {
        let mut inner = self.0.borrow_mut();
        if inner.value.is_none() && inner.placement.is_none() {
            inner.placement = Some((pos, expression.clone()));
        }
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
    /// - Any value is stored as it is, `Value::Undefined(_)` included: an
    ///   undefined value is not a rejection.
    /// - Each queued callback is called in registration order with a clone of the
    ///   resolved value, and its result resolves the associated child promise.
    ///   Resolution therefore cascades down the chain synchronously.
    /// - The internal borrow is released before callbacks run, so callbacks may
    ///   safely call `then` on this promise (they will see it already resolved).
    pub fn resolve(&self, v: Value) {
        // The callbacks waiting for a promise are run from a list, not by
        // recursion: a callback resolves its child promise, whose callbacks
        // resolve theirs, and a long chain (`p ~> f ~> f ~> …`) would use a
        // level of stack for each link. The list is a stack, and the children
        // of a promise go on it in reverse, so the order is the one recursion
        // would give: a callback's own chain runs before the next callback.
        let mut pending: Vec<(PromiseCallback, Value, Value)> = Vec::new();
        self.settle(v, &mut pending);
        while let Some((callback, child, value)) = pending.pop() {
            let result = callback.call(value);
            if let Value::Promise(child) = child {
                child.settle(result, &mut pending);
            }
        }
    }

    /// Stores `v` as the value of this promise, if it is pending, and adds
    /// its callbacks to `pending`, each with its child promise and the value
    /// to call it with.
    fn settle(&self, v: Value, pending: &mut Vec<(PromiseCallback, Value, Value)>) {
        // Promises resolve only once
        let placement = {
            let inner = self.0.borrow();
            if inner.value.is_some() {
                return;
            }
            inner.placement.clone()
        };

        // A Swan promise never rejects, and an undefined value is not a
        // rejection: it is stored like any other value (T9). Its undefined
        // values with no position get that of the call that returned it.
        // The promise is not borrowed meanwhile: the value may be, or hold,
        // this very promise, which placing borrows.
        let resolve_val = match &placement {
            Some((pos, expression)) => v.place_undefined_in(*pos, expression),
            None => v,
        };

        let mut inner = self.0.borrow_mut();
        if inner.value.is_some() {
            return;
        }
        inner.value = Some(resolve_val.clone());

        // Drained, so that no borrow is held when the callbacks run
        let children = std::mem::take(&mut inner.children);
        drop(inner);
        for (callback, child) in children.into_iter().rev() {
            pending.push((callback, child, resolve_val.clone()));
        }
    }
}
