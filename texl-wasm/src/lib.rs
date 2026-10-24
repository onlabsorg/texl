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
//! const text = await engine.render("Hello <% reader %>!");
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
 * A loaded document, as returned by `TexlEngine.load`.
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

/// A texl engine that renders templates and loads documents over HTTP(S).
///
/// Documents with an `http://` or `https://` URL are fetched as they are. Imports
/// resolve against the importing document: relative paths against its folder, absolute
/// paths (`/dir/doc.texl`) against its root folder. Templates without a document URL
/// (`render`, contexts created without a `uri`) use the root of the engine's base URL instead.
///
/// Each document is loaded and rendered at most once per engine.
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
        engine.add_default_loader(loader::root_loader(base_url.clone()));
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

    /// Binds a global, visible to every template and document of this engine.
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

    /// Loads and renders the document at `uri`.
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

    /// Renders a template in a fresh scope. The names it assigns are discarded.
    #[wasm_bindgen(unchecked_return_type = "Promise<string>", return_description = "The rendered text.")]
    pub fn render(
        &self,
        #[wasm_bindgen(param_description = "The template source.")]
        template: String,
    ) -> js_sys::Promise {
        let engine = self.engine.clone();
        wasm_bindgen_futures::future_to_promise(async move {
            Ok(JsValue::from_str(&engine.render(&template).await))
        })
    }

    /// Creates a context: a scope that keeps its bindings across renders.
    ///
    /// With a `uri`, the context stands for the document at that URL: imports in its
    /// templates resolve against it, as in a document loaded with `load`. Nothing is
    /// loaded from it. Without one, imports resolve against `baseUrl`, as in `render`.
    ///
    /// Throws if `uri` cannot be resolved to a URL.
    #[wasm_bindgen(js_name = createContext)]
    pub fn create_context(
        &self,
        #[wasm_bindgen(param_description = "Values bound in the context. Defaults to none.")]
        vars: Option<Object>,
        #[wasm_bindgen(param_description = "The URL of the document the context stands for, resolved against `baseUrl`. Defaults to none.")]
        uri: Option<String>,
    ) -> Result<TexlContext, JsValue> {
        let url = match uri {
            Some(uri) => Some(loader::resolve_document_url(&uri, self.base_url.as_ref()).map_err(|e| JsValue::from_str(&e))?),
            None => None,
        };
        let scope = self.engine.create_scope(url);
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
/// and panics. The engine's `time` namespace is shared, so this changes it for every document.
fn use_js_clock(engine: &Engine) {
    if let Value::Namespace(time) = engine.create_scope(None).get("time") {
        time.set(
            "now",
            Value::Builtin(BuiltinFn::new("time.now", |_| Value::Numb(js_sys::Date::now() / 1000.0))),
        );
    }
}
