# Swan and Texl in depth

This page is for advanced users and for developers of Texl and its hosts. It
describes what the user manual leaves out: why the language behaves as it
does, how scopes, promises and memory work, what a template can reach, and
where the limits are. It assumes you know the language; to learn it, read the
user manual linked from the [README](../README.md).

Every statement here describes the current implementation. The source of
truth is the code: `swan-core` for the language, `texl-core` for templates
and documents (see [Architecture](architecture.md)).

## Design principles

### Safe by failure

Evaluation never fails and never panics. Every error, from a syntax error to
an operation on the wrong types, produces an `Undefined` value, which flows
through the rest of the expression like any other value. Built-in functions
follow the same rule.

```texl
a <% 1 + %> b <% "a" - 1 %> c <% 2 %>
```

```text
a [[Undefined]] b [[Undefined]] c 2
```

The reason is the audience. Texl documents are written and kept by people
who are not programmers, and they are rendered again and again as the data
they link to changes. A rendering that stops at the first error would hide
the whole document behind one broken tag; Texl instead renders everything it
can and marks the failures in place.

Consequences:

- **Each tag is parsed on its own.** A syntax error affects one tag only; the
  next tag starts clean.
- **A tag is one whole expression.** Nothing in it is silently ignored: a
  token left over after the expression (`<% 2 + 1e3 ) 1 %>`) makes the whole
  tag `Undefined`, with the first syntax error of the tag (``unexpected `)` ``),
  and none of it runs, assignments included. A group closed by the wrong
  token, as in `(1 @ 2)`, is an unexpected token at that token; ``missing
  closing `)` `` means the tag ended before the group was closed.
