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
//! const doc = await engine.load("docs/main.texl");           // {uri, source, data, text, tags}
//! const draft = await engine.render("docs/draft.texl", "Hello <% reader %>!");
//! ```
//!
//! `engine.addLoader(scheme, fn)` and `engine.setDefaultLoader(fn)` add loaders to
//! the built-in `http`/`https` one.
//!
//! The module also exports `collect()`, the cycle collector (see [`collect`]),
//! `isUndefined(value)` (see [`is_undefined`]), `isSwanFunction(value)` (see
//! [`is_swan_function`]), `normalize(value)` (see [`normalize`]) and `Tuple`,
//! the array subclass of Swan tuples.
//!
//! See the [JavaScript guide and API reference](https://github.com/onlabsorg/texl/blob/main/docs/api/javascript.md).

mod convert;
mod function;
mod handles;
mod loader;
mod undefined;

use js_sys::{Array, Object, Reflect};
use swan_core::builtins::set_builtin;
use texl_core::document::utf16_offset;
use swan_core::value::{BuiltinFn, Scope, Value};
use texl_core::helpers::spawner_fn;
use texl_core::{Engine, loader_fn, template};
use url::Url;
use wasm_bindgen::prelude::*;

use convert::{for_each_entry, js_to_swan_value, swan_value_to_js};

#[wasm_bindgen(typescript_custom_section)]
const TS_TYPES: &'static str = r#"
/**
 * A Swan tuple. It is an `Array` with nothing added, so it reads like any
 * array, and `instanceof Tuple` tells it from a list, which is a plain array.
 * Passed to the engine, a `Tuple` becomes a Swan tuple and any other array a
 * list. Swan tuples are normalized: nested tuples are flattened, `null` and
 * `undefined` items are dropped, one item gives that item and no items give
 * `Nothing`. See `normalize` to do the same in JavaScript.
 */
export class Tuple<T = any> extends Array<T> {}

/**
 * A `<% ... %>` tag of a rendered template. Positions are UTF-16 code units
 * (JavaScript string indices): `source.slice(tag.start, tag.end)` is the tag
 * and `text.slice(tag.out_start, tag.out_end)` what it rendered.
 */
/**
 * What a loader returns: the source of a document, or the source and the final
 * URL of the document, the way an HTTP redirect gives one.
 *
 * The document is the one at its final URL: that is its `uri`, the URL its
 * relative imports resolve against, and its identity within one rendering, so
 * imports of URLs with the same final URL share one document. A relative
 * `url` resolves against the requested URL.
 */
export type TexlLoaded = string | {
    /** The final URL of the document. Defaults to the requested URL. */
    url?: string | null;
    /** The template source. */
    source: string;
};

/** The source of a document and its final URL, as `TexlEngine.fetch` gives them. */
export interface TexlFetched {
    /**
     * The final URL of the document: the one its loader returned, or the URL
     * after HTTP redirects, or else the requested URL (without fragment).
     * `load` renders the document at it.
     */
    url: string;
    /** The template source. */
    source: string;
}

/** A document loader: takes a URI and returns the source, or a promise of it. */
export type TexlLoader = (uri: string) => TexlLoaded | Promise<TexlLoaded>;

export interface TexlTag {
    /** Where the tag starts in the source (at `<%`). */
    start: number;
    /** Where the tag ends in the source (after `%>`). */
    end: number;
    /** Where the text of the tag starts in the rendered text. */
    out_start: number;
    /** Where the text of the tag ends in the rendered text. */
    out_end: number;
    /**
     * The value that was rendered, converted to JavaScript. If the expression
     * returned a promise, it is the resolved value; promises nested in the
     * value are not awaited. An error is a `TexlUndefined`, also when the tag
     * renders nothing (an assignment).
     */
    value: any;
}

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
    text: string;
    /** The tags of the source, in order, including the tags that render nothing. */
    tags: TexlTag[];
}

/**
 * A template rendered in a context, as returned by `TexlContext.render`.
 */
export interface TexlRendering {
    /** The rendered text. */
    text: string;
    /** The names bound in the context when the rendering ended, including those bound before it. */
    data: Record<string, any>;
    /** The tags of the template, in order, including the tags that render nothing. */
    tags: TexlTag[];
}

/**
 * A Swan undefined value: the result of an invalid operation, such as
 * `"a" - 1`. A frozen plain object; members that do not apply are `null`.
 * Passing it back to Swan (`context.set`, the result of a JS function) gives
 * the original undefined value.
 */
