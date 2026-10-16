# texl-wasm

The JavaScript library of [Texl](../README.md) for web pages and web workers,
compiled to WebAssembly.

```sh
wasm-pack build texl-wasm --target web     # creates texl-wasm/pkg
```

```js
import init, { TexlEngine } from "./pkg/texl_wasm.js";
await init();
const engine = new TexlEngine({ user: "Ada" });
await engine.render("Hi <% user %>!");     // "Hi Ada!"
```

See the [JavaScript API reference](https://onlabsorg.github.io/texl/book/hosts/javascript.html).
The doc comments in `src/lib.rs` are also copied into `pkg/texl_wasm.d.ts`.

## Tests

```sh
cargo test -p texl-wasm --test browser     # runs tests/web/index.html in headless Chrome
cargo run -p texl-wasm --example serve     # serves the test page to inspect it
```

## License

[MIT](../LICENSE)
