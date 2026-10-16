# texl-core

The [Texl](../README.md) template engine: renders text templates whose
`<% ... %>` tags contain [Swan](../swan-core) expressions.

- `template::render` renders a template in a Swan scope.
- `Engine` adds globals, document loaders, `texl.import` and a cache.
- `helpers` connects the engine to the host's async runtime.

```rust,ignore
use swan_core::value::Value;
use texl_core::{Engine, helpers::spawner_fn};

let engine = Engine::new(Some(spawner_fn(|task| { tokio::task::spawn_local(task); })));
engine.add_builtin("name", Value::Text("Ada".into()));
let text = engine.render("Hello <% name %>!").await;   // inside a tokio LocalSet
```

## Documentation

- [Using Texl from Rust](https://onlabsorg.github.io/texl/book/hosts/rust.html)
- [API reference](https://onlabsorg.github.io/texl/api/texl_core/index.html)
  (or `cargo doc -p texl-core --open`)
- [Writing templates](https://onlabsorg.github.io/texl/book/templates/index.html)

## License

[MIT](../LICENSE)
