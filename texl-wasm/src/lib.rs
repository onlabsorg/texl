//! Web bindings of the texl engine.
//!
//! This crate compiles to a WebAssembly module with JavaScript bindings for
//! browsers and web workers. Its Rust items are the implementation of the
//! JavaScript API: the doc comments below are written for JavaScript readers,
//! and wasm-bindgen copies them into the generated `texl_wasm.d.ts`.
//!
//! ```js
//! import init, { TexlEngine } from "./pkg/texl_wasm.js";
//! await init();
//! const engine = new TexlEngine({ reader: "Ada" });          // globals, optional base URL
//! const doc = await engine.load("docs/main.texl");           // {uri, source, data, rendering} or null
//! const draft = await engine.render("docs/draft.texl", "Hello <% reader %>!");
//! ```
//!
//! See the [JavaScript API reference](https://onlabsorg.github.io/texl/book/hosts/javascript.html).

mod convert;
mod loader;

use js_sys::{Array, Object, Reflect};
use swan_core::value::{BuiltinFn, Scope, Value};
use texl_core::helpers::spawner_fn;
use texl_core::{Engine, loader_fn, template};
use url::Url;
use wasm_bindgen::prelude::*;

use convert::{for_each_entry, js_to_swan_value, swan_value_to_js};

#[wasm_bindgen(typescript_custom_section)]
const TS_TYPES: &'static str = r#"
/**
 * A rendered document, as returned by `TexlEngine.load` and `TexlEngine.render`.
 */
export interface TexlDocument {
    /** The URL of the document. */
    uri: string;
    /** The template source of the document. */
    source: string;
    /** The names the template assigned, converted to JavaScript values. */
    data: Record<string, any>;
    /** The rendered text. */
    rendering: string;
}
"#;

/// A texl engine that renders documents and loads them over HTTP(S).
///
/// Every document has a URL: the URIs given to `fetch`, `render` and `load` resolve
/// against `baseUrl`. Documents with an `http://` or `https://` URL are fetched as they
/// are. Imports resolve against the importing document: relative paths against its
/// folder, absolute paths (`/dir/doc.texl`) against its root folder.
///
/// The engine holds configuration only: globals and loaders. Every `render` and `load`
/// starts from a new scope and a new import cache: within one rendering, each imported
/// document is fetched and rendered once; the next rendering fetches it again. An imported
/// document renders in its own scope, and does not see the names of its importer.
#[wasm_bindgen]
pub struct TexlEngine {
    engine: Engine,
    base_url: Option<Url>,
}

#[wasm_bindgen]
impl TexlEngine {
    /// Creates an engine.
    ///
    /// Throws if `baseUrl` is not a valid absolute URL.
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(param_description = "Values bound as globals in every template and document. Defaults to none.")]
        globals: Option<Object>,
        #[wasm_bindgen(
            js_name = baseUrl,
            param_description = "The URL that relative URIs resolve against. Defaults to the URL of the current page or worker script."
        )]
        base_url: Option<String>,
    ) -> Result<TexlEngine, JsValue> {
        let base_url = match base_url.or_else(location_href) {
            Some(href) => Some(
                Url::parse(&href).map_err(|e| JsValue::from_str(&format!("invalid base URL {href}: {e}")))?,
            ),
            None => None,
        };

        let engine = Engine::new(Some(spawner_fn(wasm_bindgen_futures::spawn_local)));
        engine.add_loader("http", loader_fn(loader::load_http));
        engine.add_loader("https", loader_fn(loader::load_http));
        use_js_clock(&engine);

        if let Some(globals) = globals {
            for_each_entry(&globals, |name, value| engine.add_builtin(&name, value))?;
        }
        Ok(TexlEngine { engine, base_url })
    }

    /// The URL that relative document URIs and absolute paths resolve against,
    /// or `undefined` if there is none.
    #[wasm_bindgen(getter, js_name = baseUrl)]
    pub fn base_url(&self) -> Option<String> {
        self.base_url.as_ref().map(Url::to_string)
    }

    /// Binds a global, visible to every template and document of this engine rendered
    /// from now on. Contexts created earlier do not see it.
    pub fn set(
        &self,
        #[wasm_bindgen(param_description = "The name of the global.")]
        name: &str,
        #[wasm_bindgen(param_description = "Its value, converted to a Swan value.")]
        value: JsValue,
    ) -> Result<(), JsValue> {
        self.engine.add_builtin(name, js_to_swan_value(value)?);
        Ok(())
    }

    /// Fetches and renders the document at `uri`: `fetch` followed by `render`.
    ///
    /// The promise rejects only if `uri` cannot be resolved to a URL.
    #[wasm_bindgen(
        unchecked_return_type = "Promise<TexlDocument | null>",
        return_description = "The document, or `null` if it cannot be loaded."
    )]
    pub fn load(
        &self,
        #[wasm_bindgen(param_description = "The URL of the document, resolved against `baseUrl`.")]
        uri: &str,
    ) -> js_sys::Promise {
        let engine = self.engine.clone();
        let url = loader::resolve_document_url(uri, self.base_url.as_ref());
        wasm_bindgen_futures::future_to_promise(async move {
            let url = url.map_err(|e| JsValue::from_str(&e))?;
            Ok(swan_value_to_js(&engine.load(&url).await))
        })
    }

    /// Renders `source` as the document at `uri`, without fetching it.
    ///
    /// Imports in `source` resolve against the URL of `uri`, and an import of that URL
    /// itself gives `undefined` (a cycle). A source with no real location, such as an
    /// unsaved buffer, can use any URL, for example one next to the documents it imports.
    ///
    /// The promise rejects only if `uri` cannot be resolved to a URL.
    #[wasm_bindgen(unchecked_return_type = "Promise<TexlDocument>", return_description = "The document.")]
    pub fn render(
        &self,
        #[wasm_bindgen(param_description = "The URL of the document, resolved against `baseUrl`.")]
        uri: &str,
        #[wasm_bindgen(param_description = "The template source.")]
        source: String,
    ) -> js_sys::Promise {
        let engine = self.engine.clone();
        let url = loader::resolve_document_url(uri, self.base_url.as_ref());
        wasm_bindgen_futures::future_to_promise(async move {
            let url = url.map_err(|e| JsValue::from_str(&e))?;
            Ok(swan_value_to_js(&engine.render(&url, &source).await))
        })
    }

    /// Fetches the source of the document at `uri`, without rendering it. Nothing is
    /// cached: every call fetches again.
    ///
    /// The promise rejects only if `uri` cannot be resolved to a URL.
    #[wasm_bindgen(
        unchecked_return_type = "Promise<string | null>",
        return_description = "The source, or `null` if it cannot be fetched."
    )]
    pub fn fetch(
        &self,
        #[wasm_bindgen(param_description = "The URL of the document, resolved against `baseUrl`.")]
        uri: &str,
    ) -> js_sys::Promise {
        let engine = self.engine.clone();
        let url = loader::resolve_document_url(uri, self.base_url.as_ref());
        wasm_bindgen_futures::future_to_promise(async move {
            let url = url.map_err(|e| JsValue::from_str(&e))?;
            Ok(engine.fetch(&url).await.map_or(JsValue::NULL, |source| JsValue::from_str(&source)))
        })
    }

    /// Creates a context: a scope that keeps its bindings across renders.
    ///
    /// A context is not a document: its templates cannot import documents (`texl.import`
    /// is not defined in it). Use `render` or `load` to render documents with imports.
    #[wasm_bindgen(js_name = createContext)]
    pub fn create_context(
        &self,
        #[wasm_bindgen(param_description = "Values bound in the context. Defaults to none.")]
        vars: Option<Object>,
    ) -> Result<TexlContext, JsValue> {
        let scope = self.engine.create_scope();
        if let Some(vars) = vars {
            for_each_entry(&vars, |name, value| scope.set(name, value))?;
        }
        Ok(TexlContext { scope })
    }
}

