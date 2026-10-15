//! Web bindings of the texl engine.
//!
//! ```js
//! import init, { TexlEngine } from "./pkg/texl_wasm.js";
//! await init();
//! const engine = new TexlEngine({ reader: "Ada" });          // globals, optional base URL
//! const doc = await engine.load("docs/main.texl");           // {uri, source, data, rendering} or null
//! const text = await engine.render("Hello <% reader %>!");
//! ```

mod convert;
mod loader;

use js_sys::{Array, Object, Reflect};
use swan_core::value::{BuiltinFn, Scope, Value};
use texl_core::helpers::spawner_fn;
use texl_core::{Engine, loader_fn, template};
use url::Url;
use wasm_bindgen::prelude::*;

use convert::{for_each_entry, js_to_swan_value, swan_value_to_js};

/// A texl engine that loads documents over HTTP(S).
///
/// Documents with an `http://` or `https://` URL are fetched as they are. Imports
/// resolve against the importing document: relative paths against its folder, absolute
/// paths (`/dir/doc.texl`) against its root folder. Templates without a document URL
/// (`render`, contexts) use the engine's base URL instead.
#[wasm_bindgen]
pub struct TexlEngine {
    engine: Engine,
    base_url: Option<Url>,
}

#[wasm_bindgen]
impl TexlEngine {
    /// Creates an engine. `globals` are bound in the scope of every document.
    /// `baseUrl` defaults to the URL of the current page (or worker script).
    #[wasm_bindgen(constructor)]
    pub fn new(globals: Option<Object>, base_url: Option<String>) -> Result<TexlEngine, JsValue> {
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

    /// The URL that relative document URIs and absolute paths resolve against.
    #[wasm_bindgen(getter, js_name = baseUrl)]
    pub fn base_url(&self) -> Option<String> {
        self.base_url.as_ref().map(Url::to_string)
    }

    /// Binds a global, visible to every document rendered afterwards.
    pub fn set(&self, name: &str, value: JsValue) -> Result<(), JsValue> {
        self.engine.add_builtin(name, js_to_swan_value(value)?);
        Ok(())
    }

    /// Loads and renders the document at `uri`, resolved against the base URL.
    /// Returns a JS Promise of `{uri, source, data, rendering}`, or of `null` if the
    /// document cannot be loaded. Each document is loaded once per engine.
    pub fn load(&self, uri: &str) -> js_sys::Promise {
        let engine = self.engine.clone();
        let url = loader::resolve_document_url(uri, self.base_url.as_ref());
        wasm_bindgen_futures::future_to_promise(async move {
            let url = url.map_err(|e| JsValue::from_str(&e))?;
            Ok(swan_value_to_js(&engine.load(&url).await))
        })
    }

    /// Renders a template in a fresh scope. Returns a JS Promise of the rendered text.
    pub fn render(&self, template: String) -> js_sys::Promise {
        let engine = self.engine.clone();
        wasm_bindgen_futures::future_to_promise(async move {
            Ok(JsValue::from_str(&engine.render(&template).await))
        })
    }

    /// Creates a context: a scope that keeps its bindings across renders.
    #[wasm_bindgen(js_name = createContext)]
    pub fn create_context(&self, vars: Option<Object>) -> Result<TexlContext, JsValue> {
        let scope = self.engine.create_scope();
        if let Some(vars) = vars {
            for_each_entry(&vars, |name, value| scope.set(name, value))?;
        }
        Ok(TexlContext { scope })
    }
}

/// A scope of a [`TexlEngine`] whose bindings persist across renders: names
/// assigned by a template can be read by later templates and by JS.
#[wasm_bindgen]
pub struct TexlContext {
    scope: Scope,
}

#[wasm_bindgen]
impl TexlContext {
    /// Retrieves a variable bound in the context by key
    pub fn get(&self, key: &str) -> JsValue {
        swan_value_to_js(&self.scope.get(key))
    }

    /// Sets or updates a variable in the context scope.
    pub fn set(&self, key: &str, value: JsValue) -> Result<(), JsValue> {
        self.scope.set(key, js_to_swan_value(value)?);
        Ok(())
    }

    /// Returns the names bound in the context scope (not the engine globals), sorted.
    pub fn names(&self) -> Array {
        let mut keys: Vec<String> = self.scope.0.borrow().bindings.keys().cloned().collect();
        keys.sort();
        keys.iter().map(|k| JsValue::from_str(k)).collect()
    }

    /// Renders a template in this context. Returns a JS Promise of the rendered text.
    pub fn render(&self, template: String) -> js_sys::Promise {
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
    if let Value::Namespace(time) = engine.create_scope().get("time") {
        time.set(
            "now",
            Value::Builtin(BuiltinFn::new("time.now", |_| Value::Numb(js_sys::Date::now() / 1000.0))),
        );
    }
}
