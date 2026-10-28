# texl-core

The [Texl](../README.md) template engine: renders text templates whose
`<% ... %>` tags contain [Swan](../swan-core) expressions.

- `template::render` renders a template in a Swan scope.
- `Engine` adds globals, document loaders, and documents that import each
  other with `texl.import`, with an import cache per rendering.
- `helpers` connects the engine to the host's async runtime.

```rust,ignore
use swan_core::value::Value;
use texl_core::{Engine, helpers::spawner_fn};

let engine = Engine::new(Some(spawner_fn(|task| { tokio::task::spawn_local(task); })));
engine.add_builtin("name", Value::Text("Ada".into()));
let doc = engine.render("/hello.texl", "Hello <% name %>!").await;   // inside a tokio LocalSet
```

## Documentation

- [Using Texl from Rust](../docs/api/rust.md)
- [Architecture](../docs/architecture.md)
- API reference: `cargo doc -p texl-core --open`.
- [The user manual](https://www.onlabs.org/manual/): the template language and
  Swan.

## License

[MIT](../LICENSE)
