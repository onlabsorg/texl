//! A cycle collector for Swan memory.
//!
//! Swan values are reference-counted (`Rc`): a value is freed as soon as
//! nothing refers to it. Reference counting cannot free a *cycle*, and Swan
//! code makes cycles all the time. A function holds the scope it was defined
//! in, so a function stored in that scope (or in a scope below it, or in a
//! namespace or list stored there) keeps the scope alive, and the scope keeps
//! the function alive:
//!
//! ```text
//! scope ──"f"──▶ Func ──closure──▶ scope        f = x -> x
//! ```
//!
//! [`collect`] finds the cycles that nothing outside them refers to any more,
//! and frees them. Everything else is still freed by reference counting, at
//! once.
//!
//! # How it works
//!
//! The collector uses *trial deletion*, the algorithm of CPython's `gc`
//! module. Every cycle passes through a scope or a promise, the only mutable
//! objects, so the live scopes and promises (kept in a thread-local list of
//! weak references) are the starting points.
//!
//! 1. **Discover.** From each live scope and promise, follow the references
//!    it holds: a scope's parent and bindings, the items of lists, tuples and
//!    namespaces, a function's closure, the values captured by a
//!    [`BuiltinFn::capturing`](crate::value::BuiltinFn::capturing) built-in,
//!    a promise's value and callbacks. Every scope, promise, namespace, list
//!    and tuple met is a node of the graph.
//! 2. **Subtract.** For each node, subtract the references that come from
//!    other nodes from its reference count. What remains are the references
//!    from outside the graph: the host, a Rust variable, a running task, or a
//!    native closure the collector cannot look into.
//! 3. **Mark.** Nodes with outside references are in use, and so is
//!    everything they reach.
//! 4. **Clear.** The scopes and promises that are not in use are emptied
//!    (bindings, parent, value, callbacks). That breaks every unused cycle,
//!    and reference counting frees the rest.
//!
//! # Safety of the result
//!
//! The collector can keep too much, never too little: a reference it cannot
//! see counts as an outside reference, so what it refers to is kept. Values
//! captured by an opaque closure (a [`BuiltinFn::new`](crate::value::BuiltinFn::new)
//! built-in, a [`Promise::then`](crate::value::Promise::then) callback) are
//! therefore never collected while the closure lives; a cycle through such a
//! closure leaks. Built-ins that hold Swan values should use
//! [`BuiltinFn::capturing`](crate::value::BuiltinFn::capturing).
//!
//! # When to run it
//!
//! Between evaluations, when the host is done with a piece of work: texl-core
//! runs it after each rendering. Running it during an evaluation is safe, but
//! it frees little, since the evaluation holds what it uses.
//!
//! # Examples
//!
//! ```
//! use swan_core::builtins::root_scope;
//! use swan_core::evaluator::eval;
//! use swan_core::gc;
//! use swan_core::lexer::Lexer;
//! use swan_core::parser::Parser;
//! use swan_core::value::Scope;
//!
//! drop(root_scope());     // creates the built-ins, which are never freed
//! let before = Scope::live_count();
//!
//! let scope = root_scope();
//! eval(&Parser::new(Lexer::new("f = x -> x")).parse(), &scope);
//! drop(scope);
//! assert!(Scope::live_count() > before);      // the scope and f hold each other
//!
//! gc::collect();
//! assert_eq!(Scope::live_count(), before);
//! ```

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use crate::value::{Namespace, Promise, PromiseCallback, PromiseData, Scope, Undef, Value, WeakScope};

/// Below this many tracked objects, the list of live objects is not pruned.
const MIN_PRUNE: usize = 256;

/// The scopes and promises created on this thread, held weakly.
struct Registry {
    objects: Vec<Tracked>,
    /// Prune dead entries when the list reaches this length.
    prune_at: usize,
}

enum Tracked {
    Scope(WeakScope),
    Promise(std::rc::Weak<RefCell<PromiseData>>),
}

impl Tracked {
    fn upgrade(&self) -> Option<Obj> {
        match self {
            Tracked::Scope(weak) => weak.upgrade().map(Obj::Scope),
            Tracked::Promise(weak) => weak.upgrade().map(|rc| Obj::Promise(Promise(rc))),
        }
    }

    fn is_alive(&self) -> bool {
        match self {
            Tracked::Scope(weak) => weak.is_alive(),
            Tracked::Promise(weak) => weak.strong_count() > 0,
        }
    }
}

