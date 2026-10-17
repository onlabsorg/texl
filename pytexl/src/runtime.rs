//! Runs the engine's futures on the asyncio event loop.
//!
//! The engine's futures are single-threaded (`!Send`), so they cannot be handed to a
//! Rust runtime on another thread. Instead, each thread has a small executor whose tasks
//! are polled by a callback on the asyncio loop running on that thread:
//!
//! - [`spawn`] stores a task and marks it ready.
//! - Marking a task ready schedules one call of [`drain`] with `loop.call_soon`, unless
//!   one is already scheduled or running.
//! - [`drain`] polls the ready tasks. A pending task is woken again by its waker, which
//!   marks it ready: when a Swan promise resolves, or when a Python awaitable completes
//!   (see [`AwaitPy`]).
//!
//! Wakers fire on the loop thread: inside a poll, or inside an asyncio done callback.
//! A task marked ready when no loop is running waits for the next call of [`kick`], which
//! every Python awaitable of this crate makes when it is awaited.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};

use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use texl_core::helpers::Task;

/// The ids of the tasks to poll. It is shared with the wakers, which must be `Send`.
#[derive(Default)]
struct ReadyQueue {
    ids: Mutex<VecDeque<u64>>,
    /// True while a call of `drain` is scheduled on the loop.
    scheduled: AtomicBool,
}

impl ReadyQueue {
    fn push(&self, id: u64) {
        self.ids.lock().unwrap().push_back(id);
    }

    fn take(&self) -> VecDeque<u64> {
        std::mem::take(&mut *self.ids.lock().unwrap())
    }

    fn is_empty(&self) -> bool {
        self.ids.lock().unwrap().is_empty()
    }
}

/// The tasks of the current thread.
#[derive(Default)]
struct Executor {
    tasks: RefCell<HashMap<u64, Task>>,
    next_id: Cell<u64>,
    ready: Arc<ReadyQueue>,
    /// True while `drain` runs: tasks woken meanwhile are polled by the same call.
    draining: Cell<bool>,
}

thread_local! {
    static EXECUTOR: Executor = Executor::default();
}

/// Wakes a task by marking it ready.
struct TaskWaker {
    id: u64,
    ready: Arc<ReadyQueue>,
}

impl Wake for TaskWaker {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.ready.push(self.id);
        schedule(&self.ready);
    }
}

/// Starts a background task on the current thread. It is first polled on the asyncio loop.
pub fn spawn(task: Task) {
    let ready = EXECUTOR.with(|ex| {
        let id = ex.next_id.get();
        ex.next_id.set(id + 1);
        ex.tasks.borrow_mut().insert(id, task);
        ex.ready.push(id);
        ex.ready.clone()
    });
    schedule(&ready);
}

/// Starts `fut` as a background task and returns a Swan promise of its result.
pub fn promise_of(fut: impl Future<Output = swan_core::value::Value> + 'static) -> swan_core::value::Promise {
    let promise = swan_core::value::Promise::new();
    let p = promise.clone();
    spawn(Box::pin(async move { p.resolve(fut.await) }));
    promise
}

/// Schedules a call of [`drain`] if tasks are ready and none is scheduled. Called when a
/// Python awaitable of this crate is awaited, which happens inside a running loop.
pub fn kick() {
    let ready = EXECUTOR.with(|ex| ex.ready.clone());
    if !ready.is_empty() {
        schedule(&ready);
    }
}

/// Schedules a call of [`drain`] on the running loop of the current thread.
fn schedule(ready: &ReadyQueue) {
    // A running drain polls the tasks woken meanwhile
    if EXECUTOR.with(|ex| ex.draining.get()) || ready.scheduled.swap(true, Ordering::AcqRel) {
        return;
    }
    let scheduled = Python::attach(|py| -> PyResult<()> {
        let event_loop = asyncio(py)?.call_method0("get_running_loop")?;
        event_loop.call_method1("call_soon", (wrap_pyfunction!(drain, py)?,))?;
        Ok(())
    });
    if scheduled.is_err() {
        // No running loop: the tasks wait for the next `kick`
        ready.scheduled.store(false, Ordering::Release);
    }
}

