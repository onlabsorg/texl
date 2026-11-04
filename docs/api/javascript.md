# JavaScript

The `@onlabsorg/texl` package compiles the Texl engine to WebAssembly, with a
JavaScript API for web pages and web workers. It renders documents, from
strings or loaded over HTTP, and lets templates call your JavaScript
functions.

A package for Node.js is coming soon.

## Installing the package

Install the package from npm, and import it in code that a bundler, such as
Vite or webpack, builds:

```sh
npm install @onlabsorg/texl
```

```js
import init, { TexlEngine } from "@onlabsorg/texl";
```

The package contains the module `texl_wasm.js` with the small script it
imports from `snippets/`, the WebAssembly binary `texl_wasm_bg.wasm`, and the
TypeScript declarations `texl_wasm.d.ts`.

Without a bundler, import the module from a CDN that serves npm packages, with
the version you want:

```js
import init, { TexlEngine } from "https://cdn.jsdelivr.net/npm/@onlabsorg/texl@0.5.0/texl_wasm.js";
```

You can also build the package from a clone of the repository with
[wasm-pack](https://drager.github.io/wasm-pack/), and copy the
`texl-wasm/pkg` folder that it creates to your web site:

```sh
wasm-pack build texl-wasm --target web
```

## Quick start

```html
<script type="module">
import init, { TexlEngine } from "https://cdn.jsdelivr.net/npm/@onlabsorg/texl@0.5.0/texl_wasm.js";

await init();                                  // load the WebAssembly module

const engine = new TexlEngine({ user: { name: "Ada" } });
const doc = await engine.render("hello.texl", "Hello <% user.name %>!");
console.log(doc.text);                    // Hello Ada!
</script>
```

The page must be served over HTTP: browsers do not load WebAssembly modules,
or let them fetch documents, from `file://` pages.

## Loading documents

`engine.load` fetches a document by URL, renders it, and returns its
rendering, source and data:

```js
const doc = await engine.load("templates/report.texl");
document.querySelector("#report").textContent = doc.text;
```

A relative URL is resolved against the engine's base URL, which is the URL of
the page by default. Inside a document, [imports](../language.md#documents-imports-and-caching)
are resolved against the URL of the document:

| In `https://example.com/docs/report.texl`, `texl.import(...)` of | fetches                                  |
|------------------------------------------------------------------|------------------------------------------|
| `"header.texl"`                                                  | `https://example.com/docs/header.texl`   |
| `"../lib/format.texl"`                                           | `https://example.com/lib/format.texl`    |
| `"/lib/format.texl"`                                             | `https://example.com/lib/format.texl`    |
| `"https://cdn.example.org/a.texl"`                               | `https://cdn.example.org/a.texl`         |

Documents are fetched with `fetch`, so the usual browser rules apply: a
document on another origin must allow it with CORS headers. A response with an
error status counts as a failed load. The promises of `fetch` and `load` then
reject with an `Error` whose `name` is `"LoadError"`, with the URL in `uri` and
the reason in `reason`, such as `"HTTP status 404"`:

```js
try {
    const doc = await engine.load("templates/report.texl");
} catch (err) {
    console.log(err.message);  // cannot load `https://example.com/templates/report.texl`: HTTP status 404
}
```

In a template, `texl.import` gives an [undefined value](#texlundefined) instead,
with the operation `"document.load"`, the URL in `lhv` and the reason in `rhv`.

## Rendering a source

`engine.render` renders a source that you already have, such as the text of an
editor, as the document at a URL, without fetching it. Its imports resolve
against that URL, as if the document had been loaded from there:

```js
const doc = await engine.render("docs/report.texl", editor.value);  // imports resolve in docs/
```

A source with no real location can use any URL, for example one in the folder
of the documents it imports. `engine.fetch` returns the source of a document
without rendering it, so `engine.load(uri)` is
`engine.render(uri, await engine.fetch(uri))`.

## Caching

The engine holds configuration only: its globals and loaders. Every `render`
and `load` starts from a new scope and a new import cache. Within one
rendering, each imported document is fetched and rendered once, however many
times it is imported; the next rendering fetches it again, so it sees the
changes to the documents. The browser's HTTP cache still applies to the
requests.

An imported document renders in its own scope: it sees the engine globals, but
not the names of the document that imports it.

## Globals and context values

The objects you pass to the engine and to contexts become Swan values:

| JavaScript                          | Swan                                           |
|-------------------------------------|------------------------------------------------|
| `null`, `undefined`                 | `Nothing`                                      |
| boolean                             | `Bool`                                         |
| number                              | `Numb`                                         |
| string                              | `Text`                                         |
| [`Tuple`](#tuple)                   | `Tuple`                                        |
| other array                         | `List` (see below)                             |
| Promise, or any object with a `then` method | `Promise`; a rejection resolves to an `Undefined` |
| function                            | `Func` (see below)                             |
| a [`TexlUndefined`](#texlundefined) | the original `Undefined`                       |
| other object                        | `Namespace` of its own enumerable properties   |

Values that come back to JavaScript, from `context.get`, from document data
and as function arguments, are converted the other way:

| Swan                     | JavaScript                                             |
|--------------------------|--------------------------------------------------------|
| `Nothing`                | `null`                                                 |
| `Bool`, `Numb`, `Text`   | boolean, number, string                                |
| `List`                   | array                                                  |
| `Tuple`                  | [`Tuple`](#tuple), an array subclass                   |
| `Namespace`              | plain object with its members                          |
| `Promise`                | `Promise`                                              |
| `Func`                   | `undefined`                                            |
| `Undefined`              | a [`TexlUndefined`](#texlundefined) object             |

Arrays and tuples are normalized, as in Swan: `null` and `undefined` items are
dropped and later items move down one position, `Tuple` items of an array are
spread into it, and a `Tuple` with one item is that item. So `[1, null, 2]`
becomes the list `[1, 2]`. Members of an object are not items: `{a: null}` keeps
`a`. To keep positions in data with gaps, pass an object rather than an array.
[`normalize`](#normalize) gives you the same result without passing the value
to the engine.

### Functions

A JavaScript function becomes a Swan function of one argument. A Swan call
with several arguments passes them as one [`Tuple`](#tuple), which is an
array, so destructure it:

```js
const engine = new TexlEngine({
    shout: (s) => s.toUpperCase() + "!",
    add: ([a, b]) => a + b,
});
(await engine.render("doc.texl", '<% shout "hi" %> <% add (2, 3) %>')).text;   // "HI! 5"
```

The return value is converted to a Swan value, so a function returns several
values as a `Tuple`, and a list as an array. If the function throws, the
result is an `Undefined` of type `"HostError"`, whose `error` is the text of
what was thrown, such as `"Error: boom"`; the thrown object itself is not kept.
An `async` function, or a function that returns a promise, gives a Swan
promise, and the tag waits for it; if it rejects, the promise resolves to the
same kind of `Undefined`:

```js
const engine = new TexlEngine({
    weather: async (city) => (await fetch(`/api/weather/${city}`)).text(),
});
await engine.render("doc.texl", 'Today in Rome: <% weather "rome" %>');
```

## Contexts

`engine.render` uses a new scope each time, so the names a template binds
are only in the `data` of the document it returns. A *context* keeps them: it
is a scope that you render several templates in, and that you can read and
write from JavaScript.

```js
const context = engine.createContext({ price: 12.5 });
await context.render("<% total = price * 4 %>");
context.get("total");                              // 50
(await context.render("Total: <% total %>")).text; // "Total: 50"
```

A context is not a document: its templates cannot import documents, and
`texl.import` is not defined in it. To render a source with imports, use
[`engine.render`](#rendering-a-source).

## Finding errors

An invalid operation does not stop the rendering: it gives an undefined
value, which renders as `[[Undefined]]`. The document tells you where each
one is. Its `tags` list has one [`TexlTag`](#texltag) per `<% ... %>` tag, in
order, including the tags that render nothing, such as assignments. Each tag
has its span in the source (`start`, `end`), the span of its text in the
output (`out_start`, `out_end`), and its `value`. An undefined value is a
[`TexlUndefined`](#texlundefined): it says what went wrong (`description`),
where (`uri`, `source`, `pos`) and why (`cause`, the operand that was
already undefined).

```js
import init, { TexlEngine, isUndefined } from "@onlabsorg/texl";
await init();
const engine = new TexlEngine({ price: 12.5 });
const source = 'Total: <% total = price * "4" %><% total %>';
const doc = await engine.render("cart.texl", source);
doc.text;                                      // "Total: [[Undefined]]"
for (const tag of doc.tags) {
    if (isUndefined(tag.value)) {
        console.log(source.slice(tag.start, tag.end), tag.value.description);
    }
}
// <% total %> cannot multiply the number `12.5` by the text `"4"`
const err = doc.data.total;
source.slice(err.pos, err.pos + 1);            // "*"
```

Positions are UTF-16 code units, so they index JavaScript strings directly.
`pos` is in the source of the code that made the value: for an error in a
function imported from another document, `uri` is that document's URL and
`source` its text, so a host can show the code around an error.

## Memory

A function defined in a template holds the scope it was defined in, so a
scope that stores one keeps itself alive: a reference cycle. The cycle
collector, [`collect`](#collect), frees the scopes that nothing outside such
cycles refers to any more. `engine.render` and `engine.load` run it when they
are done, so a page that only renders documents has nothing to do. After
freeing a context that defined functions, call `collect`:

```js
import { collect } from "@onlabsorg/texl";

context.free();
collect();
```

## API reference

### `init`

```ts
default function init(
    module_or_path?: { module_or_path: InitInput | Promise<InitInput> }
): Promise<InitOutput>
```

Loads and instantiates the WebAssembly module. Call it once, and wait for it,
before using any other export.

By default, it fetches `texl_wasm_bg.wasm` from the folder of
`texl_wasm.js`. Pass `{ module_or_path: url }` to load it from somewhere else.
The module also exports `initSync({ module })`, which instantiates an already
compiled module synchronously.

```js
import init from "@onlabsorg/texl";
await init();
```

### `TexlEngine`

An engine renders and loads documents. It holds configuration only: its
globals and loaders are shared by everything it renders, and nothing else is
(see [Caching](#caching)).

#### `new TexlEngine(globals, baseUrl)`

```ts
constructor(globals?: object | null, baseUrl?: string | null)
```

Creates an engine.

| Argument  | Description                                                                 |
|-----------|-----------------------------------------------------------------------------|
| `globals` | An object whose properties become globals of every template and document. Optional. |
| `baseUrl` | The URL that relative URLs resolve against. Optional; defaults to `location.href`, the URL of the page or worker. |

Throws a string with an error message if `baseUrl` is not an absolute URL.

```js
const engine = new TexlEngine(
    { site: { name: "My site" } },
    "https://example.com/templates/",
);
```

#### `engine.baseUrl`

```ts
readonly baseUrl: string | undefined
```

Returns the base URL of the engine, normalized. It is `undefined` only when no
base URL was given and the global scope has no `location`.

```js
new TexlEngine().baseUrl;                      // the URL of the page
```

#### `engine.set(name, value)`

```ts
set(name: string, value: any): void
```

Binds a global, visible to every template and document of the engine,
from now on. Contexts created earlier do not see it. A global with the same
name is replaced.

| Argument | Description                                             |
|----------|---------------------------------------------------------|
| `name`   | The name of the global.                                 |
| `value`  | Its value, [converted](#globals-and-context-values) to a Swan value. |

```js
engine.set("today", new Date().toISOString().slice(0, 10));
```

#### `engine.fetch(uri)`

```ts
fetch(uri: string): Promise<string>
```

Fetches the source of the document at `uri`, without rendering it. Nothing is
cached: every call fetches again.

| Argument | Description                                         |
|----------|-----------------------------------------------------|
| `uri`    | The URL of the document, resolved against `baseUrl`. |

Returns a promise of the source. The promise rejects with an `Error` if `uri`
cannot be resolved to a URL, or with a [`LoadError`](#loading-documents) if the
document cannot be fetched.

```js
const source = await engine.fetch("templates/report.texl");
```

#### `engine.render(uri, source)`

```ts
render(uri: string, source: string): Promise<TexlDocument>
```

Renders `source` as the document at `uri`, in a new scope, without fetching
it. Imports in `source` resolve against the URL of `uri`; an import of that
URL itself gives `Undefined` (an [import cycle](../language.md#import-cycles)).

| Argument | Description                                         |
|----------|-----------------------------------------------------|
| `uri`    | The URL of the document, resolved against `baseUrl`. |
| `source` | The template source.                                |

Returns a promise of the [document](#texldocument). The promise rejects with an
`Error` only if `uri` cannot be resolved to a URL.

```js
const doc = await engine.render("draft.texl", "2 + 2 = <% 2 + 2 %>");
doc.text;                                 // "2 + 2 = 4"
```

#### `engine.load(uri)`

```ts
load(uri: string): Promise<TexlDocument>
```

Fetches and renders the document at `uri`: [`fetch`](#enginefetchuri)
followed by [`render`](#enginerenderuri-source).

| Argument | Description                                         |
|----------|-----------------------------------------------------|
| `uri`    | The URL of the document, resolved against `baseUrl`. |

Returns a promise of the [document](#texldocument). The promise rejects with an
`Error` if `uri` cannot be resolved to a URL, or with a
[`LoadError`](#loading-documents) if the document cannot be loaded.

```js
const doc = await engine.load("templates/report.texl");
console.log(doc.text);
```

#### `engine.createContext(vars)`

```ts
createContext(vars?: object | null): TexlContext
```

Creates a [context](#texlcontext): a scope that keeps its names across
renders. Templates in a context cannot import documents.

| Argument | Description                                                         |
|----------|---------------------------------------------------------------------|
| `vars`   | An object whose properties are bound in the context. Optional.      |

```js
const context = engine.createContext({ user: "Ada" });
```

#### `engine.free()`

```ts
free(): void
```

Releases the memory of the engine. The engine cannot be used afterwards. You
only need it for engines that you create and discard many times; the engine
also supports `using` declarations through `Symbol.dispose`.

### `TexlContext`

A scope of an engine, created with `engine.createContext`. Templates rendered
in a context share its names, and so do `get` and `set`. The context also sees
the globals of its engine. It cannot import documents.

#### `context.get(name)`

```ts
get(name: string): any
```

Returns the value of a name, [converted](#globals-and-context-values) to a
JavaScript value. The name is looked up in the context, then in the engine
globals and the built-ins. A name that is not bound gives `null`.

| Argument | Description            |
|----------|------------------------|
| `name`   | The name to look up.   |

```js
await context.render("<% x = (1, 2) %>");
context.get("x");                              // [1, 2]
```

#### `context.set(name, value)`

```ts
set(name: string, value: any): void
```

Binds a name in the context, replacing any previous value.

| Argument | Description                                             |
|----------|---------------------------------------------------------|
| `name`   | The name to bind.                                       |
| `value`  | Its value, [converted](#globals-and-context-values) to a Swan value. |

```js
context.set("items", ["a", "b"]);
```

#### `context.names()`

```ts
names(): string[]
```

Returns the names bound in the context, sorted. Engine globals and built-ins
are not included. The list always includes `texl`, the engine's namespace,
which has no `import` in a context.

```js
engine.createContext({ b: 1, a: 2 }).names();  // ["a", "b", "texl"]
```

#### `context.render(template)`

```ts
render(template: string): Promise<TexlRendering>
```

Renders a template in the context, and returns a promise of the
[rendering](#texlrendering): the text, the names of the context and the tags.
The names the template binds stay in the context.

| Argument   | Description            |
|------------|------------------------|
| `template` | The template source.   |

```js
await context.render("<% count = 1 %>");
const rendering = await context.render("Count: <% count %>");
rendering.text;                                // "Count: 1"
rendering.tags;                                // [{start: 7, end: 18, out_start: 7, out_end: 8, value: 1}]
```

#### `context.free()`

```ts
free(): void
```

Releases the memory of the context, like [`engine.free`](#enginefree).
Scopes the context made reference cycles with stay until the next
[`collect`](#collect).

### `collect`

```ts
function collect(): number
```

Frees the scopes and values of templates that only reference cycles keep
alive (see [Memory](#memory)), for every engine of the page or worker.
Returns the number of scopes and promises it cleared.

```js
import { collect } from "@onlabsorg/texl";
collect();    // 0 when there is nothing to free
```

### `isUndefined`

```ts
function isUndefined(value: any): value is TexlUndefined
```

Tells whether a value is a [`TexlUndefined`](#texlundefined) that came from
Swan. A copy of one (`{...u}`) is not.

```js
import { isUndefined } from "@onlabsorg/texl";
isUndefined(doc.tags[0].value);    // true if the first tag gave an error
```

### `normalize`

```ts
function normalize(value: any): any
```

Gives a value the form Swan would give it, without passing it to the engine. A
`Tuple` is flattened and loses its `null` and `undefined` items; with one item
left it is that item, with none it is `null`. An array spreads its `Tuple` items
and loses its `null` and `undefined` items. This goes through nested arrays and
plain objects, whose members are kept even when `null`. Anything else, such as
a function, a promise or a class instance, is returned as it is. The value
passed is not changed.

```js
import { Tuple, normalize } from "@onlabsorg/texl";
normalize([1, null, Tuple.of(2, 3), [null]]);          // [1, 2, 3, []]
normalize(Tuple.of(1, Tuple.of(2, null), null));       // Tuple [1, 2]
normalize(Tuple.of(null, 7));                          // 7
normalize({ a: [null, 1], n: null });                  // {a: [1], n: null}
```

### `Tuple`

```ts
class Tuple<T = any> extends Array<T> {}
```

A Swan tuple in JavaScript. It is an `Array` with nothing added, so it reads
like any array, and `instanceof Tuple` tells it from a list, which is a plain
array. Tuples from Swan arrive as `Tuple` objects, and a `Tuple` you pass to
the engine becomes a Swan tuple.

```js
import { Tuple } from "@onlabsorg/texl";
const context = engine.createContext();
context.set("pair", Tuple.of(1, 2));
(await context.render("<% pair + pair %>")).text;      // "2, 4"
context.get("pair") instanceof Tuple;                  // true
```

Swan tuples are flat and never hold `Nothing`, so a `Tuple` is normalized when
it reaches Swan: nested tuples are flattened, `null` and `undefined` items are
dropped, one item gives that item and no items give `Nothing`.

```js
context.set("flat", Tuple.of(1, Tuple.of(2, 3), null));
context.get("flat");                                   // Tuple [1, 2, 3]
context.set("one", Tuple.of(7));
context.get("one");                                    // 7
```

Create tuples with `Tuple.of` or `Tuple.from`: as with `Array`, `new Tuple(3)`
is three empty slots, not a tuple holding 3. Array methods that make a new
array, such as `map` and `filter`, return a `Tuple` when called on one.

### `TexlDocument`

```ts
interface TexlDocument {
    uri: string;
    source: string;
    data: Record<string, any>;
    text: string;
    tags: TexlTag[];
}
```

A rendered document, as returned by `engine.load` and `engine.render`.

| Property    | Description                                                        |
|-------------|--------------------------------------------------------------------|
| `uri`       | The URL of the document.                                           |
| `source`    | The template source.                                               |
| `data`      | The names the document's tags bound, [converted](#globals-and-context-values) to JavaScript values. Functions are `undefined`. |
| `text`      | The rendered text.                                                 |
| `tags`      | The [tags](#texltag) of the source, in order, including the tags that render nothing. |

```js
const doc = await engine.load("settings.texl");
console.log(doc.data.title, doc.text);
```

An imported document, in `data` or in a tag value, has the same members.

### `TexlRendering`

```ts
interface TexlRendering {
    text: string;
    data: Record<string, any>;
    tags: TexlTag[];
}
```

A template rendered in a context, as returned by
[`context.render`](#contextrendertemplate).

| Property | Description                                                          |
|----------|----------------------------------------------------------------------|
| `text`   | The rendered text.                                                   |
| `data`   | The names bound in the context when the rendering ended, including those bound before it. |
| `tags`   | The [tags](#texltag) of the template, in order, including the tags that render nothing. |

### `TexlTag`

```ts
interface TexlTag {
    start: number;
    end: number;
    out_start: number;
    out_end: number;
    value: any;
}
```

A `<% ... %>` tag of a rendered template. Positions are UTF-16 code units:
`source.slice(tag.start, tag.end)` is the tag, and
`text.slice(tag.out_start, tag.out_end)` is what it rendered.

| Property    | Description                                                    |
|-------------|----------------------------------------------------------------|
| `start`     | Where the tag starts in the source (at `<%`).                  |
| `end`       | Where the tag ends in the source (after `%>`).                 |
| `out_start` | Where the text of the tag starts in the rendered text.         |
| `out_end`   | Where the text of the tag ends in the rendered text.           |
| `value`     | The value that was rendered. If the expression returned a promise, it is the resolved value; promises nested in the value, such as tuple items, are not awaited. An error is a [`TexlUndefined`](#texlundefined), also when the tag renders nothing. |

A document keeps the values of all its tags as long as it lives.

### `TexlUndefined`

```ts
interface TexlUndefined {
    readonly type: string;
    readonly description: string;
    readonly op: string | null;
    readonly val: any;
    readonly lhv: any;
    readonly rhv: any;
    readonly expected: string | null;
    readonly token: string | null;
    readonly error: string | null;
    readonly pos: number | null;
    readonly source: string | null;
    readonly uri: string | null;
    readonly cause: TexlUndefined | null;
}
```

A Swan undefined value: the result of an invalid operation, such as
`"a" - 1`. It is a frozen plain object, with the members of `undef.inspect`
in Swan and `source`; those that do not apply are `null`. Everything but
`source` is converted when the object is made. Passing it back to Swan (with
`context.set`, or as the result of a JavaScript function) gives the original
undefined value.

| Property      | Description                                                  |
|---------------|--------------------------------------------------------------|
| `type`        | The kind of error (see the table below).                     |
| `description` | The error in plain words, such as ``cannot add the text `"a"` and the number `1` ``. |
| `op`          | The name of the failed operation, such as `"subtraction"`.   |
| `val`         | The operand of a unary operation, or the value of `undef.from`. |
| `lhv`, `rhv`  | The operands of a binary operation.                          |
| `expected`    | The missing closing bracket of a `"Termination"` error.      |
| `token`       | The misplaced token of a `"Token"` error.                    |
| `error`       | The text of what a host function threw or a host promise rejected with, such as `"Error: boom"`, of a `"HostError"`. |
| `pos`         | Where the error is in the source of the code that made it, in UTF-16 code units; for an error of a built-in or host function, the call that returned it. `null` for values made by `undef.from` and when the error has no known place in any source. |
| `source`      | The text `pos` is an index of: the whole document, or the code run by `parse`. `null` when `pos` is. A getter: the string is made each time it is read, so keep it in a variable to read it many times. |
| `uri`         | The URL of the document the error is in. `null` when `pos` is, and for templates rendered in a context. |
| `cause`       | The first operand (or tuple item) that is itself undefined: following `cause` leads to the root cause. |

| `type`              | Meaning                                          |
|---------------------|--------------------------------------------------|
| `"UnaryOperation"`  | operation `op` failed on the operand `val`       |
| `"BinaryOperation"` | operation `op` failed on `lhv` and `rhv`         |
| `"Termination"`     | the closing bracket `expected` is missing        |
| `"Token"`           | a token is in the wrong place                    |
| `"Name"`            | a name was expected                              |
| `"Value"`           | `val` wrapped by `undef.from`                    |
| `"HostError"`       | a host function threw, or a host promise rejected, with `error` |

```js
const doc = await engine.render("doc.texl", '<% 2 * ("a" - 1) %>');
const err = doc.tags[0].value;
err.description;          // "cannot multiply the number `2` by an undefined value"
err.cause.description;    // "cannot subtract the number `1` from the text `\"a\"`"
```
