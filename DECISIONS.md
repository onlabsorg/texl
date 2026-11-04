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

## T7 — 2026-11-03 — An expression must use up its whole source

`Parser::parse` checks that the expression ends where the source ends. When
tokens are left over (a stray `)`, `]` or `}`, an unknown character), the
whole expression becomes the first syntax error found in the source, as a
`Node::UnexpectedToken` (`Undef::Token`, "unexpected `)`"); the part parsed
so far is dropped. A group closed by the wrong token, as in `(1 @ 2)`, is an
unexpected token at that token, no longer "missing closing `)`";
`Node::Unclosed` is kept for a source that ends before the group is closed.
*Reason:* the parser stopped at the first token that could not continue the
expression and silently ignored the rest, so `2 + 1e3 ) 1` gave 1002 and in
`<% x = 1 ) y = 2 %>` the assignment of `y` never happened, with no error.
A syntax error must not produce a value (T3: errors explain themselves
where they occur). Reporting the first error, rather than the left-over
token, points at the cause: in `(1 ] 2)` the left-over is the final `)`,
but the mistake is the `]`.

## T8 — 2026-11-03 — Built-in errors take the position of their call (supersedes part of T2)

The position of an undefined value is an `Option<usize>`: `None` means "no
position", and `Some(0)` is a real position at the start of the
expression, no longer told apart by the absence of an expression. Built-in
and host functions still run with no current expression and make their
errors with no position (`Value::undef_unary(op, val)`,
`Value::undef_binary(op, lhv, rhv)`; the evaluator's operations use
`undef_unary_at`/`undef_binary_at`). When a built-in returns, the evaluator
gives every undefined value it returns with no position (the value itself,
or the items of a tuple) the position of the call (the argument, as for
"cannot apply" errors, or the `=>` of `x => f`) and the current
expression. A value that already has a position, such as an undefined
argument passed on, keeps it. A callback chained with `~>` captures the
expression of the `~>` and runs with it, so a late call is placed in the
right expression. Values inside a list or namespace a built-in returns are
not placed.
*Reason:* errors of the standard library, of `texl.import` and of host
functions had no position, so a host could not show where they happened.
A built-in cannot know where it is called from, but the evaluator can, so
placing the value at the call covers every built-in and host function in
one place, without changing them. "Position 0 and no expression" as a
marker of "no position" was ambiguous when a host evaluates code without an
expression: a value genuinely made at offset 0 and passed on by a built-in
would have been moved. An explicit `None` removes the ambiguity. Passing
the call position to every built-in, or wrapping built-in errors in a new
"call" error, were considered: the first changes every built-in and host
function for the same result, the second adds a level that repeats the
function name already in `op`.

## T9 — 2026-11-03 — Promises keep undefined values

A promise resolved with an `Undefined` value holds it, like any other
value; it is no longer replaced with `Nothing`. Promises still never
reject: there is no failure state. So `x ~> f` behaves as a later `f x`
(an undefined `x` reaches `f`), a `~>` whose function fails resolves to
the error, with its description and position, and a tag that waits for
such a promise renders `[[Undefined]]`. The JavaScript and Python
bindings resolve their promises to the `TexlUndefined` /
`pytexl.Undefined` object. A promise whose work cannot run (a failed
import, a rejected JavaScript promise, a Python coroutine that raises)
still resolves to `Nothing`: there is no Swan error to keep.
*Reason:* the replacement was justified as "a promise never rejects", but
an undefined value is not a rejection: in Swan errors are values that flow
on and are shown where they end up. Replacing it hid every error that went
through a promise (a tag rendered empty instead of `[[Undefined]]`), made
`x ~> f` differ from `f x`, and threw away the call position given to
errors since T8. No part of the engine relies on it: a pending promise is
told apart by having no value, not by an undefined one.

## T10 — 2026-11-03 — Positions through promises, host calls and round trips

Three rules complete T8. (1) A pending promise a built-in returns gives the
position of the call to the position-less undefined values it resolves to.
The position is stored on the promise (`Promise::place_undefined`), and the
first call that returns it decides. (2) A built-in or host that calls a Swan
value from Rust uses `evaluator::apply`, which adds no position, so its
errors are placed at the outer call; composed functions and the Python
`Function` use it. A position that cannot be located is `null` / `None` in
the bindings, never `0`. (3) The bindings convert a document's tag positions
back to bytes when an object or mapping with the members of a document comes
back from the host, so Swan always holds bytes and a document round-trips.
*Reason:* since T9 errors travel through promises, and without (1) those of
async host functions had no location. A child promise that places the value
was tried first and rejected: it breaks the guarantee that imports of one
document return the same promise. Without (2), `eval_apply(.., 0)` outside
an expression fixed position `0`, which T8 never moves, so these errors
could not reach their call. Without (3), a document passed back was
converted twice and its positions were wrong in non-ASCII text. Marking
converted documents was not chosen: the marker would have to survive plain
JavaScript objects and Python dicts. A host object that has exactly the five
members of a document is treated as one, in both directions.

