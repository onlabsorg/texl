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

**Documentation: <https://onlabsorg.github.io/texl/>**

- [Getting started](https://onlabsorg.github.io/texl/book/getting-started.html)
- [Writing templates](https://onlabsorg.github.io/texl/book/templates/index.html)
- [The Swan language](https://onlabsorg.github.io/texl/book/swan/index.html)
- [JavaScript API](https://onlabsorg.github.io/texl/book/hosts/javascript.html),
  [Python API](https://onlabsorg.github.io/texl/book/hosts/python.html),
  [Rust API](https://onlabsorg.github.io/texl/api/)

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
docs/build.sh                                   # build the documentation site into target/site
```

The documentation sources are in [`docs`](docs); see
[`docs/STYLE.md`](docs/STYLE.md) for the conventions.

## License

[MIT](LICENSE)
