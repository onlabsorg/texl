# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

Texl is a text-template engine whose `<% ... %>` tags contain expressions in **Swan**, a small dynamically-typed, expression-oriented language. It is a Cargo workspace (edition 2024, resolver 3) with these crates:

- `swan-core` – the Swan language: lexer → parser → evaluator, runtime values, builtins. It has no dependencies.
- `texl-core` – the template renderer plus the `Engine` (scope setup, document loading, `texl.import`). It depends on `swan-core` and `url`.
- `texl-cli` – the `texl` binary (clap + tokio). It renders a file (a required path, so relative imports can be resolved) to stdout or `-o FILE`, using an `Engine` with a local file loader registered for `file://` and as the default loader.
- `texl-wasm` – wasm-bindgen bindings for the web only (a Node.js crate is planned separately): a `TexlEngine` class (`load`, `render`, `set`, `createContext`, `baseUrl`) and a `TexlContext` (`get`/`set`/`names`/`render`).

Planned crates (`texl-py`, `texl.nvim`, `vscode-texl`) are commented out in the root `Cargo.toml`.

## Commands

```bash
cargo build                                   # whole workspace
cargo test                                    # all Rust unit tests (they live in #[cfg(test)] modules inside src files)
cargo test -p swan-core                       # one crate
cargo test -p texl-core test_engine_caches    # tests whose name contains this string
cargo run -p texl-cli -- texl-cli/tests/fixtures/sample.texl   # render a sample template

cargo test -p texl-wasm --test browser       # builds texl-wasm with wasm-pack and runs tests/web/index.html in headless Chrome (skipped if either is missing)
cargo run -p texl-wasm --example serve        # builds texl-wasm/pkg and serves the browser test page, to inspect it
docs/build.sh                                 # builds the documentation site into target/site (needs mdbook; MDBOOK=path overrides)
./helper.sh build        # wasm-pack build of texl-wasm into texl-wasm/pkg
./helper.sh repomix <dir>                      # writes <dir>.repomix.md, a single-file snapshot used as LLM context
```

`helper.sh`, `*.repomix.md`, `*.todo.md` (the roadmap is `texl.todo.md`), `local/` and `texl-wasm/pkg/` are gitignored local files.

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

The language manual is `docs/src/swan/` (part of the documentation book, see below).

### texl-core

- `template::render(template, &scope)` is `async`. It evaluates each `<% expr %>` in order against one shared scope, so assignments carry over to later tags. When a tag returns a `Value::Promise`, rendering waits for it to resolve before continuing.
- `helpers.rs` keeps the core runtime-agnostic. The host can pass a `SpawnFn` to run background tasks (tokio, or `spawn_local` in wasm). If it does not, tasks go onto a thread-local queue that `run_pending()` drains. `promise_async(spawner, fut)` turns a Rust future into a Swan `Promise`.
- `Engine::new(spawner)` holds the base scope (`root_scope()` plus anything added with `add_builtin`), loaders registered per URL scheme (`add_loader`), a default loader for scheme-less paths (`add_default_loader`), and a cache keyed by normalized URL.
  - Every document scope gets a `texl` namespace whose `import` builtin is bound to that scope. `texl.import(uri)` always returns a Promise that resolves to `{uri, source, data, rendering}`.
  - The cache holds the pending promise while a document is loading, so concurrent imports of the same document share one load. Once loading finishes, the cache holds the resolved namespace.
  - If no loader matches or the load fails, the result is `Nothing` and the cache entry is removed.
- In tests, async code runs on a small busy-polling `block_on` that calls `helpers::run_pending()` on every iteration (see the tests in `engine.rs`).

### texl-wasm

`TexlEngine` wraps an `Engine` that spawns tasks with `wasm_bindgen_futures::spawn_local`. `loader.rs` fetches `http://` and `https://` URLs with the global `fetch`. Imports inside a document fetched by URL resolve to full URLs (absolute paths against the document's root), so the default loader only receives scheme-less paths from templates without a document URL (`render`, contexts); it fetches them from the root folder of the engine's base URL, which defaults to `location.href`. `TexlEngine::load` resolves its URI against the base URL too. `time.now` is replaced with a JS-clock version, since `SystemTime::now` panics on wasm32-unknown-unknown.

`convert.rs` converts between JS values and Swan `Value`s in both directions. JS functions become builtins. JS Promises and async functions become Swan Promises (a rejection resolves to `Nothing`), and Swan Promises come back to JS as JS Promises. `load` and `render` return JS Promises.

The browser test (`tests/browser.rs`) serves the crate folder with a small std-only HTTP server (`tests/support/mod.rs`, shared with `examples/serve.rs`). The page renders `tests/web/fixtures/sample.texl` (the CLI sample plus a web section) and compares it with `sample.expected.txt`, then runs API checks; it sets `data-status="pass"` on `<body>` when all pass.

### Documentation

`docs/` holds the documentation site, published to GitHub Pages by `.github/workflows/docs.yml`:
- `docs/src/` — an mdBook: the template manual, the Swan manual (with the built-in reference), and the CLI, JavaScript and Rust host guides. `docs/book.toml` builds it into `target/book`.
- `docs/site/index.html` — the landing page. `docs/assets/` — shared CSS tokens (`texl.css`, mapped onto mdBook by `book.css` and onto rustdoc by `rustdoc.css`) and the Swan/Texl highlighter.
- The Rust API reference is rustdoc for `swan-core` and `texl-core`, which both use `#![warn(missing_docs)]`. The JavaScript API is documented by the doc comments in `texl-wasm/src/lib.rs` (copied into the generated `.d.ts`) and in `docs/src/hosts/javascript.md`.
- `docs/STYLE.md` defines the narrative style, the API entry format and the example conventions. Every `# =>` result and template rendering in the docs must match the real output.
