# Hosts and APIs

A *host* is the program that runs the Texl engine: it gives the engine a
template, the data the template needs, and a way to load the documents it
imports. The template language is the same in every host.

| Host                         | Use it to                                               | Reference |
|------------------------------|---------------------------------------------------------|-----------|
| [Command line](cli.md)       | render template files from a terminal or a script       | [`texl` command](cli.md#reference) |
| [JavaScript](javascript.md)  | render templates in a web page or a web worker          | [JavaScript API](javascript.md#api-reference) |
| [Rust](rust.md)              | render templates in a Rust program, or build a new host | [`texl_core`](../../api/texl_core/index.html), [`swan_core`](../../api/swan_core/index.html) |
| Python                       | coming soon                                             |           |
| Node.js                      | coming soon                                             |           |

## What differs between hosts

The hosts differ in three ways:

- **Which locations they can load.** The command-line tool reads local files;
  the JavaScript library fetches URLs over HTTP. A Rust program decides for
  itself, by registering *loaders*.
- **Which globals they pass.** The command-line tool passes none. JavaScript and
  Rust programs pass any values and functions they want.
- **How they run asynchronous work.** Each host connects the engine to its own
  event loop. This only matters to Rust programs.

## The crates

Texl is a Cargo workspace with four crates:

| Crate       | Contents                                                              |
|-------------|-----------------------------------------------------------------------|
| `swan-core` | The Swan language: lexer, parser, evaluator and built-in library.     |
| `texl-core` | The template renderer and the engine: documents, loaders, imports.    |
| `texl-cli`  | The `texl` command.                                                   |
| `texl-wasm` | The JavaScript library, compiled to WebAssembly.                      |

The API documentation of `swan-core` and `texl-core` is generated from the
source code by rustdoc.
