# Hosts and APIs

A *host* is the program that runs the Texl engine: it gives the engine a
template, the data the template needs, and a way to load the documents it
imports. The template language is the same in every host.

| Host                         | Use it to                                               | Reference |
|------------------------------|---------------------------------------------------------|-----------|
| [JavaScript](javascript.md)  | render templates in a web page or a web worker          | [JavaScript API](javascript.md#api-reference) |
| [Python](python.md)          | render templates in a Python program, with asyncio      | [Python API](python.md#api-reference) |
| [Rust](rust.md)              | render templates in a Rust program, or build a new host | [`texl_core`](../../api/texl_core/index.html), [`swan_core`](../../api/swan_core/index.html) |
| Node.js                      | coming soon                                             |           |

## What differs between hosts

The hosts differ in three ways:

- **Which locations they can load.** The JavaScript library fetches URLs over
  HTTP; the Python library reads local files and fetches URLs. Python and Rust
  programs can add their own *loaders*.
- **Which globals they pass.** JavaScript, Python and Rust programs pass any
  values and functions they want.
- **How they run asynchronous work.** Each host connects the engine to its own
  event loop: the browser's, asyncio in Python. This only matters to Rust
  programs, which choose the runtime.

## The crates

Texl is a Cargo workspace with four crates:

| Crate       | Contents                                                              |
|-------------|-----------------------------------------------------------------------|
| `swan-core` | The Swan language: lexer, parser, evaluator and built-in library.     |
| `texl-core` | The template renderer and the engine: documents, loaders, imports.    |
| `texl-wasm` | The JavaScript library, compiled to WebAssembly (`@onlabsorg/texl` on npm). |
| `pytexl`    | The Python library: a native module and the `pytexl` package.         |

The API documentation of `swan-core` and `texl-core` is generated from the
source code by rustdoc.
