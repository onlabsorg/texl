# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

Texl is a text-template engine whose `<% ... %>` tags contain expressions in **Swan**, a small dynamically-typed, expression-oriented language. It is a Cargo workspace (edition 2024, resolver 3) with these crates:

- `swan-core` – the Swan language: lexer → parser → evaluator, runtime values, builtins. It has no dependencies.
- `texl-core` – the template renderer plus the `Engine` (scope setup, document loading, `texl.import`). It depends on `swan-core` and `url`.
- `texl-cli` – the `texl` binary (clap + tokio). It renders a file (a required path, so relative imports can be resolved) to stdout or `-o FILE`, using an `Engine` with a local file loader registered for `file://` and as the default loader.
- `texl-wasm` – published on npm as `@onlabsorg/texl`. wasm-bindgen bindings for the web only (a Node.js crate is planned separately): a `TexlEngine` class (`fetch`, `load`, `render(uri, source)`, `set`, `createContext`, `baseUrl`) and a `TexlContext` (`get`/`set`/`names`/`render`).
- `pytexl` – Python bindings (PyO3 0.30, abi3-py310, built with maturin). The native module `pytexl._pytexl` (`src/`) is wrapped by the pure-Python package `python/pytexl`: `Engine` (`set`, `add_loader`, `set_default_loader`, async `fetch`/`load`/`render(uri, source)`, `create_context`, `base_url`), `Context`, `Document`, plus the native `Promise` and `Function`, and `pytexl.loaders` (file loader, also the default; HTTP(S) loader).

Planned crates (`texl.nvim`, `vscode-texl`) are commented out in the root `Cargo.toml`.

## Summary for OnLabs.org (`texl.summary.md`)

`texl.summary.md` (repository root, committed) is the factual summary of
this repository for the OnLabs.org coordination folder (the parent folder):
what it is, how it is built at a high level, published versions, status,
next steps and known limitations. Strategy sessions read it instead of the
code. Strategic matters (why it matters for OnLabs, priorities, timing,
strategic open points) are not written here: they live in the coordination
folder's `SUBPROJECTS.md`.

**When to update it:** only when the user runs `/update-summary` (the
skill in `.claude/skills/update-summary/`). The skill reads the commits
made since the commit in the summary's `Checked against:` line, updates the
summary from the committed changes only (never from uncommitted or
unstaged work), stamps the line with `HEAD` and today's date, and leaves the
result for the user to review and commit. Do not update the summary on your
own initiative; if a task clearly makes it outdated, say so once at the end
and suggest committing and running `/update-summary`.

**Decisions from the coordination folder:** at the start of a session, if
`../DECISIONS.md` exists, check it for entries dated after the summary's
`Checked against:` date that concern this repository, and tell the user
about any you find before starting the task.

Rules:

- Stay at helicopter level, in plain words for a non-programmer reader; code
  details belong in this repository's docs and this file.
- Do not write strategic content or decisions into the summary.
- English; Markdown wrapped at about 80 characters.

## Commands

