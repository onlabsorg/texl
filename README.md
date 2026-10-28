# Texl

Texl is a template engine for text. A Texl template is any text, such as
Markdown, HTML or a configuration file, with expressions between `<%` and `%>`.
The expressions are written in **Swan**, a small expression language that never
stops with an error: an invalid operation produces an `Undefined` value instead.

```text
<% order = {id: 1042, items: ["pen", "ink"], total: 18.5}
%>Order #<% order.id %>
Items: <% list.enum order.items %>
Total: $<% order.total %><% order.total > 15 ? " (free shipping)" %>
```

renders as

```text
Order #1042
Items: pen, ink
Total: $18.5 (free shipping)
```

## Documentation

**For everyone who writes templates:** the user manual at
<https://www.onlabs.org/manual/> teaches the template language and
[the Swan language](https://www.onlabs.org/manual/swan/index.html), with the
[built-in reference](https://www.onlabs.org/manual/swan/builtins/index.html).

**For programmers:** the developer documentation is in [`docs`](docs/README.md):

- [Getting started](docs/getting-started.md)
- [JavaScript API](docs/api/javascript.md), [Python API](docs/api/python.md),
  [Rust guide](docs/api/rust.md) (the Rust API reference is rustdoc:
  `cargo doc --open`)
- [Architecture](docs/architecture.md)
- [Swan and Texl in depth](docs/language.md): design, scopes, async
  rendering, memory management, sandboxing
- [Examples](docs/examples/README.md)

## Crates

| Crate                    | Contents                                                   |
|--------------------------|------------------------------------------------------------|
| [`swan-core`](swan-core) | The Swan language: lexer, parser, evaluator, built-ins.    |
| [`texl-core`](texl-core) | The template renderer and the engine.                      |
| [`texl-wasm`](texl-wasm) | The JavaScript library for the browser (WebAssembly), published on npm as `@onlabsorg/texl`. |
| [`pytexl`](pytexl)       | The Python library (a native module built with PyO3).      |

A Node.js library is coming soon.

## Development

```sh
cargo build                                     # build the workspace
cargo test                                      # run the tests
cargo doc --no-deps --open                      # build and open the Rust API reference
```

See [`docs/STYLE.md`](docs/STYLE.md) for the documentation conventions.

## License

[MIT](LICENSE)