export interface TexlUndefined {
    /** The kind of error: `"UnaryOperation"`, `"BinaryOperation"`, `"Termination"`, `"Token"`, `"Name"` or `"Value"` (made by `undef.from`). */
    readonly type: string;
    /** The error in plain words. */
    readonly description: string;
    /** The name of the failed operation. */
    readonly op: string | null;
    /** The operand of a failed unary operation, or the value of `undef.from`. */
    readonly val: any;
    /** The left operand of a failed binary operation. */
    readonly lhv: any;
    /** The right operand of a failed binary operation. */
    readonly rhv: any;
    /** The missing closing bracket of a `"Termination"` error. */
    readonly expected: string | null;
    /** The misplaced token of a `"Token"` error. */
    readonly token: string | null;
    /** The error of a `"HostError"`, as the host words it, such as `"Error: boom"`. */
    readonly error: string | null;
    /** Where the error is in the source of the code that made it, in UTF-16 code units (for an error of a built-in or host function, the call that returned it); `null` for values made by `undef.from`. */
    readonly pos: number | null;
    /** The text `pos` is an index of: the whole document, or the code run by `parse`; `null` when `pos` is. A getter: the string is made each time it is read. */
    readonly source: string | null;
    /** The URL of the document the error is in; `null` when `pos` is, and for contexts. */
    readonly uri: string | null;
    /** The first operand that is itself undefined: following `cause` leads to the root cause. */
    readonly cause: TexlUndefined | null;
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
/// The size of the stack this module is linked with, in megabytes: the
/// `stack-size` of the wasm target in `.cargo/config.toml`. The depth limit
/// of the evaluator is its default, which assumes 1 MB, times this. Measured
/// in Chrome, that is about 40% of the depth at which the browser's own call
/// stack ends (which a larger wasm stack does not move).
const STACK_MEGABYTES: usize = 2;

#[wasm_bindgen]
pub struct TexlEngine {
    engine: Engine,
    base_url: Option<Url>,
}

/// The `Error` a promise of `fetch` or `load` rejects with, from the `Undefined` of the
/// failed load: its description is the message, its operands the `uri` and the `reason`.
fn load_error(undefined: &Value) -> JsValue {
    let details = swan_value_to_js(undefined);
    let get = |name: &str| Reflect::get(&details, &JsValue::from_str(name)).unwrap_or(JsValue::UNDEFINED);
    let error = js_sys::Error::new(&get("description").as_string().unwrap_or_default());
    error.set_name("LoadError");
    let _ = Reflect::set(&error, &JsValue::from_str("uri"), &get("lhv"));
    let _ = Reflect::set(&error, &JsValue::from_str("reason"), &get("rhv"));
    error.into()
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

        report_panics();
        let engine = Engine::new(Some(spawner_fn(wasm_bindgen_futures::spawn_local)));
        engine.add_loader("http", loader_fn(loader::load_http));
        engine.add_loader("https", loader_fn(loader::load_http));
        use_js_clock(&engine);
        // The limit of the evaluator assumes 1 MB of stack; this module has more
        swan_core::evaluator::set_max_depth(STACK_MEGABYTES * swan_core::evaluator::DEFAULT_MAX_DEPTH);

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

    /// Registers the loader of the URIs with a given scheme. It replaces any previous
    /// loader of the scheme, including the built-in `http` and `https` loader.
    ///
    /// The loader serves `fetch`, `load` and `texl.import`. Nothing is cached: wrap the
    /// loader to cache sources.
    ///
    /// A loader that resolves URIs (aliases, links, redirects) returns `{url, source}`
    /// with the URL it resolved to: the document then is the one at that URL, which is
    /// its `uri` and the base of its relative imports, and imports of URIs with the same
    /// final URL share one document within a rendering.
    #[wasm_bindgen(js_name = addLoader)]
    pub fn add_loader(
        &self,
        #[wasm_bindgen(param_description = "The URI scheme, without the colon, such as `\"mem\"`. It is case-insensitive.")]
        scheme: &str,
        #[wasm_bindgen(
            unchecked_param_type = "TexlLoader",
            param_description = "A function that takes the full URI and returns the source of the document, or `{url, source}` with its final URL, or a promise of either. If it throws, its promise rejects, or its result is neither, the document cannot be loaded."
        )]
        loader: js_sys::Function,
    ) {
        self.engine.add_loader(scheme, loader::js_loader(loader));
    }

    /// Registers the loader of the URIs without a scheme. The engine normalizes the paths
    /// before calling the loader: they are absolute and percent-encoded, such as
    /// `/dir/my%20doc.texl`.
    ///
    /// The URIs given to the engine resolve against `baseUrl` to full URLs, and so do the
    /// imports in their documents, so the engine does not currently produce paths without
    /// a scheme.
    #[wasm_bindgen(js_name = setDefaultLoader)]
    pub fn set_default_loader(
        &self,
        #[wasm_bindgen(
            unchecked_param_type = "TexlLoader",
            param_description = "A function that takes the path and returns the source of the document, or a promise of it, like the loaders of `addLoader`."
        )]
        loader: js_sys::Function,
    ) {
        self.engine.add_default_loader(loader::js_loader(loader));
    }

    /// Fetches and renders the document at `uri`: `fetch` followed by `render` of the
    /// source as the document at its final URL (the one its loader returned, or the
    /// URL after HTTP redirects), which is the `uri` of the document.
    ///
    /// The promise rejects with an `Error` if `uri` cannot be resolved to a URL or the
    /// document cannot be loaded. For a failed load, the `name` of the error is
    /// `"LoadError"`, and it has the URL in `uri` and the reason in `reason`.
    #[wasm_bindgen(
        unchecked_return_type = "Promise<TexlDocument>",
        return_description = "The document."
    )]
    pub fn load(
        &self,
        #[wasm_bindgen(param_description = "The URL of the document, resolved against `baseUrl`.")]
        uri: &str,
    ) -> js_sys::Promise {
        let engine = self.engine.clone();
        let url = loader::resolve_document_url(uri, self.base_url.as_ref());
        wasm_bindgen_futures::future_to_promise(async move {
            let url = url.map_err(|e| JsValue::from(js_sys::Error::new(&e)))?;
            Ok(swan_value_to_js(&engine.load(&url).await.map_err(|undefined| load_error(&undefined))?))
        })
    }

    /// Renders `source` as the document at `uri`, without fetching it.
    ///
    /// Imports in `source` resolve against the URL of `uri`, and an import of that URL
    /// itself gives an undefined value (a cycle). A source with no real location, such as an
    /// unsaved buffer, can use any URL, for example one next to the documents it imports.
    ///
    /// The promise rejects with an `Error` only if `uri` cannot be resolved to a URL.
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
            let url = url.map_err(|e| JsValue::from(js_sys::Error::new(&e)))?;
            Ok(swan_value_to_js(&engine.render(&url, &source).await))
        })
    }

    /// Fetches the source of the document at `uri` and its final URL, without rendering it.
    /// Nothing is cached: every call fetches again. The final URL is the one the loader
    /// returned, or the URL after HTTP redirects, or else the requested URL (without
    /// fragment): `load(uri)` is `render(fetched.url, fetched.source)`.
    ///
    /// The promise rejects with an `Error` if `uri` cannot be resolved to a URL or the
    /// document cannot be fetched. For a failed load, the `name` of the error is
    /// `"LoadError"`, and it has the URL in `uri` and the reason in `reason`.
    #[wasm_bindgen(
        unchecked_return_type = "Promise<TexlFetched>",
        return_description = "The final URL and the source."
    )]
    pub fn fetch(
        &self,
        #[wasm_bindgen(param_description = "The URL of the document, resolved against `baseUrl`.")]
        uri: &str,
    ) -> js_sys::Promise {
        let engine = self.engine.clone();
        let url = loader::resolve_document_url(uri, self.base_url.as_ref());
        wasm_bindgen_futures::future_to_promise(async move {
            let url = url.map_err(|e| JsValue::from(js_sys::Error::new(&e)))?;
            let fetched = engine.fetch(&url).await.map_err(|undefined| load_error(&undefined))?;
            let object = Object::new();
            Reflect::set(&object, &JsValue::from_str("url"), &JsValue::from_str(&fetched.url))?;
            Reflect::set(&object, &JsValue::from_str("source"), &JsValue::from_str(&fetched.source))?;
            Ok(object.into())
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
        self.scope.namespace().names().iter().map(|k| JsValue::from_str(k)).collect()
    }

    /// Renders a template in this context. The names it assigns stay in the context.
    #[wasm_bindgen(
        unchecked_return_type = "Promise<TexlRendering>",
        return_description = "The rendering: the text, the names of the context and the tags."
    )]
    pub fn render(
        &self,
        #[wasm_bindgen(param_description = "The template source.")]
        template: String,
    ) -> js_sys::Promise {
        let scope = self.scope.clone();
        wasm_bindgen_futures::future_to_promise(async move {
            let rendering = template::render(template, &scope).await;
            let rendering = swan_value_to_js(&Value::Namespace(rendering.to_namespace(utf16_offset)));
            // As `TexlEngine.render` does: free what only reference cycles keep alive
            swan_core::gc::collect();
            Ok(rendering)
        })
    }
}

