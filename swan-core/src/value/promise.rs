use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;
use crate::value::Value;
use crate::evaluator::eval_apply;

pub type PromiseCallback = Rc<dyn Fn(Value) -> Value>;

#[derive(Clone)]
pub struct PromiseData {
    pub value: Option<Value>,
    pub children: Vec<(PromiseCallback, Value)>,
}

impl fmt::Debug for PromiseData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PromiseData")
            .field("value", &self.value)
            .field("children_count", &self.children.len())
            .finish()
    }
}

#[derive(Clone, Debug)]
pub struct Promise(pub Rc<RefCell<PromiseData>>);

impl Promise {
    pub fn new() -> Self {
        Self(Rc::new(RefCell::new(PromiseData {
            value: None,
            children: Vec::new(),
        })))
    }

    /// Primary Rust-level method: takes a Rust function/closure `f`
    /// and returns a child `Value::Promise`.
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

    /// User/Swan code method: takes a callable `Value` `f` and delegates
    /// to `self.then` by wrapping it with `eval_apply`.
    pub fn then_apply(&self, f: Value, pos: usize) -> Value {
        self.then(move |v| eval_apply(f.clone(), v, pos))
    }

    /// Resolves the promise value to `Some(v)` (converting `Undefined` to `Nothing`),
    /// and resolves all child promises.
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