thread_local! {
    static REGISTRY: RefCell<Registry> = const {
        RefCell::new(Registry { objects: Vec::new(), prune_at: MIN_PRUNE })
    };
}

fn track(object: Tracked) {
    // `try_with`: the registry may already be gone when thread-locals are destroyed
    let _ = REGISTRY.try_with(|registry| {
        let Ok(mut registry) = registry.try_borrow_mut() else { return };
        registry.objects.push(object);
        if registry.objects.len() >= registry.prune_at {
            registry.objects.retain(Tracked::is_alive);
            registry.prune_at = (2 * registry.objects.len()).max(MIN_PRUNE);
        }
    });
}

/// Adds a new scope to the objects the collector starts from.
pub(crate) fn track_scope(scope: &Scope) {
    track(Tracked::Scope(scope.downgrade()));
}

/// Adds a new promise to the objects the collector starts from.
pub(crate) fn track_promise(promise: &Promise) {
    track(Tracked::Promise(Rc::downgrade(&promise.0)));
}

/// Frees the scopes and promises that only cycles keep alive.
///
/// See the [module documentation](self). Returns the number of scopes and
/// promises it cleared; reference counting then frees them and everything
/// only they referred to.
pub fn collect() -> usize {
    let mut graph = Graph::default();

    // 1. Discover the graph from the live scopes and promises
    let starts: Vec<Obj> = REGISTRY.with(|registry| match registry.try_borrow_mut() {
        Ok(mut registry) => {
            registry.objects.retain(Tracked::is_alive);
            registry.prune_at = (2 * registry.objects.len()).max(MIN_PRUNE);
            registry.objects.iter().filter_map(Tracked::upgrade).collect()
        }
        Err(_) => Vec::new(),
    });
    for obj in starts {
        graph.node(obj);
    }
    while let Some(id) = graph.todo.pop() {
        graph.expand(id);
    }

    // 2. References from outside the graph: the count, minus the graph's own
    //    handle on the node, minus the references from other nodes
    let mut outside: Vec<isize> = graph.objs.iter().map(|obj| obj.strong_count() as isize - 1).collect();
    for edges in &graph.edges {
        for &to in edges {
            outside[to] -= 1;
        }
    }
    debug_assert!(outside.iter().all(|&n| n >= 0), "gc: an edge without a strong reference");

    // 3. Mark what is in use
    let mut marked = vec![false; graph.objs.len()];
    let mut stack: Vec<usize> = (0..graph.objs.len())
        .filter(|&id| outside[id] > 0 || graph.opaque[id])
        .collect();
    while let Some(id) = stack.pop() {
        if !std::mem::replace(&mut marked[id], true) {
            stack.extend(graph.edges[id].iter().copied().filter(|&to| !marked[to]));
        }
    }

    // 4. Clear the unused scopes and promises. The contents are dropped after
    //    every borrow is released, since dropping them frees other objects.
    let mut cleared = 0;
    let mut contents: Vec<Contents> = Vec::new();
    for (id, obj) in graph.objs.iter().enumerate() {
        if marked[id] {
            continue;
        }
        match obj {
            Obj::Scope(scope) => {
                if let Ok(mut data) = scope.0.try_borrow_mut() {
                    contents.push(Contents::Scope(data.parent.take(), std::mem::take(&mut data.bindings)));
                    cleared += 1;
                }
            }
            Obj::Promise(promise) => {
                if let Ok(mut data) = promise.0.try_borrow_mut() {
                    contents.push(Contents::Promise(data.value.take(), std::mem::take(&mut data.children)));
                    cleared += 1;
                }
            }
            Obj::Map(_) | Obj::List(_) => {}
        }
    }
    drop(graph);
    drop(contents);

    cleared
}

/// What a cleared scope or promise held, dropped once the collection is done.
#[allow(dead_code)]
enum Contents {
    Scope(Option<Scope>, Namespace),
    Promise(Option<Value>, Vec<(PromiseCallback, Value)>),
}

/// An object of the graph. The graph holds it strongly while it runs.
#[derive(Clone)]
enum Obj {
    Scope(Scope),
    Promise(Promise),
    /// The map of a namespace
    Map(Namespace),
    /// The items of a list or tuple, or the values captured by a built-in
    List(Rc<Vec<Value>>),
}