/// Frees the scopes and values of templates that only reference cycles keep alive.
///
/// A function defined in a template holds the scope it was defined in, so a scope
/// that stores one is never freed by reference counting alone. `collect` finds the
/// scopes that nothing outside such cycles refers to any more, and frees them.
///
/// `TexlEngine.render` and `TexlEngine.load` run it when they are done, so a page
/// that only renders documents does not need it. Call it after freeing a
/// `TexlContext` (`context.free()`) that defined functions. It works for every
/// engine of the page or worker.
#[wasm_bindgen(return_description = "The number of scopes and promises it cleared.")]
pub fn collect() -> usize {
    swan_core::gc::collect()
}

/// Gives a value the form Swan would give it, without passing it to the engine.
///
/// A `Tuple` is flattened and loses its `null` and `undefined` items; with one
/// item left it is that item, with none it is `null`. An array spreads its
/// `Tuple` items and loses its `null` and `undefined` items. This goes through
/// nested arrays and plain objects, whose members are kept even when `null`.
/// Anything else, such as a function, a promise or a class instance, is
/// returned as it is. The value passed is not changed.
#[wasm_bindgen(return_description = "The normalized value.")]
pub fn normalize(
    #[wasm_bindgen(param_description = "Any value.")]
    value: JsValue,
) -> Result<JsValue, JsValue> {
    convert::normalize(value)
}

