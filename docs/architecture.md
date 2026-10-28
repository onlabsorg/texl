# Architecture

Texl is a Cargo workspace of four crates. Two implement the language and the
engine; the other two are bindings that make the engine available to
JavaScript and Python programs.

```text
              swan-core            the Swan language (no dependencies)
                  ▲
                  │
              texl-core            templates, documents, the engine  (+ url)
               ▲      ▲
               │      │
       texl-wasm      pytexl       hosts: JavaScript (WebAssembly), Python
```

| Crate       | Path         | Contents                                                     | Published as |
|-------------|--------------|--------------------------------------------------------------|--------------|
| `swan-core` | `swan-core/` | Lexer, parser, evaluator, values, built-in library, cycle collector | not yet |
| `texl-core` | `texl-core/` | Template renderer, `Engine` (globals, loaders, documents, imports), async helpers | not yet |
| `texl-wasm` | `texl-wasm/` | wasm-bindgen bindings: `TexlEngine`, `TexlContext`, `collect`, HTTP loader | `@onlabsorg/texl` on npm |
| `pytexl`    | `pytexl/`    | PyO3 native module `pytexl._pytexl` and the Python package `pytexl` | `pytexl` on PyPI |

`swan-core` knows nothing about templates, and `texl-core` knows nothing about
any host: both are plain, single-threaded Rust with no async runtime of their
own. The bindings add what is specific to a host: value conversion, an event
loop, loaders.

## From template text to rendering

```text
 source ──▶ template::render ──for each <% tag %>──▶ Lexer ──▶ Parser ──▶ eval ──▶ Value
                 │                                                                   │
                 │◀───────────── await if it is a Promise ◀──────────────────────────┘
                 │
                 └──▶ text form of the value, appended to the output
```

1. **`Engine::render(url, source)`** (`texl-core/src/engine.rs`) normalizes
   the URL, creates the import cache of this rendering, and renders the source
   as a document: it creates a document scope (`Engine::create_scope`), binds
   a `texl` namespace with an `import` function bound to the document's URL,
   and calls `template::render`.
2. **`template::render(template, &scope)`** (`texl-core/src/template.rs`)
   copies the text outside tags, and evaluates each tag in order in the same
   scope. When a tag's value is a promise, it awaits it. It then appends the
   text form of the value.
3. **The `Lexer`** (`swan-core/src/lexer.rs`) turns the tag's source into
   tokens, without copying it.
4. **The `Parser`** (`swan-core/src/parser.rs`), a Pratt parser, builds an
   `ast::Node`. Operator binding powers are in its `lbp` table; the precedence
   levels are documented in `swan-core/src/token.rs`. Syntax errors become
   error nodes, not failures.
5. **`evaluator::eval(&node, &scope)`** (`swan-core/src/evaluator.rs` and
   `evaluator/*_operation(s).rs`) returns a `Value`. It never fails: every
   error is a `Value::Undefined`.
6. When the rendering is done, the engine builds the document namespace
   `{uri, source, data, rendering}` (`data` is a snapshot of the document
   scope) and runs the cycle collector (`swan_core::gc::collect`).

