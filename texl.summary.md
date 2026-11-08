# Texl — summary

*What this repository is, how it is built and where it stands, in plain
words. The OnLabs.org coordination folder reads this file instead of the
code; keep it current as described in `CLAUDE.md`.*

## In one sentence

Texl is a template engine for any kind of text, with an embedded expression
language (Swan), that turns documents into small programs which compute their
own content, import other documents by URL, and run the same way in the
browser, in Python and in Rust.

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
  another document as `{uri, source, data, text, tags}`: its source text, the
  names it defined, its rendered output, and a description of each of its
  expressions (see "Finding errors"). Documents can thus act as
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
- **Finding errors.** Because errors do not stop the rendering, a rendered
  document also lists every expression it contains, in order, including
  those that output nothing: where it is in the source, where its output is
  in the rendered text, and the value it produced. An `Undefined` value
  explains itself in plain words (for example "cannot subtract the number 1
  from the text "a""), points to the value that caused it, and records the
  document and the position where it was made, also when the error happens
  inside a function imported from another document. A host program can also
  read that document's source from the error, to show the code around it. An
  editor can thus highlight each broken expression and say what is wrong with
  it. The
  default output, `[[Undefined]]`, is unchanged.
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
                              ┌────────────────┼───────────────┐
                              ▼                ▼               ▼
                          texl-wasm         pytexl         Rust API
                        (browser, npm)   (Python, PyPI)    (crates)
```

| Component   | Role                                                                 | Distribution |
|-------------|----------------------------------------------------------------------|--------------|
| `swan-core` | The Swan language: lexer, Pratt parser, evaluator, standard library (`bool`, `numb`, `text`, `list`, `nspace`, `promise`, `undef`, `math`, `time`), and a memory cleaner (cycle collector). No dependencies. | Rust crate |
| `texl-core` | Template rendering and the `Engine`: pluggable loaders per URL scheme, and `fetch` (get a document's source), `render` (render a source as the document at a location) and `load` (both), with `texl.import` and an import cache per rendering. | Rust crate |
| `texl-wasm` | JavaScript library for the browser via WebAssembly: `TexlEngine` (`fetch`, `load`, `render`, `set`, `addLoader`, `setDefaultLoader`, `createContext`) with HTTP loading and loaders of its own, `collect` (the memory cleaner), `isUndefined`, `isSwanFunction`, `normalize` and `Tuple`; JS functions and promises cross into Swan and back, the functions a template defines can be called from JavaScript, lists and tuples stay distinct, and errors arrive as objects with their details. | npm: `@onlabsorg/texl` |
| `pytexl`    | Python library (PyO3): async `Engine` (`fetch`, `load`, `render`), `Context`, `collect` (the memory cleaner), `normalize`, file and HTTP loaders; Python callables, mappings and awaitables cross into Swan and back, and errors arrive as `Undefined` objects with their details. | PyPI: `pytexl` |

Hosts can inject their own values and functions into the template scope, and
register loaders for their own URL schemes. A host application can thus give
templates access to its data and services, and decide where documents come
from.

The Swan language knows nothing about templates or documents: it evaluates
single expressions, and an error records its position within the expression
that made it, plus a label of where that expression came from, which the
program using Swan sets and reads back. The template engine uses that label
to tell which document and which expression an error belongs to, and turns
the position into one in the whole document. A *context* (a scope that keeps its names across several renders) is
available to hosts too, but it cannot import documents.

## Status

- MIT licensed; source at <https://github.com/onlabsorg/texl>.
- Latest releases (2026-11-08): `texl-wasm` 0.7.1, `pytexl` 0.7.0. The Rust
  crates (`swan-core` 0.5.0, `texl-core` 0.6.0) are used
  from the repository; they are not on crates.io.
- Version 0.4.0 changed how hosts use the engine: rendering text now needs a
  location, documents are no longer cached between renderings, imported
  documents no longer see the names of their importer, namespaces are
  immutable and no longer inherit names from where they were created,
  names a document binds with `bind` are now part of its data, and memory
  is now cleaned up after each rendering (see "Memory" below).
- Version 0.5.0 (2026-11-03) brought the error details and expression list
  described in "Finding errors". They change the host interface: a document's rendered output is now called `text` instead of
  `rendering`, rendering in a context returns the text together with the
  context's names and the expression list, errors reach JavaScript as
  objects with their details instead of `undefined`, and error positions
  count from the start of the document and use each host's own string
  indices. Since 2026-11-03, the Swan language no longer knows about
  documents (see "Architecture"); for templates and hosts nothing changes,
  except that inspecting an error in code built at run time with `parse`
  gives its position within that code, with no document address. Also
  since 2026-11-03, an expression followed by something that does not
  belong to it (a stray `)` or an unknown symbol, as in `2 + 1e3 ) 1`) is
  an error instead of silently ignoring the rest; such a tag used to show a
  value, and now shows `[[Undefined]]` and sets no names. A bracket closed
  by the wrong symbol is reported as that symbol, not as a missing bracket.
  Errors of built-in functions (such as `math.sqrt` of a negative number or
  a failed `texl.import`) and of the functions a host adds now point to the
  call that produced them, where before they had no position. Rust
  programs that add their own functions write their errors slightly
  differently (one argument less). An error that passes through a promise
  (a value that arrives later) is now kept and shown as `[[Undefined]]`
  with its details; before, it was silently replaced by "nothing", so such
  a tag rendered empty.
  Errors of functions that wait for their result now point to their call
  too, and so do errors of combined functions (`f << g`) and of built-in
  functions that a Python function calls. A built-in function called
  directly from Python gives an error with no position. A document handed
  back to the engine by JavaScript or Python keeps correct expression
  positions; they used to go wrong in text with non-English characters.
- Version 0.6.0 (2026-11-04) changes the host interface again. An error also
  gives JavaScript and Python programs the source text it was made in, so they can
  show the code around it. JavaScript programs now tell a tuple from a list:
  a tuple arrives as a `Tuple` (an array with its own class), and passing one
  back gives a tuple, where it used to become a list. Lists now follow the
  same rule whichever program builds them: a list holds no tuples and no
  empty values. This changes what hosts see: an empty value (`null`, `None`)
  in a list passed to a template is removed and the later items move down one
  position, and a tuple in such a list is spread into it. JavaScript and Python
  also get a `normalize` function that does this without going through a
  template. A JavaScript or Python function that fails while a template calls
  it now gives an `Undefined` value holding the host's error message, at the
  position of the call, where before the result was silently "nothing"; the
  same holds for functions that wait for their result and for rejected or
  cancelled promises passed to a template. Such a tag now shows
  `[[Undefined]]` instead of rendering empty.
  A document that cannot be loaded now says why. An import of it in a template
  gives an `Undefined` value holding the document's location and the reason,
  so the tag shows `[[Undefined]]` instead of rendering empty, and a fallback
  must now be written with `|` instead of `;`. A host program that loads or
  fetches such a document gets an error in its own language: an exception in
  Python, a rejected promise in JavaScript, an error result in Rust; before,
  it got an empty result. A location that is not valid now gives a JavaScript
  program a proper `Error` instead of a text. An `Undefined` value made
  deliberately with `undef.from` now describes itself like any other, as
  "Custom undefined value: 5" instead of "undefined on purpose".
- Version 0.7.0 (2026-11-08) changes the host interface for JavaScript: the
  functions a template defines, and the built-in functions, now reach
  JavaScript programs as
  ordinary functions they can call, as Python programs already could; before,
  they arrived as `undefined`. Such a function can be handed back to the
  engine and stays the same Swan function, and `isSwanFunction` tells it from
  the program's own functions. Calls now read the same in both directions: a
  JavaScript function called from a template with several values receives
  them as separate arguments, where before it received them as one `Tuple`;
  existing JavaScript functions written for one `Tuple` argument must change.
  A JavaScript or Python function given to the engine now comes back to its
  program as the very same function; Python used to get a wrapper.
  A namespace can now choose how it is shown as text: if it has a
  `__text__` member (a function), that function receives the namespace and
  the text it returns is what a tag shows and what `text.from` gives. A
  function that returns data, such as a table, can thus be labelled and shown
  in place in one tag (`<% t: table(...) %>`) while its data stays usable. If
  the member returns anything but text, the result is an `Undefined` value
  (`[[Undefined]]` in the output). Rendering and `text.from` now share one
  conversion, so a value shows the same text in both. Names that start and
  end with two underscores are reserved for such special members; the
  language does not enforce it.
- `texl-wasm` 0.7.1 (2026-11-08): JavaScript programs can register
  their own loaders for a URL scheme, as Python and Rust programs already
  could: a function that receives a document's location and returns its
  source. It can also replace the built-in web loader, for example to limit
  what a template may read. A loader for locations without a scheme can be
  set too, as in Python, but the engine does not produce such locations yet.
- Available: browser/JavaScript, Python, Rust.
- The `texl` command-line tool (`texl-cli`) has been removed from the
  repository; its last release was 0.2.0 (2026-10-25), and its binaries
  remain on GitHub Releases. The documentation examples that used it now use
  the Python library.
- Documentation: the user manual (writing templates, the Swan language, the
  built-in reference) is no longer in this repository; it lives on the
  OnLabs.org site, <https://www.onlabs.org/manual/>, and the README files
  link to it. The repository keeps developer documentation only, as plain
  pages in `docs/` read on GitHub: getting started, the JavaScript, Python
  and Rust guides, an overview of how Texl is built, an in-depth description
  of the language for advanced users (design, async rendering, memory
  cleanup, what a template can and cannot reach, limits) and worked
  examples. The former documentation site and its build are gone.
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
no longer grow in memory with each rendering. Hosts that render into a
*context* call `collect` themselves after dropping it; it is available in
Rust, JavaScript and Python. A Swan function held by a JavaScript or Python
program keeps its document alive while the program holds it; once the
program's own memory manager drops it, the next cleanup frees the document.
The browser tests check this. The developer example that measured what each
rendering leaves behind was part of the command-line tool and was removed
with it.

## Known limitations

- An error raised inside a built-in function points to the call of that
  function, not to anything more precise, and errors a built-in function puts
  inside a list or namespace have no position. A misspelled name gives
  "nothing", not an error.
- The memory cleaner only sees Swan's own memory. Swan values kept inside
  a host's own functions, and loops of references that pass through both
  Swan and JavaScript or Python objects, are not cleaned up. The JavaScript and
  Python guides describe the common case (a host function that refers to its
  own context) and how to avoid it.
