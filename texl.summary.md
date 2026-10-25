# Texl — summary

*What this repository is, how it is built and where it stands, in plain
words. The OnLabs.org coordination folder reads this file instead of the
code; keep it current as described in `CLAUDE.md`.*

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

- **Documents addressed by URL.** Every template is rendered as a document
  with a location (a file path, an `http(s)://` URL, or any scheme the host
  registers a loader for). The host can load a document from its location, or
  render text it already has (such as the content of an editor) as the
  document at a given location. `texl.import(uri)` loads, renders and returns
  another document as `{uri, source, data, rendering}`: its source text, the
  names it defined, and its rendered output. Documents can thus act as
  *partials* (reusable text), *libraries* (reusable functions) or *data
  sources*. Relative imports resolve against the importing document's
  location, so a set of documents forms a linked web of content.
- **Self-contained documents.** An imported document renders on its own: it
  sees the built-in functions and the data the host provides, but not the
  names of the document that imports it, so it renders the same whoever
  imports it. A document that imports itself, directly or through others,
  gets an `Undefined` value instead of waiting forever.
- **Swan, a tolerant expression language.** Swan is small, dynamically typed
  and expression-only: no statements, no exceptions. Every error, including a
  syntax error, becomes an `Undefined` value that flows on like any other
  value, so a broken expression never stops the rendering of a document.
  Values are numbers, text, lists, namespaces, functions, tuples and promises;
  operators apply item-by-item over tuples. All values except promises are
  immutable: a namespace (a set of named values) cannot change once it is
  created, so a document's data cannot be changed by the documents that use
  it.
- **Asynchronous by design.** Loading a document or fetching data may take
  time. Swan has a built-in `Promise` value (chained with `~>`) and rendering
  waits for promises before continuing. Within one rendering, each document
  is loaded and rendered once, and concurrent imports of it share that load.
  Nothing is kept between renderings: the next rendering loads its documents
  again, so it always sees their current content. A host that wants to cache
  documents does it in its loaders.
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
          │ evaluator,   │            │ loaders, imports │
          │ builtins     │            │ (per rendering)  │
          └──────────────┘            └────────┬─────────┘
                                               │
              ┌──────────────┬─────────────────┼───────────────┐
              ▼              ▼                 ▼               ▼
         texl-cli       texl-wasm           pytexl         Rust API
       (`texl` binary)  (browser, npm)     (Python, PyPI)  (crates)
```

| Component   | Role                                                                 | Distribution |
|-------------|----------------------------------------------------------------------|--------------|
| `swan-core` | The Swan language: lexer, Pratt parser, evaluator, standard library (`bool`, `numb`, `text`, `list`, `nspace`, `promise`, `undef`, `math`, `time`), and a memory cleaner (cycle collector). No dependencies. | Rust crate |
| `texl-core` | Template rendering and the `Engine`: pluggable loaders per URL scheme, and `fetch` (get a document's source), `render` (render a source as the document at a location) and `load` (both), with `texl.import` and an import cache per rendering. | Rust crate |
| `texl-cli`  | The `texl` command: renders a file to stdout or a file, loading imports from the local file system. | Prebuilt binaries for Linux, macOS, Windows (GitHub Releases) |
| `texl-wasm` | JavaScript library for the browser via WebAssembly: `TexlEngine` (`fetch`, `load`, `render`, `set`, `createContext`) with HTTP loading, and `collect` (the memory cleaner); JS functions and promises cross into Swan and back. | npm: `@onlabsorg/texl` |
| `pytexl`    | Python library (PyO3): async `Engine` (`fetch`, `load`, `render`), `Context`, `collect` (the memory cleaner), file and HTTP loaders; Python callables, mappings and awaitables cross into Swan and back. | PyPI: `pytexl` |

Hosts can inject their own values and functions into the template scope, and
register loaders for their own URL schemes. A host application can thus give
templates access to its data and services, and decide where documents come
from. A *context* (a scope that keeps its names across several renders) is
available to hosts too, but it cannot import documents.

## Status

- MIT licensed; source at <https://github.com/onlabsorg/texl>.
- Latest releases (2026-10-24): `texl-cli` 0.1.1, `texl-wasm` 0.3.0,
  `pytexl` 0.3.0.
- The repository holds the next, not yet released versions (`swan-core`
  0.2.0, `texl-core` 0.3.0, `texl-cli` 0.2.0, `texl-wasm` and `pytexl`
  0.4.0). They change how hosts use the engine: rendering text now needs a
  location, documents are no longer cached between renderings, imported
  documents no longer see the names of their importer, namespaces are
  immutable and no longer inherit names from where they were created,
  names a document binds with `bind` are now part of its data, and memory
  is now cleaned up after each rendering (see "Memory" below).
- Available and published: CLI, browser/JavaScript, Python, Rust.
- Documentation site (template manual, Swan manual with built-in reference,
  host guides, rustdoc): <https://onlabsorg.github.io/texl/>.
- Planned next: a Node.js library, editor support (`texl.nvim`,
  `vscode-texl`), and Swan language extensions (operator overloading on
  namespaces via `__add__`-style members, a recursion depth limit, a `filter`
  built-in).

## Memory

A Swan function remembers the place where it was defined, so a document
that defines functions holds on to itself: plain reference counting would
never free it. Swan therefore has a memory cleaner (a cycle collector, the
technique Python uses) that finds such self-held documents once nothing else
uses them, and frees them. The engine runs it after every rendering, so
programs that keep rendering for a long time, such as an editor or a server,
no longer grow in memory with each rendering. Hosts that render into a *context* call `collect` themselves after
dropping it; it is available in Rust, JavaScript and Python. A developer
example (`texl-cli/examples/leak.rs`) measures what each rendering leaves
behind.

## Known limitations

- The memory cleaner only sees Swan's own memory. Swan values kept inside
  a host's own functions, and loops of references that pass through both
  Swan and JavaScript or Python objects, are not cleaned up.
