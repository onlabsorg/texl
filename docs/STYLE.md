# Documentation style guide

This guide keeps Texl's developer documentation consistent. It applies to the
Markdown in `docs/`, the crate READMEs, the Rust doc comments and the Python
docstrings. The user manual of the template language and of Swan is not in
this repository (see the [README](../README.md)).

## Where things live

| What                                   | Where                                   |
|----------------------------------------|-----------------------------------------|
| Index of the developer docs            | `docs/README.md`                        |
| Host guides, JavaScript and Python API | `docs/api/`                             |
| Architecture, the language in depth    | `docs/architecture.md`, `docs/language.md` |
| Tutorials                              | `docs/examples/`                        |
| Rust API (`swan-core`, `texl-core`)    | doc comments in the code (rustdoc)      |
| JavaScript API typings                 | doc comments in `texl-wasm/src/lib.rs` (copied into `texl_wasm.d.ts`) |
| Python API                             | `docs/api/python.md`, and the docstrings of `pytexl/python/pytexl` |

The docs are plain Markdown, read on GitHub. There is no build step.

## Audience and narrative style

- The reader is a programmer who embeds Texl, writes a host, or works on Texl
  itself. Assume they know the template language; link to
  [language.md](language.md) for how it works inside.
- Address the reader as "you". Use the present tense and the active voice.
- Prefer short sentences and short paragraphs. One idea per paragraph.
- Show an example first, then explain it.
- Describe what the software does today. Describe planned features only as
  "coming soon", and never in an API reference.
- When the behavior is surprising, say so plainly and show the fix.
- Use American spelling. Wrap Markdown at about 80 characters.

## Links

- Link between the docs with relative paths (`../language.md#memory-management`),
  so they work on GitHub and in a clone.
- Do not link into the OnLabs.org website from `docs/` or from doc comments;
  only the README files (the root one and the crate READMEs) link the user
  manual.
- The `texl-wasm` and `pytexl` READMEs are also the npm and PyPI package
  READMEs: their links are absolute URLs.
- Rust items are named in code font (`Engine::render`); the reader finds them
  with `cargo doc`.

## Terms

| Use                | For                                                              |
|--------------------|------------------------------------------------------------------|
| template           | text with `<% %>` tags                                           |
| tag                | one `<% ... %>`                                                  |
| document           | a template rendered at a URL by an engine                        |
| engine             | the object that renders templates and loads documents           |
| host               | the program that runs the engine: a web page, a Python or Rust program |
| global             | a name that the host binds for every template                    |
| loader             | a function that returns the source of a document from its URL    |
| context            | a scope that keeps its names across renders, without imports     |
| `Nothing`, `Undefined`, `Numb`, `Text`, ... | Swan types, always in code font         |
| name               | a Swan identifier (not "variable", except in passing)            |

Write `TRUE` and `FALSE` in code font. Write "Swan" and "Texl" capitalized in
prose, and `texl` in code font for the `texl` namespace.

## Code examples

- Swan code goes in ```` ```swan ```` blocks, template code in ```` ```texl ````
  blocks, output in ```` ```text ```` blocks, and host code in blocks of its
  language (`js`, `html`, `python`, `rust`, `sh`).
- A ```` ```texl ```` block immediately followed by a ```` ```text ```` block
  shows a template and its exact rendering.
- In a Swan block, a line can end with `# => result`, the value of that line.
  Results use Swan notation: `42`, `"text"`, `TRUE`, `()`, `(1, 2)`, `[1, 2]`,
  `{a: 1, b: 2}` (members sorted by name).
- Host code shows results in a trailing comment (`// 42`, `# 42`), or in a
  ```` ```text ```` block for what a program prints.
- Tutorials in `docs/examples/` are complete programs that run as shown.
- Keep examples small. Every result, rendering and printed output must be the
  real output of the current implementation: run the example before you
  commit it.

## API references

Every API entry, in every language, has the same shape:

1. A heading with the name, in code font.
2. A signature block.
3. A one-sentence summary that starts with a verb in the third person
   ("Returns ...", "Creates ...", "Binds ...").
4. Details: arguments, return value, special cases, errors.
5. An example.

Signature notation:

| API                 | Notation                                     | Example                                        |
|---------------------|----------------------------------------------|------------------------------------------------|
| JavaScript          | TypeScript                                   | `load(uri: string): Promise<TexlDocument \| null>` |
| Python              | Python, with type hints                      | `async def load(uri: str) -> Document \| None` |
| Rust                | Rust (rustdoc shows it)                      |                                                |

Rust doc comments follow the rustdoc conventions: a one-line summary, then
details, then `# Examples`, `# Errors` and `# Panics` sections as needed.
Public items of `swan-core` and `texl-core` must be documented
(`#![warn(missing_docs)]`).

Python docstrings follow the Google style: a one-line summary, then details,
then `Args:`, `Returns:`, `Raises:` and `Example:` sections as needed. Every
public module, class, method and function of `pytexl` has one. The docstrings
of the native classes are the doc comments of `pytexl/src`.