/// Tells whether `value` is a `TexlUndefined`: an undefined value that came from Swan.
#[wasm_bindgen(js_name = isUndefined, unchecked_return_type = "value is TexlUndefined")]
pub fn is_undefined(
    #[wasm_bindgen(param_description = "Any value.")]
    value: JsValue,
) -> bool {
    undefined::js_to_undefined(&value).is_some()
}

/// Tells whether `value` is a function that came from Swan.
///
/// A Swan function becomes a plain JS function: `f(a, b)` calls it with the
/// tuple `(a, b)`, `f()` with `()`. Passed back to the engine, it is the
/// original Swan function. A JS function that went into Swan and came back is
/// the original JS function, so it is not a Swan function.
#[wasm_bindgen(js_name = isSwanFunction, unchecked_return_type = "value is (...args: any[]) => any")]
pub fn is_swan_function(
    #[wasm_bindgen(param_description = "Any value.")]
    value: JsValue,
) -> bool {
    function::js_to_function(&value).is_some()
}

/// `location.href` of the current page or worker, if any.
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console, js_name = error)]
    fn console_error(message: &str);
}

/// Makes a panic write its message to the console. A panic is a bug of this
/// library: it stops the module with `RuntimeError: unreachable`, which says
/// nothing of what went wrong.
fn report_panics() {
    static HOOK: std::sync::Once = std::sync::Once::new();
    HOOK.call_once(|| {
        std::panic::set_hook(Box::new(|info| console_error(&format!("texl: {info}"))));
    });
}

fn location_href() -> Option<String> {
    let location = Reflect::get(&js_sys::global(), &JsValue::from_str("location")).ok()?;
    Reflect::get(&location, &JsValue::from_str("href")).ok()?.as_string()
}

/// Replaces `time.now`, which reads the system clock: wasm32-unknown-unknown has none
/// and panics. The built-ins are shared by every scope of the thread, so this changes it
/// for every document. Namespaces are immutable: the `time` built-in is replaced by a
/// copy with the new `now`.
fn use_js_clock(engine: &Engine) {
    if let Value::Namespace(mut time) = engine.create_scope().get("time") {
        time.insert(
            "now",
            Value::Builtin(BuiltinFn::new("time.now", |_| Value::Numb(js_sys::Date::now() / 1000.0))),
        );
        set_builtin("time", Value::Namespace(time));
    }
}
