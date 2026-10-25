# texl-wasm

The JavaScript library of [Texl](https://github.com/onlabsorg/texl) for web
pages and web workers, compiled to WebAssembly. It is published on npm as
[`@onlabsorg/texl`](https://www.npmjs.com/package/@onlabsorg/texl).

```sh
npm install @onlabsorg/texl
```

```js
import init, { TexlEngine } from "@onlabsorg/texl";
await init();
const engine = new TexlEngine({ user: "Ada" });
const doc = await engine.render("hi.texl", "Hi <% user %>!");
doc.rendering;                            // "Hi Ada!"
```

See the [JavaScript API reference](https://onlabsorg.github.io/texl/book/hosts/javascript.html).
The doc comments in `src/lib.rs` are also copied into `texl_wasm.d.ts`.

## Building

```sh
wasm-pack build texl-wasm --target web     # creates texl-wasm/pkg
```

## Tests

```sh
cargo test -p texl-wasm --test browser     # runs tests/web/index.html in headless Chrome
cargo run -p texl-wasm --example serve     # serves the test page to inspect it
```

## Releases

Pushing a `texl-wasm-v<version>` tag builds and tests the package, and
publishes it to npm (see `.github/workflows/texl-wasm.yml`).

## License

[MIT](https://github.com/onlabsorg/texl/blob/main/LICENSE)
