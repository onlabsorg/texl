# A live preview in a web page

This example builds a small page with an editor and a live preview: every
time the text changes, the page renders it with the JavaScript library and
shows the result. It uses `@onlabsorg/texl` from a CDN, so it needs no build
step. The [JavaScript guide](../api/javascript.md) documents the API.

## The files

```text
index.html
docs/
    signature.texl
```

`docs/signature.texl` is a partial that the edited text can import:

```texl
--
<% user.name %>, <% user.role %>
```

## The page

```html
<!doctype html>
<meta charset="utf-8">
<title>Texl preview</title>
<textarea id="source" rows="8" cols="60">Hello <% user.name %>!
<% texl.import("signature.texl") ~> (doc -> doc.rendering) %></textarea>
<pre id="output"></pre>

<script type="module">
import init, { TexlEngine } from "https://cdn.jsdelivr.net/npm/@onlabsorg/texl@0.4.0/texl_wasm.js";

await init();
const engine = new TexlEngine({ user: { name: "Ada", role: "editor" } });

const source = document.querySelector("#source");
const output = document.querySelector("#output");
let latest = 0;

async function preview() {
    const run = ++latest;
    const doc = await engine.render("docs/draft.texl", source.value);
    if (run === latest) {                      // drop the results of older runs
        output.textContent = doc.rendering;
    }
}

source.addEventListener("input", preview);
preview();
</script>
```

Serve the folder over HTTP, for example with `python -m http.server`, and
open `index.html`: browsers do not run WebAssembly modules, or let them fetch
documents, from `file://` pages.

## How it works

- **The document's URL.** `engine.render("docs/draft.texl", text)` renders the
  text as if it were the document `docs/draft.texl`, resolved against the URL
  of the page. Its imports resolve against that URL, so
  `texl.import("signature.texl")` fetches `docs/signature.texl`. The draft
  itself does not need to exist on the server.
- **Globals.** The object passed to `new TexlEngine` becomes the template's
  globals: `user` is a Swan namespace.
- **Out-of-order results.** Renderings are asynchronous, and one that waits for
  an import can finish after a later one. The `latest` counter keeps only the
  newest result.
- **No cache.** Every rendering fetches its imports again, so the preview
  shows changes to `signature.texl` as soon as the text is edited. The
  browser's HTTP cache still applies. To fetch less often, delay the preview
  until typing pauses.
- **Errors.** A mistake in the text never throws: the preview shows
  `[[Undefined]]` where a tag is invalid, and a missing partial gives
  `Nothing`. `engine.render` rejects only if the URL cannot be resolved.
- **Memory.** `engine.render` frees what the rendering no longer needs when
  it is done, so the page does not grow as you type. A page that keeps
  contexts frees them with `context.free()` and calls `collect()` (see
  [Memory](../api/javascript.md#memory)).