## T11 — 2026-11-04 — Undefined values show hosts their source

The `TexlUndefined` object and `pytexl.Undefined` have a `source` member: the
text `pos` is an index of, which is the whole document for a value made by a
tag and the code itself for code run by `parse`. So `source[pos]` is where the
error is, in the host's string units, and it is `null` / `None` exactly when
`pos` is. `undef.inspect` in Swan and `Undefined.inspect()` in Python do not
list it; Rust hosts already have it as the `text` of `document::locate`.
*Reason:* a host could not show the code around an error unless it kept every
source itself, and could not do so at all for imported documents and `parse`d
code, whereas the value already holds the text. The whole text rather than a
chunk around the error, because how much to show is up to the host. Leaving it
out of `inspect` keeps these dumps short and readable. Both bindings make the
string only when it is read, since a document can be large and every nested
`cause` has its own object: a Python property, and in JavaScript the one
accessor of the frozen object (T5 converts the other members up front). The
JavaScript getters are one shared function bound to the object's key in the
table of T5; a closure per object would have to be freed by hand, and one
that held the object would keep it from being collected.

## T12 — 2026-11-04 — JavaScript has a Tuple class

`texl-wasm` exports `Tuple`, a class that extends `Array` and adds nothing. A
Swan tuple becomes a `Tuple` and a Swan list a plain array; a `Tuple` becomes
a normalized Swan tuple (`Value::tuple`) and any other array a list. `Tuple`
items of an array are spread into the list, as in Swan's `[(1, 2), 3]`. A Swan
call with several arguments passes a JavaScript function one `Tuple`, which
destructures as before.
*Reason:* both used to become arrays, so a tuple came back to Swan as a list
and a JavaScript function could not return a tuple at all. A subclass keeps
every array operation working and `instanceof` tells the two apart. Spreading
keeps the Swan invariant that a list holds no tuples. The class is defined
with `inline_js`, the only way to give the package a real JavaScript subclass,
which has two consequences: the release workflow adds `snippets` to the package
files, since wasm-pack leaves them out and the published package would not
load; and the class is exported through `reexport` on a never-called function
import, since on a type `reexport` always declares `Tuple: unknown` in the
TypeScript declarations and would clash with the real one.

## T13 — 2026-11-04 — Lists are normalized wherever they are built

`Value::list` normalizes its items: tuple items are spread into the list and
`Nothing` items are dropped, as Swan's `[X]` has always done. So a `null` or
`None` item of a host array is dropped and later items move down one position.
Members of namespaces are not items and may still be `Nothing`. This supersedes
the spreading in the JavaScript conversion of T12. Both hosts export `normalize`
(`normalize` in JavaScript, `pytexl.normalize` in Python), which gives a host
value this form in the host's own types, through nested arrays, tuples and plain
objects or mappings, and returns anything else as it is.
*Reason:* hosts could build lists Swan cannot, such as a Python list holding a
tuple, and a rule kept in each host would drift. One constructor covers both
hosts and every built-in, and already-normalized lists pay only for one scan.
The cost, accepted by the founder, is that positional data with gaps shifts
silently; such data should be passed as objects. `normalize` is written in the
host's types rather than as a round trip through Swan, which would lose functions
and copy promises. It stops at class instances in JavaScript, which it leaves
alone rather than turning into plain objects, since that is conversion and not
normalization.

## T14 — 2026-11-04 — Host errors are undefined values (supersedes part of T9)

A Python or JavaScript function called from Swan that raises an exception or
throws gives an `Undefined` instead of `Nothing`. So does a rejected JavaScript
promise and a Python awaitable that raises or is cancelled, which T9 made
`Nothing`. It is a new kind of undefined value, `Undef::HostError`, of type
`HostError`, with the text of the error in the member `error`
(`"ValueError: boom"`, `"Error: boom"`) and the position of the call, placed as
for other built-ins (T8, T10). A failed import and a failed load still give
`Nothing`, and a loader that raises is still a failed load.
*Reason:* `Nothing` is the empty tuple, a valid result, so a bug in a host
function silently rendered as an empty text. T9 kept `Nothing` because "there
is no Swan error to keep"; the host's message is that error. A new kind was
needed since `Undef::Value` has no position, and the operation kinds would have
to name an operation and an operand that say nothing about the failure. Only
the text is kept, not the exception object: Swan values cannot hold host
objects, and the conversion keeps Swan values host-independent.