/// A scope of a `TexlEngine` whose bindings persist across renders: names
/// assigned by a template can be read by later templates and by JavaScript.
///
/// Create one with `TexlEngine.createContext`.
#[wasm_bindgen]
pub struct TexlContext {
    scope: Scope,
}

#[wasm_bindgen]
impl TexlContext {
    /// Returns the value of a name, converted to a JavaScript value.
    ///
    /// The name is looked up in the context, then in the engine globals and the
    /// built-ins. A name that is not bound gives `null`.
    pub fn get(
        &self,
        #[wasm_bindgen(param_description = "The name to look up.")]
        key: &str,
    ) -> JsValue {
        swan_value_to_js(&self.scope.get(key))
    }

    /// Binds a name in the context, replacing any previous value.
    pub fn set(
        &self,
        #[wasm_bindgen(param_description = "The name to bind.")]
        key: &str,
        #[wasm_bindgen(param_description = "Its value, converted to a Swan value.")]
        value: JsValue,
    ) -> Result<(), JsValue> {
        self.scope.set(key, js_to_swan_value(value)?);
        Ok(())
    }

    /// Returns the names bound in the context, sorted. Engine globals and
    /// built-ins are not included; `texl` always is.
    #[wasm_bindgen(unchecked_return_type = "string[]")]
    pub fn names(&self) -> Array {
        let mut keys: Vec<String> = self.scope.0.borrow().bindings.keys().cloned().collect();
        keys.sort();
        keys.iter().map(|k| JsValue::from_str(k)).collect()
    }

    /// Renders a template in this context. The names it assigns stay in the context.
    #[wasm_bindgen(unchecked_return_type = "Promise<string>", return_description = "The rendered text.")]
    pub fn render(
        &self,
        #[wasm_bindgen(param_description = "The template source.")]
        template: String,
    ) -> js_sys::Promise {
        let scope = self.scope.clone();
        wasm_bindgen_futures::future_to_promise(async move {
            Ok(JsValue::from_str(&template::render(&template, &scope).await))
        })
    }
}

/// `location.href` of the current page or worker, if any.
fn location_href() -> Option<String> {
    let location = Reflect::get(&js_sys::global(), &JsValue::from_str("location")).ok()?;
    Reflect::get(&location, &JsValue::from_str("href")).ok()?.as_string()
}

/// Replaces `time.now`, which reads the system clock: wasm32-unknown-unknown has none
/// and panics. The built-in namespaces are shared by every scope of the thread, so this
/// changes it for every document.
fn use_js_clock(engine: &Engine) {
    if let Value::Namespace(time) = engine.create_scope().get("time") {
        time.set(
            "now",
            Value::Builtin(BuiltinFn::new("time.now", |_| Value::Numb(js_sys::Date::now() / 1000.0))),
        );
    }
}
