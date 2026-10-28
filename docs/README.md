# Texl developer documentation

This folder documents Texl for programmers: how to embed the engine in a
program, its APIs, how it is built, and how the language works inside. The
user manual, which teaches the template language and the Swan language to
everyone, is linked from the [README](../README.md).

| Page                                          | Content                                                        |
|-----------------------------------------------|----------------------------------------------------------------|
| [Getting started](getting-started.md)         | Install the Python package and render a first template         |
| [Hosts and APIs](api/README.md)               | The JavaScript, Python and Rust APIs                            |
| — [JavaScript](api/javascript.md)             | `@onlabsorg/texl`: guide and API reference                     |
| — [Python](api/python.md)                     | `pytexl`: guide and API reference                              |
| — [Rust](api/rust.md)                         | `texl-core` and `swan-core`: guide (the reference is rustdoc)  |
| [Architecture](architecture.md)               | The crates, the path from template text to rendering, extension points |
| [Swan and Texl in depth](language.md)         | Design principles, scopes, evaluation order, promises, imports and caching, memory management, sandboxing, limits |
| [Examples](examples/README.md)                | Tutorials: a web page, a Python report, custom loaders, Rust host functions |
| [Style guide](STYLE.md)                       | Conventions for this documentation and for doc comments        |

The Rust API reference is generated from the doc comments:

```sh
cargo doc -p texl-core -p swan-core --no-deps --open
```