impl Obj {
    fn addr(&self) -> *const () {
        match self {
            Obj::Scope(scope) => Rc::as_ptr(&scope.0) as *const (),
            Obj::Promise(promise) => Rc::as_ptr(&promise.0) as *const (),
            Obj::Map(ns) => Rc::as_ptr(ns.map()) as *const (),
            Obj::List(items) => Rc::as_ptr(items) as *const (),
        }
    }

    fn strong_count(&self) -> usize {
        match self {
            Obj::Scope(scope) => Rc::strong_count(&scope.0),
            Obj::Promise(promise) => Rc::strong_count(&promise.0),
            Obj::Map(ns) => Rc::strong_count(ns.map()),
            Obj::List(items) => Rc::strong_count(items),
        }
    }
}

/// The objects reachable from the live scopes and promises, and the strong
/// references between them.
#[derive(Default)]
struct Graph {
    index: HashMap<*const (), usize>,
    objs: Vec<Obj>,
    /// One entry per strong reference: a node that refers to another twice has two edges to it.
    edges: Vec<Vec<usize>>,
    /// Nodes whose contents could not be read (borrowed elsewhere): kept, as if used.
    opaque: Vec<bool>,
    /// Nodes not expanded yet.
    todo: Vec<usize>,
}

impl Graph {
    /// Returns the id of `obj`, adding it to the graph if it is new.
    fn node(&mut self, obj: Obj) -> usize {
        let addr = obj.addr();
        if let Some(&id) = self.index.get(&addr) {
            return id;
        }
        let id = self.objs.len();
        self.index.insert(addr, id);
        self.objs.push(obj);
        self.edges.push(Vec::new());
        self.opaque.push(false);
        self.todo.push(id);
        id
    }

    /// Finds the edges of node `id`.
    fn expand(&mut self, id: usize) {
        let mut edges = Vec::new();
        match self.objs[id].clone() {
            Obj::Scope(scope) => match scope.0.try_borrow() {
                Ok(data) => {
                    if let Some(parent) = &data.parent {
                        edges.push(self.node(Obj::Scope(parent.clone())));
                    }
                    edges.push(self.node(Obj::Map(data.bindings.clone())));
                }
                Err(_) => self.opaque[id] = true,
            },
            Obj::Promise(promise) => match promise.0.try_borrow() {
                Ok(data) => {
                    if let Some(value) = &data.value {
                        self.value(value, &mut edges);
                    }
                    for (callback, child) in &data.children {
                        if let PromiseCallback::Apply(f, _) = callback {
                            self.value(f, &mut edges);
                        }
                        self.value(child, &mut edges);
                    }
                }
                Err(_) => self.opaque[id] = true,
            },
            Obj::Map(ns) => {
                for (_, value) in ns.iter() {
                    self.value(value, &mut edges);
                }
            }
            Obj::List(items) => {
                for value in items.iter() {
                    self.value(value, &mut edges);
                }
            }
        }
        self.edges[id] = edges;
    }

