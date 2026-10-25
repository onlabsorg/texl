//! Glue between Swan promises and the host's async runtime.
//!
//! The engine runs imports as background tasks. It does not depend on any
//! runtime: the host passes a [`SpawnFn`] that starts a [`Task`] on its own
//! executor. Without one, tasks go onto a thread-local queue that the host
//! drains with [`run_pending`].
//!
//! [`promise_async`] is also useful to hosts: it turns a Rust future into a
//! Swan promise, so a host function can do asynchronous work.
//!
//! # Examples
//!
//! A host function that returns a promise:
//!
//! ```
//! use swan_core::value::{BuiltinFn, Value};
//! use texl_core::{Engine, helpers::promise_async};
//!
//! let engine = Engine::new(None);
//! engine.add_builtin("fetch_answer", Value::Builtin(BuiltinFn::new("fetch_answer", |_| {
//!     promise_async(None, async { Value::Numb(42.0) })
//! })));
//! let Value::Namespace(doc) = block_on(engine.render("/main.texl", "<% fetch_answer() %>")) else { panic!() };
//! assert_eq!(doc.get("rendering"), Value::Text("42".into()));
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

use std::cell::RefCell;
use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Waker};

use swan_core::value::{Promise, Value};

/// A background task, as handed to a spawner. It is not `Send`: tasks run on the thread that created them.
pub type Task = Pin<Box<dyn Future<Output = ()>>>;

/// A host-provided function that runs a background task to completion.
pub type SpawnFn = Rc<dyn Fn(Task)>;

/// Builds a [`SpawnFn`] from a plain closure.
pub fn spawner_fn(f: impl Fn(Task) + 'static) -> SpawnFn {
    Rc::new(f)
}

thread_local! {
    /// Queue of tasks spawned via [`spawn_default`], awaiting [`run_pending`].
    static LOCAL_TASKS: RefCell<VecDeque<Task>> = RefCell::new(VecDeque::new());
}

/// Fallback spawner: queues the task on a thread-local queue.
///
/// The task does not run until [`run_pending`] is called.
pub fn spawn_default(task: Task) {
    LOCAL_TASKS.with(|q| q.borrow_mut().push_back(task));
}

/// Polls queued fallback tasks until no further progress is possible.
///
/// Uses a no-op waker, so it cannot be woken by external events. It stops when a full
/// pass completes no task and spawns no new one.
pub fn run_pending() {
    let mut cx = Context::from_waker(Waker::noop());
    loop {
        let batch: VecDeque<Task> = LOCAL_TASKS.with(|q| std::mem::take(&mut *q.borrow_mut()));
        if batch.is_empty() {
            break;
        }
        let mut progressed = false;
        let mut still_pending = 0;
        for mut task in batch {
            if task.as_mut().poll(&mut cx).is_ready() {
                progressed = true;
            } else {
                still_pending += 1;
                LOCAL_TASKS.with(|q| q.borrow_mut().push_back(task));
            }
        }
        let queued = LOCAL_TASKS.with(|q| q.borrow().len());
        if !progressed && queued == still_pending {
            break;
        }
    }
}

/// Runs `task` with `spawner`, or queues it with [`spawn_default`] if `spawner` is `None`.
pub fn spawn(spawner: Option<&SpawnFn>, task: Task) {
    match spawner {
        Some(s) => s(task),
        None => spawn_default(task),
    }
}

/// Wraps an async operation in a background task that resolves a Swan [`Promise`].
///
/// Executes `fut` inside a background task spawned via [`spawn`]. Returns immediately
/// with a [`Value::Promise`]. Once `fut` completes, the returned promise resolves to its value.
pub fn promise_async<F>(spawner: Option<&SpawnFn>, fut: F) -> Value
where
    F: Future<Output = Value> + 'static,
{
    let promise = Promise::new();
    let promise_val = Value::Promise(promise.clone());

    let task_promise = promise;
    spawn(
        spawner,
        Box::pin(async move {
            let result = fut.await;
            task_promise.resolve(result);
        }),
    );

    promise_val
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn test_promise_async_resolves_value() {
        let val = promise_async(None, async { Value::Numb(42.0) });

        assert!(matches!(val, Value::Promise(_)));

        if let Value::Promise(p) = val {
            let slot: Rc<RefCell<Option<Value>>> = Rc::new(RefCell::new(None));
            let slot_clone = slot.clone();
            p.then(move |v| {
                *slot_clone.borrow_mut() = Some(v);
                Value::Nothing
            });

            run_pending();
            assert_eq!(*slot.borrow(), Some(Value::Numb(42.0)));
        }
    }
}
