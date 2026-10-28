# Host functions in Rust

This example adds a host function that takes a Swan function and returns
another one, serves documents from memory, and checks that the rendering
leaves no memory behind. It runs without an async runtime; the
[Rust guide](../api/rust.md) shows the same engine on tokio.

## The program

```rust
use std::collections::HashMap;
use std::rc::Rc;
use std::{future::Future, pin::pin, task::{Context, Poll, Waker}};
use swan_core::evaluator::eval_apply;
use swan_core::gc;
use swan_core::value::{BuiltinFn, Scope, Value};
use texl_core::{Engine, helpers::run_pending, loader_fn};

fn block_on<F: Future>(future: F) -> F::Output {
    let mut cx = Context::from_waker(Waker::noop());
    let mut future = pin!(future);
    loop {
        run_pending();
        if let Poll::Ready(value) = future.as_mut().poll(&mut cx) {
            return value;
        }
    }
}

/// `twice f` returns a function that applies `f` two times.
fn twice(f: Value) -> Value {
    Value::Builtin(BuiltinFn::capturing("twice", vec![f], |captured, arg| {
        let once = eval_apply(captured[0].clone(), arg, 0);
        eval_apply(captured[0].clone(), once, 0)
    }))
}

fn main() {
    let engine = Engine::new(None);
    engine.add_builtin("twice", Value::Builtin(BuiltinFn::new("twice", twice)));

    let docs: Rc<HashMap<String, String>> = Rc::new(HashMap::from([
        ("mem://app/lib/math.texl".to_string(), "<% inc = n -> n + 1, add2 = twice inc %>".to_string()),
    ]));
    engine.add_loader("mem", loader_fn(move |uri| {
        let source = docs.get(&uri).cloned().ok_or(format!("{uri}: not found"));
        async move { source }
    }));

    drop(swan_core::builtins::root_scope());     // creates the built-ins, which live as long as the thread
    let before = Scope::live_count();
    let source = r#"<% texl.import("lib/math.texl") ~> (lib -> lib.data.add2 40) %>"#;
    let Value::Namespace(doc) = block_on(engine.render("mem://app/main.texl", source)) else { unreachable!() };
    println!("{:?}", doc.get("rendering"));
    drop(doc);
    gc::collect();
    println!("leaked scopes: {}", Scope::live_count() - before);
}
```

It prints:

```text
Text("42")
leaked scopes: 0
```

## How it works

- **The host function.** `twice` receives its argument as one `Value`, here
  the Swan function `inc`, and returns a new built-in. `eval_apply` calls any
  Swan callable (a function, a built-in, a list or a namespace) the way Swan
  code would, and never fails: a wrong argument gives an `Undefined`.
- **Capturing Swan values.** The returned built-in holds `inc`, which holds
  the scope of `math.texl`, which holds `add2`, which is the built-in: a
  reference cycle. `BuiltinFn::capturing` keeps `inc` where the cycle
  collector can see it, so `gc::collect` frees the cycle once the document is
  dropped. Written with `BuiltinFn::new` and a `move` closure that captures
  `f`, the same program prints `leaked scopes: 2`: the collector cannot look
  into a closure, so it keeps everything the closure refers to.
- **The loader.** `loader_fn` turns an async function from a URL to
  `Result<String, String>` into a loader. Its `Err` is a failed load, which
  makes the import resolve to `Nothing`.
- **The leak check.** `Scope::live_count` counts the scopes alive on the
  thread. The first `root_scope()` creates the built-ins scope, which is never
  freed, so the count is taken after it. `Engine::render` already runs the
  collector when it is done; the explicit call frees what the document held
  until it was dropped.

See [Memory management](../language.md#memory-management) for the collector,
and [Architecture](../architecture.md) for the crates.
