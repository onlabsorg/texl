# Texl — summary

*What this repository is, how it is built and where it stands, in plain
words. The OnLabs.org coordination folder reads this file instead of the
code; keep it current as described in `CLAUDE.md`.*

Checked against: 000a5df (2026-10-17)

## In one sentence

Texl is a template engine for any kind of text, with an embedded expression
language (Swan), that turns documents into small programs which compute their
own content, import other documents by URL, and run the same way on the
command line, in the browser, in Python and in Rust.

## What it does

A Texl template is ordinary text (Markdown, HTML, a configuration file, a
letter) with expressions between `<%` and `%>`. Rendering replaces each
expression with its value:

```texl
<% user = {name: "Ada", tasks: ["review", "deploy"]} %>
Hi <% user.name %>,
you have <% list.size user.tasks %> open tasks: <% list.enum user.tasks %>.
```

```text
Hi Ada,
you have 2 open tasks: review, deploy.
```

The expressions are evaluated in order against one shared scope, so names
defined in one tag are visible in the next. A template is therefore both a
document and a little program that produces it.

## Key ideas

- **Documents addressed by URL.** A template can be loaded from a location (a
  file path, an `http(s)://` URL, or any scheme the host registers a loader
  for). `texl.import(uri)` loads, renders and returns another document as
  `{uri, source, data, rendering}`: its source text, the names it defined, and
  its rendered output. Documents can thus act as *partials* (reusable text),
  *libraries* (reusable functions) or *data sources*. Relative imports resolve
  against the importing document's location, so a set of documents forms a
  linked web of content.
- **Swan, a tolerant expression language.** Swan is small, dynamically typed
  and expression-only: no statements, no exceptions. Every error, including a
  syntax error, becomes an `Undefined` value that flows on like any other
  value, so a broken expression never stops the rendering of a document.
  Values are numbers, text, lists, namespaces, functions, tuples and promises;
  operators apply item-by-item over tuples.
- **Asynchronous by design.** Loading a document or fetching data may take
  time. Swan has a built-in `Promise` value (chained with `~>`) and rendering
  waits for promises before continuing. Concurrent imports of the same
  document share one load, and each document is loaded and rendered once per
  engine (cached by normalized URL).
- **Host-agnostic core.** The engine is written in Rust with no runtime
  assumptions. The host plugs in how tasks are spawned, how URLs are loaded,
  and which extra built-ins (functions, data) are available to templates. This
  is what lets the same engine run in very different environments.

## Architecture

```
            Swan language                Template engine
          ┌──────────────┐            ┌──────────────────┐
          │  swan-core   │ ─────────▶ │    texl-core     │
          │ lexer/parser │            │ renderer, Engine │
          │ evaluator,   │            │ loaders, cache,  │
          │ builtins     │            │ texl.import      │
          └──────────────┘            └────────┬─────────┘
                                               │
              ┌──────────────┬─────────────────┼───────────────┐
              ▼              ▼                 ▼               ▼
         texl-cli       texl-wasm           pytexl         Rust API
       (`texl` binary)  (browser, npm)     (Python, PyPI)  (crates)
```

| Component   | Role                                                                 | Distribution |
|-------------|----------------------------------------------------------------------|--------------|
| `swan-core` | The Swan language: lexer, Pratt parser, evaluator, standard library (`bool`, `numb`, `text`, `list`, `nspace`, `promise`, `undef`, `math`, `time`). No dependencies. | Rust crate |
| `texl-core` | Template rendering and the `Engine`: scope setup, pluggable loaders per URL scheme, document cache, `texl.import`. | Rust crate |
| `texl-cli`  | The `texl` command: renders a file to stdout or a file, loading imports from the local file system. | Prebuilt binaries for Linux, macOS, Windows (GitHub Releases) |
| `texl-wasm` | JavaScript library for the browser via WebAssembly: `TexlEngine` (`load`, `render`, `set`, `createContext`) with `fetch`-based loading; JS functions and promises cross into Swan and back. | npm: `@onlabsorg/texl` |
| `pytexl`    | Python library (PyO3): async `Engine`, `Context`, file and HTTP loaders; Python callables, mappings and awaitables cross into Swan and back. | PyPI: `pytexl` |

Hosts can inject their own values and functions into the template scope, and
register loaders for their own URL schemes. A host application can thus give
templates access to its data and services, and decide where documents come
from.

## Status

- Version 0.1–0.2; MIT licensed; source at <https://github.com/onlabsorg/texl>.
- Available and published: CLI, browser/JavaScript, Python, Rust.
- Documentation site (template manual, Swan manual with built-in reference,
  host guides, rustdoc): <https://onlabsorg.github.io/texl/>.
- Planned next: a Node.js library, editor support (`texl.nvim`,
  `vscode-texl`), and Swan language extensions (operator overloading on
  namespaces via `__add__`-style members, a recursion depth limit, a `filter`
  built-in).
