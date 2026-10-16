# JavaScript

The `texl-wasm` crate compiles the Texl engine to WebAssembly, with a
JavaScript API for web pages and web workers. It renders templates from
strings, loads documents over HTTP, and lets templates call your JavaScript
functions.

A package for Node.js is coming soon.

## Building the package

The package is not published on npm yet. Build it from a clone of the
repository with [wasm-pack](https://rustwasm.github.io/wasm-pack/):

```sh
wasm-pack build texl-wasm --target web
```

This creates `texl-wasm/pkg`, with the module `texl_wasm.js`, the WebAssembly
binary `texl_wasm_bg.wasm`, and the TypeScript declarations `texl_wasm.d.ts`.
Copy the folder to your web site.

## Quick start

```html
<script type="module">
import init, { TexlEngine } from "./pkg/texl_wasm.js";

await init();                                  // load the WebAssembly module

const engine = new TexlEngine({ user: { name: "Ada" } });
const text = await engine.render("Hello <% user.name %>!");
console.log(text);                             // Hello Ada!
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

Templates rendered with `engine.render` or with a context have no URL of
their own: their imports are resolved against the root of the base URL, so
`"lib/format.texl"` and `"/lib/format.texl"` both fetch
`https://example.com/lib/format.texl`.

Documents are fetched with `fetch`, so the usual browser rules apply: a
document on another origin must allow it with CORS headers. A response with an
error status counts as a failed load: `load` gives `null`, and `texl.import`
gives `Nothing`.

Each engine loads and renders each document once, and reuses the result for
later loads and imports. Create a new engine to see changes to the documents.

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
await engine.render('<% shout "hi" %> <% add (2, 3) %>');   // "HI! 5"
```

The return value is converted to a Swan value. If the function throws, the
result is `Nothing`. An `async` function, or a function that returns a
promise, gives a Swan promise, and the tag waits for it:

```js
const engine = new TexlEngine({
    weather: async (city) => (await fetch(`/api/weather/${city}`)).text(),
});
await engine.render('Today in Rome: <% weather "rome" %>');
```

## Contexts

`engine.render` uses a fresh scope each time, so the names a template binds
are lost when it ends. A *context* keeps them: it is a scope that you render
several templates in, and that you can read and write from JavaScript.

```js
const context = engine.createContext({ price: 12.5 });
await context.render("<% total = price * 4 %>");
context.get("total");                          // 50
await context.render("Total: <% total %>");    // "Total: 50"
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
import init from "./pkg/texl_wasm.js";
await init();
```

### `TexlEngine`

An engine renders templates and loads documents. Its globals, documents and
engine-wide names (bound with [`bind`](../swan/builtins/root.md#bind)) are
shared by everything it renders.

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
including contexts created earlier. A global with the same name is replaced.

| Argument | Description                                             |
|----------|---------------------------------------------------------|
| `name`   | The name of the global.                                 |
| `value`  | Its value, [converted](#globals-and-context-values) to a Swan value. |

```js
engine.set("today", new Date().toISOString().slice(0, 10));
```

#### `engine.load(uri)`

```ts
load(uri: string): Promise<TexlDocument | null>
```

Loads and renders the document at `uri`.

| Argument | Description                                         |
|----------|-----------------------------------------------------|
| `uri`    | The URL of the document, resolved against `baseUrl`. |

Returns a promise of the [document](#texldocument), or of `null` if it cannot
be loaded. The promise rejects, with a string message, only if `uri` cannot be
resolved to a URL. Each document is loaded once per engine; later calls with
the same URL return the same result.

```js
const doc = await engine.load("templates/report.texl");
console.log(doc?.rendering ?? "not found");
```

#### `engine.render(template)`

```ts
render(template: string): Promise<string>
```

Renders a template in a fresh scope, and returns a promise of the rendered
text. The names the template binds with `=` or `:` are discarded afterwards.

| Argument   | Description            |
|------------|------------------------|
| `template` | The template source.   |

```js
await engine.render("2 + 2 = <% 2 + 2 %>");    // "2 + 2 = 4"
```

#### `engine.createContext(vars)`

```ts
createContext(vars?: object | null): TexlContext
```

Creates a [context](#texlcontext): a scope that keeps its names across
renders.

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
the globals of its engine.

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
are not included. The list always includes `texl`, the namespace that holds
`texl.import`.

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

### `TexlDocument`

```ts
interface TexlDocument {
    uri: string;
    source: string;
    data: Record<string, any>;
    rendering: string;
}
```

A loaded document, as returned by `engine.load`.

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