    /// Adds to `edges` the nodes that `value` holds a strong reference to.
    fn value(&mut self, value: &Value, edges: &mut Vec<usize>) {
        match value {
            Value::List(items) | Value::Tuple(items) => edges.push(self.node(Obj::List(items.clone()))),
            Value::Namespace(ns) => edges.push(self.node(Obj::Map(ns.clone()))),
            Value::Func { params, closure, .. } => {
                self.value(params, edges);
                edges.push(self.node(Obj::Scope(closure.clone())));
            }
            Value::Builtin(builtin) => {
                if let Some(captured) = builtin.captured() {
                    edges.push(self.node(Obj::List(captured.clone())));
                }
            }
            Value::Promise(promise) => edges.push(self.node(Obj::Promise(promise.clone()))),
            Value::Undefined(undef) => match undef {
                Undef::UnaryOperation { val, .. } => self.value(val, edges),
                Undef::BinaryOperation { lhv, rhv, .. } => {
                    self.value(lhv, edges);
                    self.value(rhv, edges);
                }
                Undef::Value(val) => self.value(val, edges),
                Undef::Termination { .. } | Undef::Token { .. } | Undef::Name { .. } => {}
            },
            Value::Nothing | Value::Bool(_) | Value::Numb(_) | Value::Text(_) | Value::Name(_) => {}
        }
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtins::root_scope;
    use crate::evaluator::eval;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn run(source: &str, scope: &Scope) -> Value {
        eval(&Parser::new(Lexer::new(source)).parse(), scope)
    }

    /// Evaluates `source` in a new root scope, drops the scope, and returns the
    /// number of scopes left alive before and after a collection.
    fn leaked(source: &str) -> (usize, usize) {
        drop(root_scope());
        collect();
        let before = Scope::live_count();
        let scope = root_scope();
        run(source, &scope);
        drop(scope);
        let leaked = Scope::live_count() - before;
        collect();
        (leaked, Scope::live_count() - before)
    }

    #[test]
    fn test_collect_frees_cycles() {
        let cases = [
            "f = x -> x, f 1",
            "fact = n -> n <= 1 ? 1 ; n * fact (n - 1), fact 5",
            "even = n -> n == 0 | odd (n - 1), odd = n -> n != 0 & even (n - 1), even 4",
            "ns = {f: x -> x}",
            "l = [x -> x]",
            "t = (x -> x, y -> y)",
            "g = (() -> (y = 1, () -> y))()",
            "p = promise.create(), q = p ~> (x -> x)",
            "f = x -> x, h = f >> f",
            "f = x -> x, m = list.map f",
            "u = undef.from (x -> x)",
            "f = x -> x, ns = {a: {b: [f]}}",
        ];
        for source in cases {
            let (leaked, after) = leaked(source);
            assert!(leaked > 0, "{source}: no cycle to collect");
            assert_eq!(after, 0, "{source}: not collected");
        }
    }

    #[test]
    fn test_collect_frees_promise_cycles() {
        // A promise resolved with a list that holds it: no scope in the cycle
        let p = Promise::new();
        let weak = Rc::downgrade(&p.0);
        p.resolve(Value::list(vec![Value::Promise(p.clone())]));
        drop(p);
        assert!(weak.upgrade().is_some());
        collect();
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn test_collect_keeps_what_is_held() {
        drop(root_scope());
        let scope = root_scope();
        run("rate = 2, convert = a -> a * rate, fact = n -> n <= 1 ? 1 ; n * fact (n - 1)", &scope);

        // Held by the host: the scope and everything it reaches
        collect();
        assert_eq!(run("convert 10, fact 4", &scope), Value::tuple(vec![Value::Numb(20.0), Value::Numb(24.0)]));

        // Held through a function taken out of the scope only
        let convert = scope.get("convert");
        drop(scope);
        collect();
        let Value::Func { closure, .. } = &convert else { panic!() };
        assert_eq!(closure.get("rate"), Value::Numb(2.0));
        assert_eq!(crate::evaluator::eval_apply(convert.clone(), Value::Numb(5.0), 0), Value::Numb(10.0));
    }

    #[test]
    fn test_collect_keeps_namespaces_held_by_the_host() {
        drop(root_scope());
        let scope = root_scope();
        run("x = 3, f = () -> x", &scope);
        let data = scope.namespace();     // shares its map with the scope's bindings
        drop(scope);

        collect();
        let Value::Func { closure, .. } = data.get("f") else { panic!() };
        assert_eq!(closure.get("x"), Value::Numb(3.0));
    }

    #[test]
    fn test_collect_keeps_pending_promises_held_by_the_host() {
        drop(root_scope());
        let scope = root_scope();
        let Value::Promise(p) = run("p = promise.create(), q = p ~> (x -> x + y), y = 1, p", &scope) else { panic!() };
        let Value::Promise(q) = scope.get("q") else { panic!() };
        drop(scope);

        collect();
        p.resolve(Value::Numb(41.0));
        assert_eq!(q.0.borrow().value, Some(Value::Numb(42.0)));
    }

    #[test]
    fn test_collect_keeps_scopes_in_use() {
        // A scope borrowed by the caller is kept, whatever holds it
        drop(root_scope());
        let scope = root_scope();
        run("f = x -> x", &scope);
        let held = scope.clone();
        let _borrow = held.0.borrow_mut();
        drop(scope);
        assert_eq!(collect(), 0);
    }

    #[test]
    fn test_collect_ignores_cycles_held_from_outside() {
        drop(root_scope());
        collect();
        let before = Scope::live_count();
        let scope = root_scope();
        run("f = x -> x", &scope);
        let f = scope.get("f");
        drop(scope);
        collect();
        assert!(Scope::live_count() > before);
        drop(f);
        collect();
        assert_eq!(Scope::live_count(), before);
    }
}
