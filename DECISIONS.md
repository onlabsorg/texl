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

## T15 — 2026-11-04 — A failed load says why (supersedes part of T14)

A document cannot be loaded when the loader fails or no loader matches the URL.
All loading goes through `Engine::fetch`, which then makes an `Undefined`: one
binary operation, `document.load`, with the normalized URL in `lhv` and the
reason in `rhv`. Loader messages no longer repeat the URL. Each caller gets this
failure in its own form:

- In Rust, `Engine::fetch` returns `Ok(source)` or `Err(undefined)`, and
  `Engine::load` returns `Ok(document)` or `Err(undefined)`.
- In Python, `fetch` returns a string and `load` a `Document`, or they raise
  `pytexl.LoadError`, with `uri` and `reason`.
- In JavaScript, `fetch` resolves to a string and `load` to a `TexlDocument`, or
  they reject with an `Error` named `LoadError`, with `uri` and `reason`.
- In a template, `texl.import` returns a promise of the document or of the
  `Undefined`, placed at the import (T10). This replaces `Nothing`.

A failed load is still not cached. JavaScript's `fetch`, `load` and `render` also
reject with an `Error`, not a string, for a URI that cannot be resolved to a URL.
*Reason:* `Nothing` is a valid result, so a broken link rendered as an empty
text, and a host could not tell why a load failed; this is the problem T14 solved
for host functions. Templates have no exceptions, so errors are values there. A
host calls `fetch` and `load` as ordinary functions of its own language, so it
gets that language's way of failing, as for a URI it cannot resolve. An operation
fits since it has a real operand, the URL, as `import.cycle` does, and keeps the
URL and the reason apart. `HostError` would not cover a URL with no loader, and
would leave the URL inside a host's wording.

## T16 — 2026-11-08 — Swan functions reach JavaScript; host functions come back as themselves

In `texl-wasm`, a Swan function (a `Func` or a built-in) becomes a plain
JavaScript function instead of `undefined`:

- `f(a, b)` calls the Swan function with the tuple `(a, b)`, `f(a)` with `a`
  and `f()` with `()`; `this` is ignored. A JavaScript function called from
  Swan gets the arguments the same way: Swan `f(a, b)` calls it with `a` and
  `b`, `f()` with none, as in Python. This replaces the single argument that
  received a tuple as a `Tuple`.
- The result is converted as usual: a promise becomes a JS Promise, and an
  undefined value is returned as a `TexlUndefined`, never thrown.
- The function is a handle, like a `TexlUndefined` (T5): it holds a key into
  a table of Swan values, shared with undefined values, and a
  `FinalizationRegistry` removes the entry when JavaScript collects the
  function. Its scope is then freed by the next `collect()`, which is not run
  from the finalizer. Passed back to Swan, it is the original Swan function.
- Converting the same Swan function twice gives two different JS functions.
  `isSwanFunction(value)` tells a Swan function from the host's own.

In both bindings, a host function that goes into Swan comes back as itself: a
JavaScript function as that function, a Python callable as that callable (no
longer a `pytexl.Function`). A built-in carries the host function it wraps as
opaque host data (`BuiltinFn::with_host_data` / `host_data`), which must not
hold Swan values.
*Reason:* hosts need to call the functions a document defines (a document as a
library), as Python could already. A `wasm_bindgen` `Closure` would have to be
leaked, since JavaScript does not say when a function is no longer used; the
handle table frees every function JavaScript drops. Spreading the arguments
makes calls read the same in both directions and in both bindings. Returning
undefined values keeps "never fails". Identity and a finalizer-driven collection
were left out: the first needs a weak reverse map, and finalizers run at
unpredictable times while `collect()` already runs after every rendering. A
cycle that passes through both a host object and a Swan value is still freed by
neither collector; that is a separate question.

## T17 — 2026-11-08 — Namespaces give their own text with `__text__`