```bash
cargo build                                   # whole workspace
cargo test                                    # all Rust unit tests (they live in #[cfg(test)] modules inside src files)
cargo test -p swan-core                       # one crate
cargo test -p texl-core test_engine_diamond   # tests whose name contains this string
cargo run -p texl-cli -- texl-cli/tests/fixtures/sample.texl   # render a sample template

cargo test -p texl-wasm --test browser       # builds texl-wasm with wasm-pack and runs tests/web/index.html in headless Chrome (skipped if either is missing)
cargo run -p texl-wasm --example serve        # builds texl-wasm/pkg and serves the browser test page, to inspect it
cargo test -p pytexl --test python            # builds the native module and runs the Python unittest suite (pytexl/tests/test_*.py); skipped without python3 >= 3.10 (PYTEXL_PYTHON overrides)
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
- **Short-circuit operators.** `&`, `|`, `?` and `;` are not lifted over tuples: they test their left operand as a whole (`bool::from`, a tuple is true if any item is; `;` checks for `Nothing`) and evaluate the right operand only when it is the result.
- **Single-threaded, `Rc`-based.** `Value` clones are cheap, and `Scope`, `Promise` and `BuiltinFn` are shared handles. Nothing is `Send`.
- `Scope` is both the lexical environment and the storage behind `Namespace`. It uses parent-chain (prototype) lookup, and `new_child()` creates a child scope.
- `Promise` (`value/promise.rs`) is a single-assignment value that never rejects. Code chains on it with `then`, and Swan code chains with the `~>` operator (precedence 13, `evaluator/then_operation.rs`).
- `builtins::root_scope()` returns a new, empty user scope below a per-call layer holding `bind`/`label` (which hold the user scope as a `WeakScope`, so no cycle), below a thread-local built-ins scope with the `bool`, `numb`, `text`, `list`, `nspace`, `promise`, `undef`, `math` and `time` namespaces plus root functions. The built-ins scope is shared by every root scope of the thread and never freed (its namespaces are its children); it and its namespaces are frozen (`Scope::freeze`: Swan `=`/`:` in them give `Undefined`, host `Scope::set` still works, children are not frozen); a root scope is freed with its last reference. Swan closures (`Func`) still capture their defining scope strongly, so a scope that stores a function defined in it is never freed.

`evaluator.rs` handles most node kinds itself. Operations with more involved logic are in the `evaluator/*_operation(s).rs` submodules.

The language manual is `docs/src/swan/` (part of the documentation book, see below).

### texl-core

- `template::render(template, &scope)` is `async`. It evaluates each `<% expr %>` in order against one shared scope, so assignments carry over to later tags. When a tag returns a `Value::Promise`, rendering waits for it to resolve before continuing.
- `helpers.rs` keeps the core runtime-agnostic. The host can pass a `SpawnFn` to run background tasks (tokio, or `spawn_local` in wasm). If it does not, tasks go onto a thread-local queue that `run_pending()` drains. `promise_async(spawner, fut)` turns a Rust future into a Swan `Promise`.
- `Engine::new(spawner)` holds configuration only: the globals added with `add_builtin` (a name → value map), loaders registered per URL scheme (`add_loader`) and a default loader for scheme-less paths (`add_default_loader`). There is no engine-wide scope or cache.
- `Engine::create_scope()` creates a new `root_scope()`, copies the globals into it, and returns a child of it with a new, empty `texl` namespace (no `import`). Globals added later do not reach existing scopes. `bind`/`label` write into that root scope, so they are per document (visible to its tags, not in its `data`). The bindings expose it as `createContext(vars)` / `create_context(vars)`.
- `fetch(url) -> Option<String>` normalizes the URL (fragment removed, query kept as is) and calls the matching loader; no caching. `render(url, source) -> Value` renders a source as the document at `url` and returns `{uri, source, data, rendering}`. `load(url)` is `fetch` + `render` (`Nothing` if the fetch fails, without rendering).
  - Each `render` creates an `Imports` (per-render import cache + wait-for graph), stores the root document in it as a pending promise, and adds `import` to the `texl` namespace of every document it renders. The `import` builtin holds `Imports` weakly (only `render` and running import tasks hold it strongly), so a finished rendering is freed when the host drops the document; an import called after that starts a new `Imports`. `texl.import` fetches and renders the imported document in its own `create_scope()` scope (not a child of the importer's), with the same `Imports`.
  - The cache stores the promise as soon as an import starts, so concurrent imports share one load. A failed load resolves to `Nothing` and removes the entry.
  - Cycles: before a document X waits for a pending Y, a DFS checks whether Y reaches X in the wait-for graph (X → X counts); if so the import returns `Undefined` (`import.cycle`), otherwise the edge X → Y is added and removed when Y resolves.
- In tests, async code runs on a small busy-polling `block_on` that calls `helpers::run_pending()` on every iteration (see the tests in `engine.rs`).

### texl-wasm

`TexlEngine` wraps an `Engine` that spawns tasks with `wasm_bindgen_futures::spawn_local`. `loader.rs` fetches `http://` and `https://` URLs with the global `fetch`. `fetch`, `render(uri, source)` and `load` resolve their URI against the base URL (default `location.href`), so every document has a full URL and its imports resolve to full URLs (absolute paths against the document's root): there is no default loader. Invalid URIs reject the returned promise. `time.now` is replaced with a JS-clock version, since `SystemTime::now` panics on wasm32-unknown-unknown.

`convert.rs` converts between JS values and Swan `Value`s in both directions. JS functions become builtins. JS Promises and async functions become Swan Promises (a rejection resolves to `Nothing`), and Swan Promises come back to JS as JS Promises. `load` and `render` return JS Promises.

The browser test (`tests/browser.rs`) serves the crate folder with a small std-only HTTP server (`tests/support/mod.rs`, shared with `examples/serve.rs`). The page renders `tests/web/fixtures/sample.texl` (the CLI sample plus a web section) and compares it with `sample.expected.txt`, then runs API checks; it sets `data-status="pass"` on `<body>` when all pass.

### pytexl

`runtime.rs` runs the engine's `!Send` futures on the asyncio loop of the current thread: `spawn` stores a task in a thread-local executor and schedules `drain` with `loop.call_soon`; wakers re-mark tasks ready. Tasks spawned with no running loop wait for `kick()`, which every `Promise.__await__` calls. `AwaitPy` awaits a Python awaitable from Rust (via `asyncio.ensure_future` + a done callback). Native classes are `unsendable` and the module is `gil_used = true`; the Python `Engine`/`Context` raise `RuntimeError` on other threads.

`convert.rs` converts values: Python tuples ↔ Swan tuples, mappings → namespaces, awaitables → promises, callables → builtins (a Swan tuple argument is spread as positional args, `Nothing` means no args; an exception gives `Nothing`). Integral numbers come back as `int`. Swan promises/functions come back as `pytexl.Promise` (awaitable) and `pytexl.Function` (callable). `Nothing` is `None`; `Undefined` is `pytexl.Undefined`, which exposes the `undef.inspect` members as attributes and round-trips back to Swan.

Python tests use stdlib `unittest` (`IsolatedAsyncioTestCase`) and a local `http.server` (`tests/support.py`); `tests/fixtures/sample.texl` is the CLI sample plus a Python section, compared with `sample.expected.txt`. The cargo test `tests/python.rs` builds the module with `PYO3_BUILD_EXTENSION_MODULE=1` in its own target dir, stages it next to the Python sources and runs `python -m unittest discover`. API docstrings are Google style.

### Releases

Three tag-triggered workflows build the releases; the tag version must match the crate version.
- `.github/workflows/texl-cli-release.yml` – generated by [dist](https://github.com/axodotdev/cargo-dist) from `dist-workspace.toml` (and `[profile.dist]` in `Cargo.toml`); do not edit it by hand, change the config and run `dist generate`. A `texl-cli-v<version>` tag builds `texl` for Linux (gnu x86_64/aarch64, musl x86_64), macOS (x86_64/aarch64) and Windows x86_64, and creates a GitHub Release with the archives and shell/PowerShell installers.
- `.github/workflows/pytexl.yml` – adapted from `maturin generate-ci github`. It builds abi3 wheels (manylinux and musllinux x86_64/aarch64, Windows x64/arm64, macOS x86_64/arm64) and the sdist, runs the Python tests against the installed wheel on each OS with Python 3.10 and 3.14, and on a `pytexl-v<version>` tag publishes to PyPI through trusted publishing (environment `pypi`).
- `.github/workflows/texl-wasm.yml` – runs the browser test, builds `texl-wasm/pkg` with wasm-pack, renames the package to `@onlabsorg/texl` (`npm pkg set`) and adds the root `LICENSE`. On a `texl-wasm-v<version>` tag it publishes to npm through trusted publishing (environment `npm`). The crate README is also the npm package README, so its links are absolute.

### Documentation

`docs/` holds the documentation site, published to GitHub Pages by `.github/workflows/docs.yml`:
- `docs/src/` — an mdBook: the template manual, the Swan manual (with the built-in reference), and the CLI, JavaScript, Python and Rust host guides. `docs/book.toml` builds it into `target/book`.
- `docs/site/index.html` — the landing page. `docs/assets/` — shared CSS tokens (`texl.css`, mapped onto mdBook by `book.css` and onto rustdoc by `rustdoc.css`) and the Swan/Texl highlighter.
- The Rust API reference is rustdoc for `swan-core` and `texl-core`, which both use `#![warn(missing_docs)]`. The JavaScript API is documented by the doc comments in `texl-wasm/src/lib.rs` (copied into the generated `.d.ts`) and in `docs/src/hosts/javascript.md`. The Python API is documented by the docstrings in `pytexl/python/pytexl` and in `docs/src/hosts/python.md`.
- `docs/STYLE.md` defines the narrative style, the API entry format and the example conventions. Every `# =>` result and template rendering in the docs must match the real output.
