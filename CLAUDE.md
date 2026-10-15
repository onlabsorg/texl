# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

Texl is a text-template engine whose `<% ... %>` tags contain expressions in **Swan**, a small dynamically-typed, expression-oriented language. It is a Cargo workspace (edition 2024, resolver 3) with these crates:

- `swan-core` – the Swan language: lexer → parser → evaluator, runtime values, builtins. It has no dependencies.
- `texl-core` – the template renderer plus the `Engine` (scope setup, document loading, `texl.import`). It depends on `swan-core` and `url`.
- `texl-cli` – the `texl` binary (clap + tokio). It renders a file (a required path, so relative imports can be resolved) to stdout or `-o FILE`, using an `Engine` with a local file loader registered for `file://` and as the default loader.
- `texl-wasm` – wasm-bindgen bindings (`create_context`, `render`, a `TexlContext` with `get`/`set`/`names`) for web and Node.

Planned crates (`texl-py`, `texl.nvim`, `vscode-texl`) are commented out in the root `Cargo.toml`.

## Commands

```bash
cargo build                                   # whole workspace
cargo test                                    # all Rust unit tests (they live in #[cfg(test)] modules inside src files)
cargo test -p swan-core                       # one crate
cargo test -p texl-core test_engine_caches    # tests whose name contains this string
cargo run -p texl-cli -- texl-cli/tests/fixtures/sample.texl   # render a sample template

./helper.sh build        # wasm-pack build of texl-wasm into texl-wasm/pkg-web and texl-wasm/pkg-node
node texl-wasm/tests/test-texl-node.js        # JS integration tests (run ./helper.sh build first; they load pkg-node)
./helper.sh repomix <dir>                      # writes <dir>.repomix.md, a single-file snapshot used as LLM context
```

`helper.sh`, `*.repomix.md`, `*.todo.md` (the roadmap is `texl.todo.md`), `local/` and `pkg-*/` are gitignored local files.

**Current state:** commit 28be11f replaced `texl_core::context` with `texl_core::Engine`. `texl-cli` has been moved to `Engine`, but `texl-wasm/src/lib.rs` still imports `texl_core::context`, so `cargo build` of the whole workspace fails until it is moved too. Meanwhile, use `cargo test -p swan-core -p texl-core -p texl-cli`.

## Architecture

### swan-core pipeline

`Lexer` (zero-copy; yields `(byte_offset, Token<'a>)`) → `Parser` (a Pratt parser in `parser.rs`; binding powers are in its `lbp` table and match the precedence numbers documented in `token.rs`) → `ast::Node` → `evaluator::eval(&Node, &Scope) -> Value`.

Invariants that apply across the crate:
- **Evaluation never fails or panics.** Every error, including syntax errors, is returned as `Value::Undefined(Undef)` and passed on like any other value. Builtins follow the same rule, using helpers such as `Value::undef_unary(op, arg, pos)`.
- **Tuples are flat and normalized.** Every value is conceptually a tuple: `Nothing` is the empty tuple, and `Nothing` items are dropped. Operators are lifted over tuples item by item through the `map_values!` and `map_pairs!` macros in `evaluator.rs`.
- **Single-threaded, `Rc`-based.** `Value` clones are cheap, and `Scope`, `Promise` and `BuiltinFn` are shared handles. Nothing is `Send`.
- `Scope` is both the lexical environment and the storage behind `Namespace`. It uses parent-chain (prototype) lookup, and `new_child()` creates a child scope.
- `Promise` (`value/promise.rs`) is a single-assignment value that never rejects. Code chains on it with `then`, and Swan code chains with the `~>` operator (precedence 13, `evaluator/then_operation.rs`).
- `builtins::root_scope()` builds the global scope with the `bool`, `numb`, `text`, `list`, `nspace`, `promise`, `undef`, `math` and `time` namespaces plus root functions.

`evaluator.rs` handles most node kinds itself. Operations with more involved logic are in the `evaluator/*_operation(s).rs` submodules.

The full language spec is `swan-core/docs/swan_language_manual.md`. `swan-core/README.md` is partly out of date: it still describes the parser and evaluator as stubs, and its operator table is missing `~>`.

### texl-core

- `template::render(template, &scope)` is `async`. It evaluates each `<% expr %>` in order against one shared scope, so assignments carry over to later tags. When a tag returns a `Value::Promise`, rendering waits for it to resolve before continuing.
- `helpers.rs` keeps the core runtime-agnostic. The host can pass a `SpawnFn` to run background tasks (tokio, or `spawn_local` in wasm). If it does not, tasks go onto a thread-local queue that `run_pending()` drains. `promise_async(spawner, fut)` turns a Rust future into a Swan `Promise`.
- `Engine::new(spawner)` holds the base scope (`root_scope()` plus anything added with `add_builtin`), loaders registered per URL scheme (`add_loader`), a default loader for scheme-less paths (`add_default_loader`), and a cache keyed by normalized URL.
  - Every document scope gets a `texl` namespace whose `import` builtin is bound to that scope. `texl.import(uri)` always returns a Promise that resolves to `{uri, source, data, rendering}`.
  - The cache holds the pending promise while a document is loading, so concurrent imports of the same document share one load. Once loading finishes, the cache holds the resolved namespace.
  - If no loader matches or the load fails, the result is `Nothing` and the cache entry is removed.
- In tests, async code runs on a small busy-polling `block_on` that calls `helpers::run_pending()` on every iteration (see the tests in `engine.rs`).

### texl-wasm

This crate converts between JS values and Swan `Value`s in both directions. JS functions become builtins. JS Promises and async functions become Swan Promises (a rejection resolves to `Nothing`), and Swan Promises come back to JS as JS Promises. `render` returns a JS Promise.