A namespace with a `__text__` member converts to text by applying that member
to the namespace itself. The result must be a text; anything else, promises
included, makes the conversion `Undefined` (operation `__text__`, the namespace
in `lhv`, the returned value in `rhv`), rendered `[[Undefined]]`. One
conversion of single values (`swan_core::builtins::to_text`) holds this rule
and is used by every stringifier: Swan's `text.from` and Texl's renderer, which
adds only what is template-specific (`Nothing`, tuples, promises). `__text__`
is an ordinary member, with no hidden exception. Names of the form `__name__`
are reserved for such hooks by documentation only; the parser accepts them.
*Reason:* a function that returns data often has a natural rendering (a table,
a quantity with its unit); with the hook, `<% t: table(...) %>` labels it and
renders it in one tag. Putting it in Swan rather than in the renderer keeps
one text form per value, so nested values and Swan code that builds text agree
with what a tag shows. Passing the namespace, not closing over it, lets one
function serve many namespaces. Allowing only text keeps `text.from`
synchronous and stops a hook from looping by returning its own namespace. A
visible member, unlike hidden metadata, survives a round trip through Python
or JavaScript, so a host can build such values.

## T18 — 2026-11-09 — A loader may give a document's final URL

A loader returns the source of a document and, optionally, its *final URL*,
the way an HTTP redirect does: `Loaded {url, source}` in Rust, `{url, source}`
in JavaScript, `(final_url, source)` in Python; a bare string still means the
requested URL. The final URL resolves against the requested one and is
normalized like it. The document is the one at its final URL: its `uri`, the
base of its relative imports and its key in the import cache. A requested URL
whose final URL differs becomes an alias of it for the rest of the rendering,
so it is fetched once; the document at the final URL renders once, whatever
URLs lead to it. While a URL is fetched, its importers wait for it under that
URL and share one promise, as before; after the fetch, if the document at the
final URL is pending and waits for them, the import resolves to `Undefined`
(`import.cycle`), otherwise their wait-for edges move to the final URL. The
HTTP loaders of both bindings return the URL after redirects
(`response.url`, `geturl()`), so `pytexl.loaders.load_http` now returns
`(final_url, source)`. `fetch` returns the final URL with the source:
`Fetched {url, source}` in Rust, `{url, source}` (`TexlFetched`) in
JavaScript, a `Fetched(url, source)` named tuple in Python; so `load(uri)` is
`render` of what `fetch(uri)` returns, and `load` gives the final URL as the
document's `uri`. `Engine::render(url, source)` is unchanged: no loader
runs for the root.
*Reason:* the engine took a document's identity from the URL it requested,
before the loader ran, so a host that gives a URL a meaning of its own (Stilo's
`~` for the project home) chose which file was read but not which document it
was: a library importing itself through `~` asked for a longer URL at every
turn and never closed the cycle, two paths to one library rendered it twice,
and relative imports resolved from the alias. Resolving first and identifying
by the result is what module systems (Node.js) and HTTP do, and Texl still
gives no path segment a meaning. `fetch` returns the final URL too, at the
cost of compatibility, so that a host that fetches a source and renders it
later (an editor) renders it as the right document. Keeping one shared promise per requested URL
keeps promise identity (`a == b` for two concurrent imports of one URL) exactly
as it was when no loader redirects; the price is that importers sharing the
fetch of a URL that turns out to close a cycle all get `Undefined`, not only
those in the cycle. A cycle through a redirect is found after one fetch
instead of before it, and an importer that does not wait for its import may
finish first and then get the finished document, as any import of a finished
document does.

## T19 — 2026-11-09 — `text.template` fills `{name}` placeholders from a namespace

`text.template tpl` returns a function that takes a namespace and returns
`tpl` with every `{name}` placeholder replaced by the text of that member.
The value is converted exactly as a Texl tag renders it: the renderer's
conversion moved to swan-core (`builtins::to_rendered_text`, built on
`to_text`), and both use it. A placeholder is `{` + a Swan name + `}`; `{{`
and `}}` are literal braces, any other brace is literal text, so parsing never
fails; the template is parsed once, when `text.template` is applied. A missing
name is `Nothing` and gives an empty text. `text.template` maps over a tuple
of templates and the returned function over a tuple of namespaces; any other
argument gives `Undefined`.
*Reason:* building text from many values with `+` and `text.from` is long and
hard to read, while a document already knows how to show any value; sharing
the tag's conversion keeps one text form per value (T17). Names rather than
expressions keep a template plain data, safe to take from a host or a file.
Treating a missing name as Nothing follows Swan's name lookup and the
renderer (`<% colr %>` renders nothing); the price is that a misspelled name
is silent, as it is in a tag. Python-style brace escapes are familiar, and
leaving other braces literal lets texts like JSON pass through unchanged.