/// Polls the ready tasks, until none is left. Called by the asyncio loop.
#[pyfunction]
fn drain() {
    EXECUTOR.with(|ex| {
        ex.ready.scheduled.store(false, Ordering::Release);
        ex.draining.set(true);
        loop {
            let ids = ex.ready.take();
            if ids.is_empty() {
                break;
            }
            for id in ids {
                // The task is out of the map while it runs, so it can spawn other tasks
                let Some(mut task) = ex.tasks.borrow_mut().remove(&id) else { continue };
                let waker = Waker::from(Arc::new(TaskWaker { id, ready: ex.ready.clone() }));
                if task.as_mut().poll(&mut Context::from_waker(&waker)).is_pending() {
                    ex.tasks.borrow_mut().insert(id, task);
                }
            }
        }
        ex.draining.set(false);
    });
}

/// The `asyncio` module.
pub fn asyncio(py: Python<'_>) -> PyResult<&Bound<'_, PyAny>> {
    static ASYNCIO: PyOnceLock<Py<PyAny>> = PyOnceLock::new();
    ASYNCIO
        .get_or_try_init(py, || py.import("asyncio").map(|m| m.into_any().unbind()))
        .map(|m| m.bind(py))
}

/// The state shared by an [`AwaitPy`] future and the done callback of its asyncio future.
enum AwaitState {
    /// Not polled yet: holds the awaitable.
    Start(Py<PyAny>),
    /// Waiting for the asyncio future, which is kept alive here: the loop only holds weak
    /// references to its tasks.
    Waiting { _future: Py<PyAny>, waker: Waker },
    /// Completed: the result, or `None` if the awaitable raised or was cancelled.
    Done(Option<Py<PyAny>>),
}

/// A Rust future that awaits a Python awaitable (a coroutine, a future or a task).
///
/// The first poll wraps the awaitable with `asyncio.ensure_future`, so it must happen on
/// the loop: tasks spawned with [`spawn`] are only polled there.
pub struct AwaitPy {
    state: Rc<RefCell<AwaitState>>,
}

impl AwaitPy {
    /// Wraps `awaitable`. Nothing happens until the future is polled.
    pub fn new(awaitable: Py<PyAny>) -> Self {
        AwaitPy { state: Rc::new(RefCell::new(AwaitState::Start(awaitable))) }
    }
}

impl Future for AwaitPy {
    type Output = Option<Py<PyAny>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut state = self.state.borrow_mut();
        match &mut *state {
            AwaitState::Done(result) => Poll::Ready(result.take()),
            AwaitState::Waiting { waker, .. } => {
                waker.clone_from(cx.waker());
                Poll::Pending
            }
            AwaitState::Start(awaitable) => {
                let awaitable = Python::attach(|py| awaitable.clone_ref(py));
                drop(state);
                let started = Python::attach(|py| -> PyResult<()> {
                    let future = asyncio(py)?.call_method1("ensure_future", (awaitable,))?;
                    // The waker must be in place before the callback can run
                    *self.state.borrow_mut() =
                        AwaitState::Waiting { _future: future.clone().unbind(), waker: cx.waker().clone() };
                    let callback = DoneCallback { state: Some(self.state.clone()) };
                    future.call_method1("add_done_callback", (callback,))?;
                    Ok(())
                });
                match started {
                    Ok(()) => Poll::Pending,
                    // Not awaitable, or no running loop
                    Err(_) => {
                        *self.state.borrow_mut() = AwaitState::Done(None);
                        Poll::Ready(None)
                    }
                }
            }
        }
    }
}

/// The done callback of the asyncio future of an [`AwaitPy`]: it stores the result and
/// wakes the task.
#[pyclass(unsendable)]
struct DoneCallback {
    state: Option<Rc<RefCell<AwaitState>>>,
}

#[pymethods]
impl DoneCallback {
    fn __call__(&mut self, future: &Bound<'_, PyAny>) {
        let Some(state) = self.state.take() else { return };
        let result = match future.call_method0("cancelled").and_then(|c| c.is_truthy()) {
            Ok(false) => future.call_method0("result").ok().map(Bound::unbind),
            _ => None,
        };
        let previous = std::mem::replace(&mut *state.borrow_mut(), AwaitState::Done(result));
        if let AwaitState::Waiting { waker, .. } = previous {
            waker.wake();
        }
    }
}