- **An `Undefined` value records what went wrong.** Its payload (the Rust type
  `Undef`) has a kind (`UnaryOperation`, `BinaryOperation`, `Termination`,
  `Token`, `Name`, `Value`), the operation, the operands, a description in
  plain words, and where it was made. `undef.inspect` returns it as a
  namespace; the hosts expose it as `pytexl.Undefined` and `TexlUndefined`.
  See [Finding errors](#finding-errors).
- **Errors are values.** `Undefined` is falsy, so `x | fallback` and
  `x ? a ; b` deal with it like with any other value. It renders as
  `[[Undefined]]`.
- **Promises never fail.** A promise has no failure state. An `Undefined`
  value is not a failure: a promise resolved with one holds it, like any
  other value, so `x ~> f` behaves as a later `f x` and an error reached
  through a promise renders as `[[Undefined]]`. A rejected JavaScript promise
  and a Python coroutine that raises hold an `Undefined` too, as does the
  import of a document that cannot be loaded.
- **Host functions must not throw.** A Rust built-in returns an `Undefined`
  (for example with `Value::undef_unary`) instead of panicking. The JavaScript
  and Python bindings turn an exception in a host function into an `Undefined`
  of type `HostError`, whose `error` member holds the host's message.

There is one exception to the rule: infinite recursion overflows the stack and
stops the host program (see [Limits](#limits-and-performance)).

### Everything is a tuple

Every value is conceptually a tuple: `Nothing` is the empty tuple, a single
value is a tuple of one item. Tuples are flat (a tuple never contains a
tuple) and never contain `Nothing`. Most operators apply item by item:
`(1, 2) + (10, 20)` is `(11, 22)`.

Lists are normalized too: a list holds no tuples and no `Nothing`, so
`[(1, 2), (), 3]` is `[1, 2, 3]`. Hosts follow the same rule, because
`Value::list` applies it: a `null` or `None` item of a host array is dropped,
and later items move down one position. Members of namespaces are not items,
so a host object keeps its `null` members. To keep positions in data with gaps,
pass an object rather than an array.

The short-circuit operators `&`, `|`, `?` and `;` are the exception. They
look at their left operand as a whole (a tuple is true if any of its items
is; `;` checks for `Nothing`) and evaluate the right operand only when it is
the result.

### Immutable values

`Text`, `List` and `Namespace` values never change. An operation that
"changes" one builds a new value. Swan code binds names only in scopes,
which are not values: a namespace literal `{...}` evaluates its content in a
new child scope and returns a *snapshot* of that scope's bindings.

```texl
<% x = 1 %><% point = {y: x} %><% x = 2 %><% point.y %>
```

```text
1
```

Immutability makes sharing cheap and safe: a host can hand the same namespace
to many documents, and a document's `data` cannot be changed by the documents
that import it. It also means that only scopes and promises can be part of a
reference cycle (see [Memory management](#memory-management)).

### Values as text

One conversion turns a value into text, and both Swan's `text.from` (item by
item over a tuple) and the template renderer use it (`to_text` in
`swan-core`'s `builtins`): `Text` as is, numbers as Rust's `f64` display writes
them (`30`, `1.5`), `Bool` as `TRUE`/`FALSE`; lists, namespaces, functions and
undefined values as markers: `[[List of N]]`, `[[Namespace of N]]`, `[[Func]]`,
`[[Undefined]]`.

A namespace with a `__text__` member gives its own text: the member is applied
to the namespace itself, and the text it returns is the namespace's text. So a
value can be labelled as data and rendered in place in one tag:

```texl
<% money = {amount: 12.5, currency: "EUR", __text__: m -> text.from(m.amount) + " " + m.currency} %>Price: <% price: money %>
Twice: <% money.amount * 2 %>
<% text.from(price, 3) %>
```

```text
Price: 12.5 EUR
Twice: 25
12.5 EUR, 3
```

- The hook gets the namespace as its argument rather than reading names from
  its closure, so one function can serve many namespaces.
- It must return a text, and only a text: anything else (a number, a tuple, a
  promise, an undefined value), or a `__text__` that is not a function, makes
  the conversion `Undefined` ("the `__text__` member of a namespace must return
  a text, not the number `1`"), which a tag renders as `[[Undefined]]`. The
  returned text is not converted again, and a promise is not awaited.
- `__text__` is a member like any other: `nspace.size` counts it, it is in a
  document's `data`, and hosts receive it as a function. A host can make such a
  namespace: a Python dict or a JavaScript object with a `__text__` function
  (decision T16 brings the function back as itself).
- A hook that converts its own namespace (`text.from` of it) recurses forever,
  like any other unbounded recursion.

What a tag adds to this conversion, for the values it leaves out, is shared
too (`to_rendered_text`): `Nothing` is empty, a tuple's items are joined with
`", "`, a resolved promise gives its value's text and a pending one
`[[Pending Promise]]`, and a value that cannot be converted gives
`[[Undefined]]`. Swan's `text.template` uses it, so a `{name}` placeholder
shows a value exactly as `<% name %>` would:

```texl
<% row = text.template "{name}: {sales} ({tags})" %><% row {name: "North", sales: 1200, tags: ("a", "b")} %>
<% row {name: "South"} %>
```

```text
North: 1200 (a, b)
South:  ()
```

- The template is parsed once, when `text.template` is applied to it; the
  function it returns can be applied to many namespaces, and to a tuple of
  namespaces (a tuple of texts).
- A placeholder is `{` + a Swan name + `}`. `{{` and `}}` are literal braces;
  any other brace (`{ a }`, `{a.b}`, `{}`) is literal text, so a template never
  fails.
- A missing name is `Nothing`, as in a tag, so it gives an empty text, not an
  error. A template applied to anything but a namespace gives `Undefined`.
- Placeholders are names, not expressions: the template is data, so a text
  from outside (a host, a loaded file) can be one without running code.

Names that start and end with two underscores are reserved for hooks like
this one, present and future. The parser does not enforce it: documents should
not use such names for anything else.

## Scopes and names

A *scope* is the environment in which code runs: a table of bindings plus a
link to a parent scope. Looking up a name searches the scope, then its
ancestors; a name that is bound nowhere is `Nothing`. Scopes are internal:
Swan code never holds one as a value.

### The layers of a document

When the engine renders a document, it builds this chain of scopes, from the
outermost:

| Scope            | Holds                                                   | Shared by                       |
|------------------|---------------------------------------------------------|---------------------------------|
| built-ins        | `TRUE`, `FALSE`, `type`, `parse` and the built-in namespaces (`bool`, `numb`, `text`, `list`, `nspace`, `promise`, `undef`, `math`, `time`) | every scope of the thread       |
| globals          | `bind`, `label`, and a copy of the engine's globals     | one document or context         |
| document scope   | the names the document's tags bind, and `texl`          | one document or context         |
| local scopes     | function parameters, the content of `{...}`             | one call or one literal         |

- The **built-ins scope** is created once per thread and never freed. It is
  *frozen*: Swan code cannot bind names in it. A host replaces a built-in with
  `swan_core::builtins::set_builtin`, which every scope of the thread sees at
  once (the JavaScript host replaces `time.now` this way, because
  WebAssembly has no system clock).
- The **globals scope** receives a copy of the engine's globals when the
  document scope is created (`Engine::create_scope`). Globals added to the
  engine later do not reach existing scopes. Every engine starts with one
  global, `undef`: the built-in namespace with Texl's `undef.inspect`, which
  gives positions in the document (see
  [Positions and origins](#positions-and-origins)).
- The **document scope** is where the tags of a document run. Its bindings,
  taken when the rendering ends, are the document's `data`.

User code can shadow any outer name, built-ins included, without changing it:
`text = 1` binds `text` in the current scope only.

### Binding names

`=` and `:` bind names in the current scope, never in an ancestor. A
function's body runs in a new child scope of the scope the function was
defined in, so a function cannot change the names around it.

`bind` and `label` are the exception: they bind names in the document scope,
whatever scope they are called from. That is how a promise callback makes a
value available to the following tags:

```texl
<% f = n -> bind "answer" n %><% f 42 %>The answer is <% answer %>.
```

```text
The answer is 42.
```

`bind` and `label` live in the globals scope and hold the document scope
weakly, so they do not keep it alive.

### Closures see later bindings

A function captures its defining scope itself, not a copy. Names in its body
are looked up when it is called, so it sees names bound after it was defined.
This is what makes recursion work: `f` is bound after the function that
refers to it is created.

```texl
<% f = () -> later %><% later = 5 %><% f () %>
```

```text
5
```

### Frozen scopes

Two kinds of scope are frozen: the built-ins scope, and the scope of a
*subcontext*. `ns.X` evaluates `X` in a frozen child of the current scope
whose bindings are the members of `ns`: `X` sees the members and the names
around it, and `=` or `:` inside it give `Undefined` (operation
`assignment` or `labelling`).

```texl
<% type (text.(size = 0)) %>, <% (undef.inspect (text.(size = 0))).op %>
```

```text
Undefined, assignment
```

`parse` compiles source text into a function of a namespace. The code runs in
a frozen scope that holds the members of that namespace, directly below the
built-ins: it sees the members and the built-ins, and nothing else, neither
the names of the calling document nor the engine's globals.

## Evaluation order

- **Operands** are evaluated eagerly, left before right. The short-circuit
  operators evaluate their right operand only when needed: `A & B` and
  `A ? B` when `A` is true, `A | B` when `A` is false, `A ; B` when `A` is
  `Nothing`.
- **The left side of `->`, `=` and `:`** is not evaluated: it is a pattern of
  names.
- **Tags** are evaluated in the order they appear, all in the same document
  scope, so a name bound in a tag is visible in the following ones. Text
  outside tags is copied as it is. A tag ends at the first `%>`, even inside a
  text literal; a `<%` without a closing `%>` makes the rest of the template
  plain text.
- **Rendering a value.** `Nothing` is written as nothing and a tuple as its
  items joined with `, `; every other value is written as its text form (see
  [Values as text](#values-as-text)), and a value with no text form as
  `[[Undefined]]`.

## Promises and async rendering

A promise is a value that may not be available yet. In Swan it has three
properties:

- **It resolves once.** Later resolutions are ignored.
- **It never rejects.** There is no failure state (see
  [Safe by failure](#safe-by-failure)); an `Undefined` value is held like
  any other value.
- **Callbacks run synchronously.** A callback registered with `~>` runs when
  the promise resolves, on the stack of the code that resolves it, or at once
  if the promise is already resolved. `A ~> f` always returns a promise, even
  when `A` is not one.

### How rendering waits

`template::render` is an `async` function. When a tag's value is a promise,
rendering stops at that tag until the promise resolves (again and again, if
it resolves to another promise), writes the value and moves on. Only the
promise that the tag itself returns is awaited: a promise inside a tuple or a
list is written as its current value, or `[[Pending Promise]]`.

A tag that returns a promise that is never resolved, such as
`promise.create ()`, makes the rendering wait forever. There is no timeout.

### Where the waiting happens

Swan itself has no event loop. Work that waits, such as an import or an
`async` host function, runs as a background *task* that resolves a promise
when it is done. The engine starts tasks through a *spawner* that the host
provides: `spawn_local` in the browser, the asyncio loop in Python, any
single-threaded executor in Rust. Without a spawner, tasks wait in a
thread-local queue that the host drains with `helpers::run_pending`.

So within one rendering:

- the tags of a document run one after the other, but the imports they start
  run concurrently: a tag that starts two imports and returns a promise that
  needs both waits for the slower one, not for the sum;
- an imported document renders in its own task, while the importer waits for
  it.

## Documents, imports and caching

A *document* is a template rendered at a URL by `Engine::render(url, source)`
or `Engine::load(url)`. The result is a namespace `{uri, source, data, text,
tags}`: `text` is the rendered text, and `tags` describes each tag (see
[Finding errors](#finding-errors)). Imported documents have the same shape.
Every document gets its own document scope, with a `texl`
namespace whose `import` resolves URLs against the document's URL.

### URLs

The engine normalizes every URL: the fragment is removed, the query is kept as
it is. Relative URLs in `texl.import` resolve against the importing document
like links in a web page (`../lib/a.texl`, `/lib/a.texl`). A URL without a
scheme is a path, normalized to an absolute path (`/docs/a.texl`) and served
by the default loader; the JavaScript and Python hosts resolve everything to
full URLs against a base URL instead.

### Loaders

The engine reads nothing by itself. A *loader* is an async function from a
URL to the source text, registered per scheme (`add_loader("https", ...)`) or
for paths without a scheme (`add_default_loader`). `Engine::fetch(url)` calls
the matching loader. All loading goes through it. A missing loader or a loader
error gives an `Undefined` of the operation `document.load`, with the URL in
`lhv` and the reason in `rhv`. `Engine::fetch` and `Engine::load` return it as
their `Err`, the JavaScript and Python hosts raise it as an error, and
`texl.import` gives it to the template.

### Final URLs

A loader may also return the *final URL* of the document, the way an HTTP
redirect does (`Loaded { url: Some(..), source }` in Rust, `{url, source}` in
JavaScript, `(final_url, source)` in Python). The document then is the one at
its final URL: that is its `uri`, the URL its relative imports resolve
against, and its key in the import cache. The final URL resolves against the
requested one and is normalized like it. The built-in HTTP loaders return the
URL after redirects.

This lets a host give a URL a meaning of its own without the engine knowing
about it. A host where `~` stands for a project folder resolves
`/p/docs/~/lib/x.texl` to `/p/lib/x.texl` in its loader and returns that URL:
every way of naming the library leads to one document, and a library that
imports itself through `~` is a cycle, not an ever longer URL. Module systems
do the same: resolve the specifier to a real path first, then identify the
module by that path.

### One cache per rendering (decision D17)

The engine holds configuration only: its globals and loaders. It has no
document cache. Each call to `Engine::render` creates an import cache for that
rendering and drops it at the end:

- **Within a rendering**, each URL is loaded and rendered once. The cache
  stores the promise as soon as an import starts, so concurrent imports of the
  same document share one load. `a.texl` and `a.texl#top` are the same
  document.
- **Final URLs.** Until it is fetched, a document is in the cache under the
  URL that was requested. When the loader returns another final URL, the
  requested URL becomes an alias of it: the import waits for the document at
  the final URL, which renders under it unless it is in the cache already, and
  a later import of the requested URL in the same rendering fetches nothing.
  Two different URLs with the same final URL are each fetched once (the final
  URL is not known before), and the document is rendered once.
- **A failed load** resolves the import to that `Undefined`, placed at the
  import, and removes the entry, so a later import of the same URL in the same
  rendering tries again. A fallback is written with `|`:
  `texl.import("a.texl") ~> (d -> d.text | "no text")`.
- **Across renderings** nothing is kept: the next rendering loads every
  document again and sees its current content. A host that wants a cache wraps
  its loaders.

The reason is that Texl documents are living documents: a rendering must show
the current state of everything it links to, and a long-running host (an
editor, a server) must not grow a cache of stale documents.

### Isolation between documents

An imported document renders in its own document scope, created like the root
one: it sees the built-ins and the engine's globals, never the names of its
importer. Its `bind` and `label` write into its own scope. Its `data` is a
snapshot of its bindings when its rendering ends, and like every namespace it
never changes afterwards.

### Import cycles

The cache holds the root document too, as a pending promise. Before a document
X waits for a document Y that is still rendering, the engine checks, with a
depth-first search in its *wait-for graph*, whether Y already waits for X,
directly or through other documents (X importing itself counts). If so, the
import returns `Undefined` (operation `import.cycle`) instead of a promise, and
X renders without it; otherwise the edge X → Y is recorded until Y resolves.
Importing a document that has finished rendering is never a cycle.

A cycle through a [final URL](#final-urls) is found after the fetch, since the
engine does not know before which document a requested URL is: while the URL
is fetched, its importers wait for it under that URL; then, if the document at
the final URL is pending and waits for them, the import resolves to
`Undefined` (`import.cycle`), and otherwise their edges move to the final
URL. All imports that share the fetch of one URL get the same result, so an
importer outside the cycle that requested the same URL at the same time gets
`Undefined` too. A document that does not wait for its import is not stuck:
if it finishes rendering before the fetch ends, the import gets the finished
document.

### Contexts

A *context* (`createContext` in JavaScript, `create_context` in Python,
`Engine::create_scope` in Rust) is a document scope without a document: the
host renders several templates in it and reads and writes its names. Its
`texl` namespace has no `import`, so its templates cannot import documents.

## Finding errors

An error does not stop the rendering, so a host needs another way to show
which tag failed, where and why. Two things give it.

### Undefined values explain themselves

`Undef::describe` writes the error in plain words, from one table of
operations: ``cannot add the text `"a"` and the number `1` ``, ``missing
closing `)` ``, ``unexpected `@` ``, `` `math.sqrt` is not defined for the
number `-1` ``. A value is named by its literal when it is short (a number, a
text up to 24 characters, a short tuple), otherwise by its type ("a list").
The description of an undefined value made by `undef.from` names the value it
wraps.

The description covers one level. An operand that is itself undefined is
"an undefined value", and `Undef::cause` returns it (the first undefined
operand, or the first undefined item of a tuple operand), so a host can
walk from the outer value to the root cause:

```texl
<% e = 2 * ("a" - 1) %><% (undef.inspect e).description %>
<% (undef.inspect e).rhv.description %>
```

```text
cannot multiply the number `2` by an undefined value
cannot subtract the number `1` from the text `"a"`
```

`undef.inspect` returns `type`, `description`, the members of the kind
(`op`, `val`, `lhv`, `rhv`, `expected`, `token`, `error`), `pos` and `uri`; `pos` and
`uri` are absent when unknown. In a document, `pos` is a byte offset in the
document. This is Texl's `undef.inspect`, which the engine binds in every
scope it creates; Swan's own `undef.inspect` (in `parse`d code, or in a scope
not made by the engine) gives `pos` relative to the expression and `origin`
instead of `uri` (see below).

### Positions and origins

Swan knows nothing about documents. An undefined value records a byte offset
(`pos`) in the `Expression` that made it (`swan_core::expression`): the text
that was evaluated and an optional *origin*, which the host sets and Swan
only displays. The evaluator keeps the expression it runs in a thread-local
*current expression*:

- the host sets it around an evaluation (`with_expression`);
- a function keeps the current expression of the place it was defined, and
  its body runs with it, so an error inside a function imported from another
  document points into that document;
- a built-in function runs with no expression and does not know where it is
  called from: it makes its errors with no position
  (`Value::undef_unary(op, val)`), and the evaluator gives each error the
  position of the call when the built-in returns it (the argument, or the
  `=>` of `x => f`), in the expression the call is in. An error that already
  has a position, such as an undefined argument passed on, keeps it;
  position `0` is a real position, not "none". A pending promise a built-in
  returns gives the same position to the value it resolves to; the first
  call that returns it decides, and it stays the same promise;
- a built-in or host that calls a Swan value from Rust uses
  `evaluator::apply`, which adds no position, so the errors of that call are
  placed at the call of the outer built-in. A composed function (`f << g`)
  works this way too;
- code parsed with `parse` runs as an expression of its own, with no origin.

Texl evaluates each tag as its own expression, whose origin
(`texl_core::document::TagOrigin`, displayed as `<uri>::<tag start>`) holds
the document (its URI and the text that was rendered) and where the tag and
its expression start. `texl_core::document::locate` turns a value's `pos`
into an offset in the whole document, with its URI and the start of its tag.
The origin keeps the rendered text alive as long as a value refers to it, so
the position stays right after the document is edited and rendered again,
and whichever way the value arrives (another tag, `data`, a function
result). Hosts then convert the offset to their own unit: UTF-16 code units
in JavaScript, code points in Python.

The tag that shows a value and the place that made it can differ: in
`<% x = "a" - 1 %>…<% x %>` the second tag shows the error, and its origin
points into the first.

### Tags

The rendering of a template records every tag: the byte span of the tag in
the source (`start`, `end`, from `<%` to after `%>`), the byte span of its
text in the output (`out_start`, `out_end`) and the value that was rendered
(`value`). Tags that render nothing, such as assignments, are included. The
value is the one the tag rendered: a promise returned by the tag is awaited,
but promises inside the value (tuple items) are not, as in the text. A
document holds the values of all its tags as long as it lives; the cycle
collector traces them like any other value.

Known limits:

- **Unknown names are not errors.** A name that is not bound is `Nothing`,
  not `Undefined`, so a typo in a name renders as an empty text.
- **Built-in errors point to the call, not further.** An error raised
  inside a built-in (`math.sqrt(-1)`, `texl.import` of an invalid URI) is at
  the call that returned it. Errors inside a list or namespace that a
  built-in returns stay with no position, as does an error made by a
  built-in called from another built-in's code until the outer call returns
  it.

## Memory management

### Reference counting

Swan values are reference-counted (`Rc`). Cloning a value is cheap, and a
value is freed as soon as the last reference to it goes. Most memory is freed
this way, at once.

### Cycles through closures

Reference counting cannot free a cycle, and Swan code makes them all the time.
A function holds the scope it was defined in, so a function stored in that
scope (or in a namespace or list stored there) keeps the scope alive, and the
scope keeps the function alive:

```text
scope ──"f"──▶ Func ──closure──▶ scope        f = x -> x
```

Every cycle passes through a scope or a promise, since they are the only
mutable objects.

### The cycle collector

`swan_core::gc::collect()` frees the cycles that nothing outside them refers
to any more. It uses *trial deletion*, the algorithm of CPython's `gc` module:

1. **Discover.** Every scope and promise registers a weak reference in a
   thread-local list when it is created. From each live one, the collector
   follows the references it holds: a scope's parent and bindings, the items of
   lists, tuples and namespaces, a function's closure, the payload of an
   `Undefined`, a promise's value and callbacks, the values captured by a
   `BuiltinFn::capturing` built-in.
2. **Subtract.** For each object, it subtracts the references that come from
   other objects of the graph from its reference count. What remains are the
   references from outside: the host, a Rust variable, a running task, a
   closure it cannot look into.
3. **Mark.** Objects with outside references are in use, and so is everything
   they reach.
4. **Clear.** It empties the scopes and promises that are not in use. That
   breaks every unused cycle, and reference counting frees the rest.

It returns the number of scopes and promises it cleared.

The collector can keep too much, never too little. A reference it cannot see
counts as an outside reference, so what it points to is kept. Values captured
by an opaque Rust closure (a `BuiltinFn::new` built-in, a `Promise::then`
callback) are therefore never collected while the closure lives, and a cycle
through such a closure leaks. That is why built-ins that hold Swan values use
`BuiltinFn::capturing`, and why `~>` stores its Swan callback as a traced
`PromiseCallback::Apply` rather than as a closure.

### When it runs

`Engine::render`, and so `Engine::load`, runs the collector when a rendering
is done. A host that only renders documents has nothing to do. A host that
holds contexts, or evaluates Swan code itself, calls it after dropping what it
no longer needs:

| Host       | Collector                       | Drop first                     |
|------------|---------------------------------|--------------------------------|
| Rust       | `swan_core::gc::collect()`      | the scopes and values          |
| JavaScript | `collect()` from `@onlabsorg/texl` | `context.free()`            |
| Python     | `pytexl.collect()`              | `del context` (and other references) |

The collector works per thread: it sees the scopes and promises of the thread
it runs on.

### What a host must do to avoid leaks

- Drop documents and contexts when done with them; documents hold their
  `data`, which holds every function they defined, and so their scopes.
- Run the collector after dropping a context or a document that defined
  functions, unless a rendering will run it soon anyway.
- Nothing to do for the Swan functions given to the host: when the host's
  garbage collector frees one (a Python `Function` object, or a JavaScript
  function, which `texl-wasm` tracks with a `FinalizationRegistry`), its
  reference goes, and the next collector run frees its scope.
- In a Rust built-in that keeps Swan values, use `BuiltinFn::capturing`, not a
  closure that captures them.
- Remember that the host's own garbage collector does not see Swan references,
  and the cycle collector does not see host references. A Swan value held by a
  host object (a `pytexl.Function` in a Python list, a Swan function held by
  JavaScript) lives as long as that object, and a cycle that passes through both a host object and a Swan value
  is freed by neither.

### Measuring leaks

`swan_core::value::Scope::live_count()` returns the number of scopes alive on
the current thread. It includes the built-ins scope, which lives as long as
the thread. To check for leaks, compare it before and after a piece of work
followed by `gc::collect()`, as the `leaked` helper of the tests in
`swan-core/src/gc.rs` does. The counter is a Rust API only.

## Determinism and sandboxing

### What a template can reach

A template sees only the names in its scope chain: the built-ins, the engine's
globals and the names it binds itself. Swan has no file, network, process,
environment or clock access of its own. A template reaches the outside world
only through:

- **`texl.import`**, which goes through the engine's loaders, and
- **host functions** that the host passes as globals,
- **`time.now`**, the only built-in that reads outside state (the clock).

So the host decides what a template can reach. Note what the default loaders
allow:

| Host       | Default loaders                         | A template can read                    |
|------------|-----------------------------------------|----------------------------------------|
| Rust       | none                                    | only what the registered loaders serve |
| JavaScript | `http`, `https` through `fetch`         | what the page could `fetch` (same origin, or CORS) |
| Python     | `file` and paths, `http`, `https`        | any file the process can read, any URL |

An imported document's `source` is in the import result, so a template can
read any text the loaders serve, not just Texl documents. A host that renders
untrusted templates registers only the loaders it needs; in Python,
`add_loader("file", ...)` and `set_default_loader(...)` replace the built-in
file loader, for example with one that raises; in JavaScript,
`addLoader("https", ...)` replaces the built-in `https` loader.

Other boundaries:

- A document cannot see or change the names of another document, only read
  the `data` snapshot of the documents it imports.
- Swan code cannot change the built-ins (their scope is frozen) or any
  namespace or list it receives (they are immutable).
- Code compiled with `parse` sees only the namespace it is called with and the
  built-ins.

### Determinism

Rendering the same sources with the same globals and loaders gives the same
output, except for:

- `time.now`, and host functions that are not deterministic;
- documents whose content changes between renderings (nothing is cached);
- the timing of concurrent imports, which can only matter through host
  functions with side effects.

All time functions work in UTC, so the result does not depend on the host's
time zone. `nspace.enum` returns names sorted, so the order of a namespace's
members never shows through.

### No resource limits

The engine sets no limit on time, memory or depth. Swan has no loops, so a
rendering can run forever only through recursion that does not end, which
overflows the stack, or through a promise that never resolves, which makes the
rendering wait forever. A host that renders untrusted templates runs them in a
separate process with its own limits.

## Limits and performance

- **Numbers** are 64-bit floats: integers are exact up to 2<sup>53</sup>.
- **Positions** in `Undefined` values are byte offsets in the expression
  that made them; Texl turns them into offsets in the document (see
  [Positions and origins](#positions-and-origins)) and the hosts convert
  them. Errors of built-in functions are at the call.
- **Recursion** uses the native stack, with no depth limit: a recursion that
  never ends stops the host program with a stack overflow instead of producing
  `Undefined`. This is a known issue.
- **Single-threaded.** Values use `Rc` and nothing is `Send`. An engine, its
  scopes and its values stay on the thread that created them (the Python
  bindings raise `RuntimeError` on another thread). Each thread has its own
  built-ins, collector and live-scope counter.
- **Parsing.** Each tag is parsed every time it is rendered; there is no
  cache of parsed templates.
- **No document cache.** Every rendering loads its imports again; wrap the
  loaders to cache sources.
- **Copy-on-write.** A scope's bindings and the namespace snapshots taken
  from it share one map. Binding a name after a snapshot copies the map once.
- **Immutable lists and namespaces.** An operation that returns a changed
  list or namespace copies it, so building a large one item by item, through
  recursion, costs time proportional to the square of its size.
- **Collector cost.** `gc::collect` traces every live scope and promise of the
  thread and what they reach, not only the garbage, so its cost grows with
  everything the host keeps alive. It runs once per rendering.