## T20 — 2026-11-12 — A syntax error anywhere is the whole expression (extends T7)

`Parser::parse` returns the first syntax error of the source whenever the
source has one, not only when tokens are left over after the expression: an
unclosed group (`{a = 1`, `[1`), a wrong token inside a group (`1 + )`), an
error in a part that is not evaluated (`TRUE | (1 +`, the body of a function).
The whole expression is that `Undefined`, nothing of it runs, and the error
is the value itself, no longer the cause of an operation that received it
(`1 + )` is ``unexpected `)` ``, not "cannot add the number `1` and an
undefined value").
*Reason:* T7 made a syntax error the whole expression only for left-over
tokens. An error node left inside the tree could still be lost: `{a = 1` gave
an empty namespace (a namespace discards the value of its body), `TRUE | (1 +`
gave `TRUE`, `f = x -> (x +` defined a function, and in `x = 1, (2 +` the
assignment ran. That is the silent loss T7 set out to remove ("a syntax error
must not produce a value").

## T21 — 2026-11-12 — A promise that holds itself is followed once

A promise can be resolved with itself, alone or inside a tuple or list
(`promise.resolve p p`). Whoever follows resolved promises stops at one it is
already following: the renderer awaits each promise of a tag once and then
takes it as the value; the text form (`to_rendered_text`, so tags and
`text.template`) writes `[[Promise]]` where a promise is met again inside its
own value; the JavaScript conversion gives the JS Promise already being made
for it. `Promise::resolve` no longer holds its borrow while it places the
positions of the value.
*Reason:* each of these followed the promise forever: the renderer looped
without end, the text form and the JavaScript conversion overflowed the
stack, and `resolve` panicked on a double borrow. Forbidding the value was
not an option: a cycle through a list cannot be seen when the promise
resolves.

## T22 — 2026-11-12 — Evaluation stops at a depth limit

Evaluation counts its depth in *levels*, one for each nested operation being
evaluated and each call in progress, and stops at `evaluator::max_depth()`
(per thread, `set_max_depth`). The call that would go deeper gives an
`Undefined` ("recursion too deep", op `recursion`), and every evaluation in
progress returns that same value, up to the outermost one: it is the result
of the whole expression, not wrapped by the operations on the way. The
default is 800 levels (400 in unoptimized builds), sized for 1 MB of stack;
texl-wasm links a 2 MB stack and sets twice the default, pytexl sets the
default times half the megabytes of the thread's stack when it knows it (the
main thread on Unix, or `threading.stack_size`). A host function call counts
for 16 levels in both bindings (`evaluator::descend`, public for Rust hosts).
The parser stops at 256 levels of nesting with a syntax error (`Node::TooDeep`,
"the expression is nested too deep"). To make the limit worth having, the
stack each level uses was cut: every kind of node is evaluated by its own
function instead of one large `eval`, a chain of `,` is evaluated in a loop,
promises run their callbacks from a list instead of by recursion, and a
syntax tree is freed without recursion.
*Reason:* unbounded recursion overflowed the stack and killed the host: the
one case where an error was not a value. Recursion is Swan's only loop, so
the limit has to be as high as the stack allows, and measuring showed the
stack was the problem: `eval` used about 3 kB per level (19 kB unoptimized),
so 1 MB held about 100 recursive calls, and a tuple of 350 items written in a
tag overflowed it. After the split a level takes at most 0.8 kB (1.7 kB), and
1 MB holds about 600 calls before overflowing; the limit is set at a fraction
of that. Levels rather than bytes, because the wasm build cannot see the
stack that ends first (the browser's, measured in Chrome at about 4000 levels
whatever the wasm stack size). Returning the one value unwrapped avoids a
chain of causes as deep as the recursion, which the conversions to
JavaScript and the collector would then walk by recursion themselves.

## T23 — 2026-11-12 — An import is waited for by the document whose tag is running

swan-core keeps an *entry expression* next to the current one
(`expression::entry`, `with_entry`): the expression at which the evaluation in
progress entered Swan. `template::render` sets it to the tag, and a function
chained with `~>` runs with the expression of its `~>`; function calls and
built-ins do not change it. `texl.import` records its edge of the wait-for
graph, and checks for a cycle, for the document of the entry expression,
falling back to the document the call is written in when Swan was not entered
from a tag (a host calling a function). Relative URIs still resolve against
the document the call is written in.
*Reason:* the edge was recorded for the document that defined the function
calling `texl.import`. When a tag of Y called a function of X that imported
Z, the graph held X → Z while it was Y that waited; Z importing Y then passed
the cycle check and the two waited for each other forever. The current
expression cannot tell who waits: a function body runs with the expression
that defined it (T6), and a built-in with none.

## T24 — 2026-11-12 — Built-ins: date ranges, NaN in max and min, strict numb.from, both arguments in errors

- `time.from_date` rolls fields over, months included (month 13 is January of
  the next year, month 0 December of the one before), and gives `Undefined`
  for a field that is not finite or beyond ±1e9; the functions that take a
  timestamp give `Undefined` for one that is not finite or beyond ±1e15
  seconds. `time.from_string` also checks the hour (0–24), minute and second,
  and the size of the year. `time.to_string` rounds to milliseconds before
  it splits the fields.
- `math.max` and `math.min` return `NaN` if an item is `NaN`.
- `numb.from` accepts only digits, a sign, a decimal point and an exponent:
  no longer `"inf"`, `"nan"`, `"infinity"`.
- A number followed by any letter is one invalid token (`12é`), as it was for
  ASCII letters (`12abc`).
- A two-argument built-in called with an argument of the wrong type gives an
  `Undefined` with both arguments (`text.find 5 "abc"`: "`text.find` is not
  defined for the number `5` and the text `"abc"`"), for `text.find`,
  `rfind`, `head`, `tail`, `split` and `list.find`, `rfind`, `head`, `tail`.
- Binding surplus values to the last name (`(a, b) = (1, 2, 3, …)`) takes one
  step, not one for each value.
*Reason:* large date fields overflowed 64-bit integers (a panic in unoptimized
builds, a meaningless number otherwise) and months beyond 12 gave a wrong
date while days and hours already rolled over; `math.max` of a `NaN` gave
`-INF`; `numb.from` accepted texts that are names, not numbers, in Swan; the
two-argument built-ins reported the valid argument as the invalid one.

## T25 — 2026-11-13 — Host values that contain themselves are errors; contexts run the collector

- A JavaScript array or object that contains itself is a `TypeError` when it
  is converted to Swan or normalized (a function that returns one gives an
  `Undefined` host error). A Python list, tuple or mapping that contains
  itself converts with an `Undefined` host error in place of the item that
  closes the cycle, since the Python conversion cannot raise.
- `TexlContext.render` and `Context.render` run the cycle collector when they
  are done, as `Engine::render` does.
- texl-wasm writes the message of a panic to the console; a handle key still
  in use is not given out again when the keys wrap around.
- The pytexl executor clears its "draining" flag also when a task panics, and
  schedules its drain again when the event loop it was scheduled on was
  closed or replaced before running it.
*Reason:* both conversions followed a cycle forever and overflowed the stack,
which in Python took the interpreter down; Swan values are immutable and
cannot hold a cycle, so there is nothing to convert it to. Garbage made by
context renderings stayed until the host called `collect()`, which a host
that only renders had no reason to do. A panic in the browser showed as
`RuntimeError: unreachable`, with nothing to report. Either executor flag
left set stopped every later rendering on the thread, without an error.


## T26 — 2026-11-13 — A function shares its body with the syntax tree

The right operand of a binary node is an `Rc<Node>` (`ast::BinaryOp.rho`), and
`Value::Func.body` is that same `Rc`: defining a function, reading it from a
scope and calling it copy a pointer.
*Reason:* the body was a `Box<Node>`, so every clone of a function value
copied its whole syntax tree, and a value is cloned each time its name is
read: a recursive function copied its tree at every call.
