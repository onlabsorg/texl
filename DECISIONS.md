# Texl — Decision Log

*Design decisions of this repository, newest last. Each entry: date,
decision, reason. To revisit a decision, add a new entry that supersedes it;
do not edit old ones. Project-level decisions are in the OnLabs.org
coordination log (`DECISIONS.md` of the parent folder, OnLabs D22).*

## T1 — 2026-10-25 — The engine renders per document, no shared cache (was OnLabs D17)

The Texl engine holds configuration only (built-ins, globals, loaders).
Every rendering starts with a new scope and a new import cache, dropped at
the end. Hosts render with `render(url, source)` (a source they already
have, as the document at `url`), `fetch(url)` and `load(url)`. Within one
rendering, each document is loaded once; imported documents render in
their own scope, without the importer's names; import cycles, including
self-imports, give `Undefined`. Contexts remain, but cannot import.
Caching across renderings is the host's job, at loader level. Namespaces
become immutable, and the engine frees reference cycles (cycle collector)
after each rendering. `texl-wasm` 0.4.0 (`@onlabsorg/texl`) is on npm.
Breaking changes were accepted since Texl has no external users yet.
*Reason:* the engine-wide cache served stale imports with no way to
invalidate them, did not see the editor's unsaved buffer, could hold one
file under two keys, and on a server would share renderings between users.
The new model makes a rendering depend only on its source and the current
content of what it imports, the same in every host.

## T2 — 2026-11-01 — Undefined values know their source

Every undefined value records a byte offset (`pos`) and the `Source` it
refers to (URI, if any, and text). The evaluator keeps a thread-local
*current source*: `template::render` sets it around each tag and lexes the
tag with its offset in the template, so positions are offsets in the whole
document; a Swan function captures the current source where it is defined
and runs its body with it; a built-in runs with none (no position). The
bindings convert `pos` lazily, from the source the value carries, to
UTF-16 code units (JavaScript) and code points (Python). `pos` and `uri`
are absent (`null`/`None`) when unknown.
*Reason:* an offset without its source cannot be located or converted:
the earlier attempt (branch `undefined-debug`, reviewed 2026-10-31) gave
wrong or byte-unit positions for values reached through another tag,
through `data` or from an imported document. Capturing the source in
functions also places errors raised inside imported functions in the right
document. A source handle in every AST node was the alternative; the
thread-local plus one pointer per function and per undefined value gives
the same result without touching the parser's node types.

## T3 — 2026-11-01 — Undefined values describe themselves

`Undef::describe()` is a method (computed, not stored) that explains one
level in plain words, from one table of operation wordings; values are
quoted when short, otherwise named by type. `Undef::cause()` returns the
first undefined operand (or tuple item), so hosts walk to the root cause.
`Undef::members()` is the single list of details used by `undef.inspect`
and both bindings; `undef.inspect` gains `description` and `uri`. An
unexpected token is described by its source text. Unknown names still
evaluate to `Nothing`, not to an error.
*Reason:* a stored string would cost memory in every undefined value and
could go stale; one table keeps messages consistent; one member list keeps
Swan, Python and JavaScript in step.

## T4 — 2026-11-01 — Renderings and documents carry their tags

`template::render(source, scope)` returns a `Rendering {source, text,
data, tags}`; `source` is a `&str`/`String` (no URI) or a `Source`.
`Engine::render` builds the document `{uri, source, data, text, tags}` from
it: the member `rendering` is renamed `text`. Each tag is `{start, end,
out_start, out_end, value}`: byte spans in the source and the output, and
the value that was rendered (a top-level promise awaited, nested ones not,
as in the text). Every tag is listed, including tags that render nothing.
Imported documents have `tags` too, and keep all their tag values while
they live. Contexts return a rendering (`{text, data, tags}`, `data` a
snapshot of the whole context) in both bindings.
*Reason:* a host (the Stilo editor) must find each error even when its tag
renders nothing, and locate each value in the output without parsing it.
Keeping every value is simple and was preferred to keeping only errors;
the values are freed with their document.

## T5 — 2026-11-01 — Undefined values reach JavaScript as frozen plain objects

`texl-wasm` converts every undefined value (in tags, `data`, `context.get`,
function arguments) to a frozen plain object, `TexlUndefined`, with all
details already converted, instead of `undefined`. A module-private
`WeakMap` maps it back to the Swan value when it returns to Swan, and a
`FinalizationRegistry` frees that value when JavaScript collects the
object. `isUndefined(value)` tells them apart. This is a breaking change of
the JS API, accepted in this phase.
*Reason:* a wasm-bindgen class would hold wasm memory until `free()` (a
leak without `--weak-refs`), and identifying it by a public property ran
user code and could be faked. A plain object needs no `free()`, the
`WeakMap` lookup cannot be faked and runs no JS code, and the behaviour
matches `pytexl.Undefined`.

## T6 — 2026-11-03 — Swan expressions carry an opaque origin (supersedes part of T2)

swan-core no longer knows about documents, URIs or offsets in a larger
text. The swan-core `Source` becomes `Expression {text, origin}` (module
`expression`): `text` is the code that was evaluated (for Texl, one tag's
expression) and `origin` an optional `Rc<dyn Origin>`, where `Origin` is a
trait with `Any + Display` as supertraits and a blanket impl for every
such type. Swan only displays an origin; the host defines its type and
gets it back by downcasting. Origins must not hold Swan values, since the
cycle collector cannot trace them. `pos` in an undefined value is a byte
offset in its `Expression.text`, so it is relative to the expression. The
thread-local current expression (T2's current source), functions capturing
theirs and built-ins running with none are kept from T2; `parse` evaluates
its text as a new expression with no origin. `Lexer::with_offset`,
`Source.uri` and the UTF-16/code-point offset helpers leave swan-core.
texl-core evaluates each tag as its own `Expression`, with an origin type
of its own holding the `Document` (URI and the text that was rendered; the
former `Source` of `Rendering`) and the tag's start; it displays as
`<uri>::<start>`. texl-core maps an origin to its document and tag, and
computes the absolute position (start + `pos`) and the bindings' units
from the document text. Swan's `undef.inspect` gives `pos` relative to the
expression and `origin` as text; texl-core replaces it (`set_builtin`)
with a version that also gives `uri` and the absolute `pos`, so templates
see the same members as before. Host APIs (`pos`, `uri` on `TexlUndefined`
and `pytexl.Undefined`) do not change. The Tag of a rendering keeps
telling where a value is shown; the origin tells where it was made.
*Reason:* swan-core had grown features that only serve texl-core (URIs,
offsets into a whole template). An opaque origin, like Lua's chunk name or
Python's filename, keeps the language independent of its hosts. The tag
that shows a value cannot replace it: a value can be made in another tag,
another document, a function body or `parse`, and reach the host through
`data`. An `Rc` origin, rather than a string or number identifier, keeps
the exact text that was rendered alive as long as a value refers to it: an
identifier would need an engine-wide table (against T1) and would point
into the wrong version of a document re-rendered after an edit. A
`members` hook on `Origin` was considered to avoid the override and not
adopted, to keep the trait minimal.
