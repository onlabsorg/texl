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

The package contains the module `texl_wasm.js`, the WebAssembly binary
`texl_wasm_bg.wasm`, and the TypeScript declarations `texl_wasm.d.ts`.

Without a bundler, import the module from a CDN that serves npm packages, with
the version you want:

```js
import init, { TexlEngine } from "https://cdn.jsdelivr.net/npm/@onlabsorg/texl@0.4.0/texl_wasm.js";
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
import init, { TexlEngine } from "https://cdn.jsdelivr.net/npm/@onlabsorg/texl@0.4.0/texl_wasm.js";

await init();                                  // load the WebAssembly module

const engine = new TexlEngine({ user: { name: "Ada" } });
const doc = await engine.render("hello.texl", "Hello <% user.name %>!");
console.log(doc.rendering);                    // Hello Ada!
</script>
```

The page must be served over HTTP: browsers do not load WebAssembly modules,
or let them fetch documents, from `file://` pages.

## Loading documents

`engine.load` fetches a document by URL, renders it, and returns its
rendering, source and data:

```js
const doc = await engine.load("templates/report.texl");
if (doc) {
    document.querySelector("#report").textContent = doc.rendering;
}
```

A relative URL is resolved against the engine's base URL, which is the URL of
the page by default. Inside a document, [imports](../templates/documents.md)
are resolved against the URL of the document:

| In `https://example.com/docs/report.texl`, `texl.import(...)` of | fetches                                  |
|------------------------------------------------------------------|------------------------------------------|
| `"header.texl"`                                                  | `https://example.com/docs/header.texl`   |
| `"../lib/format.texl"`                                           | `https://example.com/lib/format.texl`    |
| `"/lib/format.texl"`                                             | `https://example.com/lib/format.texl`    |
| `"https://cdn.example.org/a.texl"`                               | `https://cdn.example.org/a.texl`         |

Documents are fetched with `fetch`, so the usual browser rules apply: a
document on another origin must allow it with CORS headers. A response with an
error status counts as a failed load: `load` gives `null`, and `texl.import`
gives `Nothing`.

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
| array                               | `List`                                         |
| Promise, or any object with a `then` method | `Promise`; a rejection resolves to `Nothing` |
| function                            | `Func` (see below)                             |
| other object                        | `Namespace` of its own enumerable properties   |

Values that come back to JavaScript, from `context.get`, from document data
and as function arguments, are converted the other way:

| Swan                     | JavaScript                                             |
|--------------------------|--------------------------------------------------------|
| `Nothing`                | `null`                                                 |
| `Bool`, `Numb`, `Text`   | boolean, number, string                                |
| `List`, `Tuple`          | array                                                  |
| `Namespace`              | plain object with its members                          |
| `Promise`                | `Promise`                                              |
| `Func`, `Undefined`      | `undefined`                                            |

### Functions

A JavaScript function becomes a Swan function of one argument. A Swan call
with several arguments passes them as one array, so destructure it:

```js
const engine = new TexlEngine({
    shout: (s) => s.toUpperCase() + "!",
    add: ([a, b]) => a + b,
});
(await engine.render("doc.texl", '<% shout "hi" %> <% add (2, 3) %>')).rendering;   // "HI! 5"
```

The return value is converted to a Swan value. If the function throws, the
result is `Nothing`. An `async` function, or a function that returns a
promise, gives a Swan promise, and the tag waits for it:

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
context.get("total");                          // 50
await context.render("Total: <% total %>");    // "Total: 50"
```

A context is not a document: its templates cannot import documents, and
`texl.import` is not defined in it. To render a source with imports, use
[`engine.render`](#rendering-a-source).

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
fetch(uri: string): Promise<string | null>
```

Fetches the source of the document at `uri`, without rendering it. Nothing is
cached: every call fetches again.

| Argument | Description                                         |
|----------|-----------------------------------------------------|
| `uri`    | The URL of the document, resolved against `baseUrl`. |

Returns a promise of the source, or of `null` if it cannot be fetched. The
promise rejects, with a string message, only if `uri` cannot be resolved to a
URL.

```js
const source = await engine.fetch("templates/report.texl");
```

#### `engine.render(uri, source)`

```ts
render(uri: string, source: string): Promise<TexlDocument>
```

Renders `source` as the document at `uri`, in a new scope, without fetching
it. Imports in `source` resolve against the URL of `uri`; an import of that
URL itself gives `Undefined` (an [import cycle](../templates/documents.md#import-cycles)).

| Argument | Description                                         |
|----------|-----------------------------------------------------|
| `uri`    | The URL of the document, resolved against `baseUrl`. |
| `source` | The template source.                                |

Returns a promise of the [document](#texldocument). The promise rejects, with
a string message, only if `uri` cannot be resolved to a URL.

```js
const doc = await engine.render("draft.texl", "2 + 2 = <% 2 + 2 %>");
doc.rendering;                                 // "2 + 2 = 4"
```

#### `engine.load(uri)`

```ts
load(uri: string): Promise<TexlDocument | null>
```

Fetches and renders the document at `uri`: [`fetch`](#enginefetchuri)
followed by [`render`](#enginerenderuri-source).

| Argument | Description                                         |
|----------|-----------------------------------------------------|
| `uri`    | The URL of the document, resolved against `baseUrl`. |

Returns a promise of the [document](#texldocument), or of `null` if it cannot
be loaded. The promise rejects, with a string message, only if `uri` cannot be
resolved to a URL.

```js
const doc = await engine.load("templates/report.texl");
console.log(doc?.rendering ?? "not found");
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
render(template: string): Promise<string>
```

Renders a template in the context, and returns a promise of the rendered text.
The names the template binds stay in the context.

| Argument   | Description            |
|------------|------------------------|
| `template` | The template source.   |

```js
await context.render("<% count = 1 %>");
await context.render("Count: <% count %>");    // "Count: 1"
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

### `TexlDocument`

```ts
interface TexlDocument {
    uri: string;
    source: string;
    data: Record<string, any>;
    rendering: string;
}
```

A rendered document, as returned by `engine.load` and `engine.render`.

| Property    | Description                                                        |
|-------------|--------------------------------------------------------------------|
| `uri`       | The URL of the document.                                           |
| `source`    | The template source.                                               |
| `data`      | The names the document's tags bound, [converted](#globals-and-context-values) to JavaScript values. Functions are `undefined`. |
| `rendering` | The rendered text.                                                 |

```js
const doc = await engine.load("settings.texl");
console.log(doc.data.title, doc.rendering);
```
