# Documentation style guide

This guide keeps the Texl documentation consistent. It applies to the manual
(`docs/src`), the landing page, the crate READMEs and the doc comments in the
code.

## Where things live

| What                                   | Where                                   |
|----------------------------------------|-----------------------------------------|
| Landing page                           | `docs/site/index.html`                  |
| Template manual, Swan manual, host guides, JavaScript and Python API | `docs/src` (an mdBook) |
| Rust API (`swan-core`, `texl-core`)    | doc comments in the code (rustdoc)      |
| JavaScript API typings                 | doc comments in `texl-wasm/src/lib.rs` (copied into `texl_wasm.d.ts`) |
| Python API                             | `docs/src/hosts/python.md`, and the docstrings of `pytexl/python/pytexl` |
| Shared look and feel                   | `docs/assets`                           |

`docs/build.sh` builds everything into `target/site`, the folder that GitHub
Pages publishes.

## Narrative style

- Write for the reader of that page. The template manual speaks to people who
  write templates, not to Rust programmers.
- Address the reader as "you". Use the present tense and the active voice.
- Prefer short sentences and short paragraphs. One idea per paragraph.
- Show an example first, then explain it.
- Describe what the software does today. Describe planned features only as
  "coming soon", and never in an API reference.
- When the behavior is surprising, say so plainly and show the fix.
- Use American spelling.

## Terms

| Use                | For                                                              |
|--------------------|------------------------------------------------------------------|
| template           | text with `<% %>` tags                                           |
| tag                | one `<% ... %>`                                                  |
| document           | a template identified by a URI, loaded by an engine              |
| engine             | the object that renders templates and loads documents           |
| host               | the program that runs the engine: a web page, a Python or Rust program |
| global             | a name that the host binds for every template                    |
| `Nothing`, `Undefined`, `Numb`, `Text`, ... | Swan types, always in code font         |
| name               | a Swan identifier (not "variable", except in passing)            |

Write `TRUE` and `FALSE` in code font. Write "Swan" and "Texl" capitalized in
prose, and `texl` in code font for the command and the `texl` namespace.

## Code examples

- Swan code goes in ```` ```swan ```` blocks, template code in ```` ```texl ````
  blocks, and output in ```` ```text ```` blocks.
- In a Swan block, the lines are evaluated in order, in one fresh scope. A line
  can end with `# => result`, the value of that line. A line that starts with
  spaces continues the previous line.
- A result can be followed by a short explanation after a comma:
  `# => 7, because application binds tighter than +`.
- Results use Swan notation: `42`, `"text"`, `TRUE`, `()`, `(1, 2)`, `[1, 2]`,
  `{a: 1, b: 2}` (members sorted by name). Functions are written `Func`,
  undefined values `Undefined`, and promises `Promise(value)`.
- A ```` ```texl ```` block immediately followed by a ```` ```text ```` block
  shows a template and its exact rendering.
- Keep examples small and runnable. Every `# =>` result and every template
  rendering must be the real output of the current implementation.

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
| Swan built-ins      | `name arg1 arg2 -> Type`                     | `text.find needle haystack -> Numb`            |
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

## Look and feel

- All pages share the design tokens in `docs/assets/texl.css`: colors, fonts,
  spacing and code colors, in a light and a dark variant.
- `book.css` maps the tokens onto mdBook, and `rustdoc.css` onto rustdoc.
- `texl-highlight.js` highlights Swan and Texl code on every page.
- Do not add colors or fonts in a page; add a token instead.