An import (`texl.import(url)` in a tag) resolves the URL against the
importer, looks it up in the import cache, and if it is new starts a
background task that fetches the source through a loader and renders it as
another document, in its own scope. The tag gets a promise of the document.
The details, including cycle detection, are in
[Swan and Texl in depth](language.md#documents-imports-and-caching).

## Values and scopes

The runtime values are in `swan-core/src/value.rs` and `value/`:

| Type        | Role                                                         |
|-------------|--------------------------------------------------------------|
| `Value`     | Every Swan value: `Nothing`, `Bool`, `Numb`, `Text`, `List`, `Namespace`, `Func`, `Builtin`, `Promise`, `Undefined`, `Tuple` (and `Name`, internal to the evaluator) |
| `Namespace` | An immutable, copy-on-write name → value map                 |
| `Scope`     | A lexical environment: bindings plus a parent; a shared handle |
| `Promise`   | A single-assignment value that never rejects; a shared handle |
| `BuiltinFn` | A function implemented in Rust                               |
| `Undef`     | The description of an error, carried by `Value::Undefined`   |

Everything is `Rc`-based and single-threaded. See
[Swan and Texl in depth](language.md#scopes-and-names) for the scope layers.

## Where hosts plug in

| Extension point | API (Rust)                                   | JavaScript                  | Python                        |
|-----------------|----------------------------------------------|-----------------------------|-------------------------------|
| Globals         | `Engine::add_builtin(name, value)`           | `new TexlEngine(globals)`, `engine.set` | `Engine(globals)`, `engine.set` |
| Loaders         | `Engine::add_loader(scheme, loader)`, `add_default_loader` | built in: `http`, `https` | `engine.add_loader`, `set_default_loader`; built in: `file`, `http`, `https` |
| Async runtime   | the `SpawnFn` passed to `Engine::new`, or `helpers::run_pending` | `spawn_local` (built in) | the asyncio loop (built in)  |
| Built-ins       | `swan_core::builtins::set_builtin(name, value)` | —                        | —                             |
| Memory          | `swan_core::gc::collect()`                   | `collect()`                 | `pytexl.collect()`            |

- **Globals** are copied into the scope of every document and context created
  afterwards. A global can be any value; a host function is a `BuiltinFn` in
  Rust, a function in JavaScript and Python. A function that needs to wait
  returns a promise: `helpers::promise_async` turns a Rust future into one;
  the bindings turn JavaScript promises and Python awaitables into Swan
  promises.
- **Loaders** are async functions from a URL to the source text, chosen by
  the URL's scheme. The engine has none by default.
- **The spawner** is how the engine runs background tasks (imports, and the
  futures of `promise_async`). Without one, tasks wait in a thread-local queue
  that `helpers::run_pending` drains.
- **Built-ins** are shared by every scope of the thread. `set_builtin` adds or
  replaces one; `texl-wasm` uses it to replace `time.now`, since
  `wasm32-unknown-unknown` has no system clock.

## The bindings

### `texl-wasm`

- `src/lib.rs`: `TexlEngine` (wraps an `Engine` that spawns with
  `wasm_bindgen_futures::spawn_local`), `TexlContext`, `collect`. Its doc
  comments are copied into the generated `texl_wasm.d.ts`.
- `src/loader.rs`: the `http`/`https` loader, which uses the global `fetch`,
  and the resolution of URIs against the base URL (`location.href` by
  default). Every document has a full URL.
- `src/convert.rs`: JavaScript ↔ Swan value conversion. JavaScript functions
  become built-ins, JavaScript promises become Swan promises.

### `pytexl`

- `src/lib.rs`: the native classes (`unsendable`: they stay on their thread).
- `src/runtime.rs`: runs the engine's futures on the asyncio loop of the
  current thread. `spawn` stores a task in a thread-local executor and
  schedules a drain with `loop.call_soon`; wakers mark tasks ready again.
  `AwaitPy` awaits a Python awaitable from Rust.
- `src/convert.rs`: Python ↔ Swan value conversion.
- `python/pytexl/`: the Python package around the native module: `Engine`,
  `Context`, `Document`, and `loaders` (file and HTTP loaders, run in worker
  threads).

## Tests

| Command                                | Runs                                                   |
|----------------------------------------|--------------------------------------------------------|
| `cargo test`                           | the Rust unit tests, in `#[cfg(test)]` modules of each source file |
| `cargo test -p texl-wasm --test browser` | builds the package with wasm-pack and runs `texl-wasm/tests/web/index.html` in headless Chrome |
| `cargo test -p pytexl --test python`   | builds the native module and runs `pytexl/tests/test_*.py` |

The browser and Python tests both render the same sample document
(`tests/fixtures/sample.texl`, plus a host-specific section) and compare it
with `sample.expected.txt`.

## Releases

Two tag-triggered GitHub workflows publish the bindings; the tag version must
match the crate version.

- `.github/workflows/texl-wasm.yml`: on a `texl-wasm-v<version>` tag, runs the
  browser test, builds the package and publishes it to npm as
  `@onlabsorg/texl`.
- `.github/workflows/pytexl.yml`: on a `pytexl-v<version>` tag, builds abi3
  wheels for Linux, Windows and macOS and the sdist, tests them, and publishes
  them to PyPI.
