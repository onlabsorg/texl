//! The texl engine: documents, loaders and imports.
//!
//! An [`Engine`] renders templates and loads *documents*: templates identified
//! by a URI. It owns
//!
//! - a **base scope** with the Swan built-ins and any globals added by the host
//!   with [`Engine::add_builtin`];
//! - a set of **loaders** that fetch the source of a document from its URI,
//!   one per URI scheme ([`Engine::add_loader`]) plus a default loader for
//!   paths without a scheme ([`Engine::add_default_loader`]);
//! - a **cache** of documents, so that each document is loaded and rendered at
//!   most once.
//!
//! # Documents
//!
//! Loading a document produces a namespace with four fields:
//!
//! | Field       | Type        | Content                                          |
//! |-------------|-------------|--------------------------------------------------|
//! | `uri`       | `Text`      | The normalized URI of the document               |
//! | `source`    | `Text`      | The template source, as returned by the loader   |
//! | `data`      | `Namespace` | The scope the template was rendered in           |
//! | `rendering` | `Text`      | The rendered text                                |
//!
//! The `data` field holds every name the template assigned, so a document can
//! act as a library of values and functions.
//!
//! # Imports
//!
//! Every scope created by the engine has a `texl` namespace containing
//! `texl.import`. Templates call `texl.import(uri)` to load another document.
//! It always returns a promise of the document namespace (or of `Nothing`, if
//! the document cannot be loaded). Relative URIs resolve against the URI of the
//! importing document, or against the base URL given to [`Engine::create_scope`];
//! in a scope without a document, they resolve against the root path `/`. The imported document is rendered in a child of the
//! importing scope, so it can read the names of its importer.
//!
//! # Example
//!
//! ```
//! use swan_core::value::Value;
//! use texl_core::{Engine, loader_fn};
//!
//! let engine = Engine::new(None);
//! engine.add_builtin("reader", Value::Text("Ada".into()));
//! engine.add_default_loader(loader_fn(|uri| async move {
//!     match uri.as_str() {
//!         "/greeting.texl" => Ok("Hello <% reader %>!".to_string()),
//!         _ => Err(format!("{uri}: not found")),
//!     }
//! }));
//!
//! let text = block_on(engine.render(
//!     r#"<% texl.import("greeting.texl") ~> (doc -> doc.rendering) %>"#
//! ));
//! assert_eq!(text, "Hello Ada!");
//! # use std::{future::Future, pin::pin, task::{Context, Poll, Waker}};
//! # fn block_on<F: Future>(fut: F) -> F::Output {
//! #     let mut cx = Context::from_waker(Waker::noop());
//! #     let mut fut = pin!(fut);
//! #     loop {
//! #         texl_core::helpers::run_pending();
//! #         if let Poll::Ready(v) = fut.as_mut().poll(&mut cx) { return v; }
//! #     }
//! # }
//! ```

use std::cell::RefCell;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use url::Url;

use swan_core::builtins::root_scope;
use swan_core::value::{BuiltinFn, Scope, Value};

use crate::helpers::{promise_async, SpawnFn};
use crate::template;

/// A host-provided function that fetches the source of a document.
///
/// The engine calls it with the normalized URI of the document: a full URL
/// (`https://host/dir/doc.texl`) for scheme loaders, or an absolute,
/// percent-encoded path (`/dir/doc.texl`) for the default loader. The future
/// resolves to the template source, or to an error message. The engine
/// discards the message: a failed load gives `Nothing`.
///
/// Build one from an async closure with [`loader_fn`].
pub type LoaderFn = Rc<dyn Fn(&str) -> Pin<Box<dyn Future<Output = Result<String, String>>>>>;

/// Builds a [`LoaderFn`] from a closure that takes the URI and returns a future.
///
/// # Examples
///
/// ```
/// use texl_core::loader_fn;
///
/// let loader = loader_fn(|uri| async move {
///     Ok(format!("This is <% \"{uri}\" %>"))
/// });
/// ```
pub fn loader_fn<F, Fut>(f: F) -> LoaderFn
where
    F: Fn(String) -> Fut + 'static,
    Fut: Future<Output = Result<String, String>> + 'static,
{
    Rc::new(move |uri: &str| Box::pin(f(uri.to_string())))
}

/// Renders templates and loads documents. See the [module documentation](self).
///
/// `Engine` is a cheap handle: clones share the same base scope, loaders and
/// cache. It is single-threaded (not `Send`).
#[derive(Clone)]
pub struct Engine {
    base_scope: Scope,
    loaders: Rc<RefCell<HashMap<String, LoaderFn>>>,
    default_loader: Rc<RefCell<Option<LoaderFn>>>,
    cache: Rc<RefCell<HashMap<String, Value>>>,
    spawner: Rc<RefCell<Option<SpawnFn>>>,
}

impl Engine {
    /// Creates an engine with no loaders and an empty cache.
    ///
    /// Imports run as background tasks. `spawner` hands those tasks to the
    /// host's async runtime (for example `tokio::task::spawn_local`, or
    /// `wasm_bindgen_futures::spawn_local`). With `None`, tasks go onto a
    /// thread-local queue, and the host must call
    /// [`helpers::run_pending`](crate::helpers::run_pending) to run them.
    pub fn new(spawner: Option<SpawnFn>) -> Self {
        Self {
            base_scope: root_scope(),
            loaders: Rc::new(RefCell::new(HashMap::new())),
            default_loader: Rc::new(RefCell::new(None)),
            cache: Rc::new(RefCell::new(HashMap::new())),
            spawner: Rc::new(RefCell::new(spawner)),
        }
    }

    /// Binds `name` to `value` in the base scope.
    ///
    /// The name is visible in every scope and document of this engine,
    /// including those created earlier, unless a template shadows it. Use it to
    /// pass data and host functions ([`Value::Builtin`]) to templates.
    pub fn add_builtin(&self, name: &str, value: Value) {
        self.base_scope.set(name, value);
    }

    /// Registers `loader` for the URIs whose scheme is `protocol` (for example
    /// `"https"` or `"file"`, without the colon). The scheme is
    /// case-insensitive. A later call for the same scheme replaces the loader.
    pub fn add_loader(&self, protocol: &str, loader: LoaderFn) {
        let scheme = protocol.to_lowercase();
        self.loaders.borrow_mut().insert(scheme, loader);
    }

    /// Registers the loader for URIs without a scheme, such as `/dir/doc.texl`
    /// or `doc.texl`. A later call replaces it.
    pub fn add_default_loader(&self, loader: LoaderFn) {
        *self.default_loader.borrow_mut() = Some(loader);
    }

    /// Loads and renders the document at `uri`.
    ///
    /// The URI is normalized (a scheme-less path becomes absolute, so `doc.texl`
    /// is `/doc.texl`), then passed to the loader of its scheme, or to the
    /// default loader if it has none. The source is rendered in a fresh child of
    /// the base scope, and imports inside it resolve against the URI.
    ///
    /// This method always loads the document, but it stores the result in the
    /// cache, so later imports of the same URI reuse it.
    ///
    /// # Returns
    ///
    /// - The document namespace `{uri, source, data, rendering}` (see the
    ///   [module documentation](self)).
    /// - `Nothing` if no loader matches the URI or the loader fails.
    /// - `Undefined` if the URI cannot be normalized.
    ///
    /// # Examples
    ///
    /// ```
    /// use swan_core::value::Value;
    /// use texl_core::{Engine, loader_fn};
    ///
    /// let engine = Engine::new(None);
    /// engine.add_loader("mem", loader_fn(|_uri| async { Ok("<% x = 1 %>x is <% x %>".to_string()) }));
    ///
    /// let Value::Namespace(doc) = block_on(engine.load("mem://doc")) else { panic!() };
    /// assert_eq!(doc.get("rendering"), Value::Text("x is 1".into()));
    /// assert_eq!(doc.get("uri"), Value::Text("mem://doc".into()));
    /// # use std::{future::Future, pin::pin, task::{Context, Poll, Waker}};
    /// # fn block_on<F: Future>(fut: F) -> F::Output {
    /// #     let mut cx = Context::from_waker(Waker::noop());
    /// #     let mut fut = pin!(fut);
    /// #     loop {
    /// #         texl_core::helpers::run_pending();
    /// #         if let Poll::Ready(v) = fut.as_mut().poll(&mut cx) { return v; }
    /// #     }
    /// # }
    /// ```
    pub async fn load(&self, uri: &str) -> Value {
        let norm_url = match self.normalize_url(uri) {
            Ok(url) => url,
            Err(err) => return Value::undef_unary("import.invalid_uri", Value::Text(err.into()), 0),
        };
        // Relative imports inside the loaded document resolve against its own URL
        let doc_scope = self.create_scope(Some(norm_url.clone()));
        self.load_internal(doc_scope, norm_url).await
    }

    /// Renders `template` in a fresh scope (see [`Engine::create_scope`]) and
    /// returns the rendered text.
    ///
    /// Names assigned by the template are discarded afterwards. To keep them,
    /// create a scope and call [`template::render`] directly.
    ///
    /// # Examples
    ///
    /// ```
    /// use texl_core::Engine;
    ///
    /// let engine = Engine::new(None);
    /// assert_eq!(block_on(engine.render("1 + 1 = <% 1 + 1 %>")), "1 + 1 = 2");
    /// # use std::{future::Future, pin::pin, task::{Context, Poll, Waker}};
    /// # fn block_on<F: Future>(fut: F) -> F::Output {
    /// #     let mut cx = Context::from_waker(Waker::noop());
    /// #     let mut fut = pin!(fut);
    /// #     loop {
    /// #         texl_core::helpers::run_pending();
    /// #         if let Poll::Ready(v) = fut.as_mut().poll(&mut cx) { return v; }
    /// #     }
    /// # }
    /// ```
    pub async fn render(&self, template: &str) -> String {
        let scope = self.create_scope(None);
        template::render(template, &scope).await
    }

    /// Creates a new child of the base scope with its own `texl` namespace.
    ///
    /// `base_url` is the URI of the document the scope stands for: `texl.import`
    /// resolves relative URIs against it, as in a document loaded with
    /// [`Engine::load`]. It is normalized first, so `docs/main.texl` is
    /// `/docs/main.texl`. With `None`, the scope has no document and relative
    /// URIs resolve against the root path `/`.
    ///
    /// Use it to render a document whose source the host already has, or to
    /// render several templates in the same scope with [`template::render`] and
    /// share names between them. The scope is not a loaded document: nothing is
    /// loaded or cached for `base_url`.
    ///
    /// # Examples
    ///
    /// ```
    /// use texl_core::{Engine, loader_fn, template};
    ///
    /// let engine = Engine::new(None);
    /// engine.add_default_loader(loader_fn(|uri| async move {
    ///     match uri.as_str() {
    ///         "/docs/name.texl" => Ok("Ada".to_string()),
    ///         _ => Err(format!("{uri}: not found")),
    ///     }
    /// }));
    ///
    /// let scope = engine.create_scope(Some("docs/main.texl".to_string()));
    /// let source = r#"Hello <% texl.import("name.texl") ~> (doc -> doc.rendering) %>!"#;
    /// assert_eq!(block_on(template::render(source, &scope)), "Hello Ada!");
    /// # use std::{future::Future, pin::pin, task::{Context, Poll, Waker}};
    /// # fn block_on<F: Future>(fut: F) -> F::Output {
    /// #     let mut cx = Context::from_waker(Waker::noop());
    /// #     let mut fut = pin!(fut);
    /// #     loop {
    /// #         texl_core::helpers::run_pending();
    /// #         if let Poll::Ready(v) = fut.as_mut().poll(&mut cx) { return v; }
    /// #     }
    /// # }
    /// ```
    pub fn create_scope(&self, base_url: Option<String>) -> Scope {
        // Create a child so document executions do not pollute the base_scope
        let child = self.base_scope.new_child();

        // Without a document, relative imports resolve against the root `/`
        let base_url = base_url.map(|url| self.normalize_url(&url).unwrap_or(url));
        self.bind_texl_namespace(&child, base_url);

        child
    }

    // --- Internal Helpers ---

    /// Attaches a standalone `texl` namespace to `scope`, with an `import` builtin
    /// that resolves relative paths against `base_url` (the URL of the document owning `scope`)
    fn bind_texl_namespace(&self, scope: &Scope, base_url: Option<String>) {
        let texl_ns = Scope::new();
        texl_ns.set("import", self.create_import_builtin(scope.clone(), base_url));
        scope.set("texl", Value::Namespace(texl_ns));
    }

    /// Creates the `texl.import` builtin bound to a specific document scope
    fn create_import_builtin(&self, parent_scope: Scope, base_url: Option<String>) -> Value {
        let engine = self.clone();
        Value::Builtin(BuiltinFn::new("texl.import", move |arg| {
            match arg {
                Value::Text(uri) => {
                    let norm_url = match engine.resolve_url(&uri, base_url.as_deref()) {
                        Ok(url) => url,
                        Err(err) => return Value::undef_unary("import.invalid_uri", Value::Text(err.into()), 0),
                    };

                    // Deduplicate in-flight and already loaded requests
                    if let Some(cached) = engine.cache.borrow().get(&norm_url) {
                        // ALWAYS return a Promise so the `~>` operator works consistently.
                        // If it's already a resolved Namespace, wrap it in a resolved Promise.
                        return if matches!(cached, Value::Promise(_)) {
                            cached.clone()
                        } else {
                            let p = swan_core::value::Promise::new();
                            p.resolve(cached.clone());
                            Value::Promise(p)
                        };
                    }

                    let engine_clone = engine.clone();
                    let norm_url_key = norm_url.clone();
                    let parent_scope_clone = parent_scope.clone();

                    let spawner = engine.spawner.borrow().clone();
                    let promise = promise_async(
                        spawner.as_ref(),
                        async move {
                            // 1. Create a child scope so the document inherits the caller's variables
                            let doc_scope = parent_scope_clone.new_child();
                            
                            // 2. Bind the texl namespace to the new document scope, using the
                            //    imported document's URL as base for its own relative imports
                            engine_clone.bind_texl_namespace(&doc_scope, Some(norm_url_key.clone()));

                            engine_clone.load_internal(doc_scope, norm_url_key).await
                        },
                    );

                    engine.cache.borrow_mut().insert(norm_url, promise.clone());
                    promise
                }
                other => Value::undef_unary("texl.import", other, 0),
            }
        }))
    }

    /// Core async loading logic used by both `load` and `texl.import`
    /// `norm_url` is the resolved and normalized URL: it is what the loader receives and the cache key.
    async fn load_internal(&self, doc_scope: Scope, norm_url: String) -> Value {
        let loader = if let Ok(parsed) = Url::parse(&norm_url) {
            let scheme = parsed.scheme().to_lowercase();
            let l = self.loaders.borrow().get(&scheme).cloned();
            match l {
                Some(loader) => loader,
                None => {
                    self.cache.borrow_mut().remove(&norm_url);
                    return Value::Nothing; // Silent failure per previous import logic
                }
            }
        } else {
            let l = self.default_loader.borrow().clone();
            match l {
                Some(loader) => loader,
                None => {
                    self.cache.borrow_mut().remove(&norm_url);
                    return Value::Nothing;
                }
            }
        };

        match loader(&norm_url).await {
            Ok(source) => {
                let rendering = template::render(&source, &doc_scope).await;

                let result = Scope::new();
                result.set("uri", Value::Text(norm_url.clone().into()));
                result.set("source", Value::Text(source.into()));
                result.set("data", Value::Namespace(doc_scope));
                result.set("rendering", Value::Text(rendering.into()));

                let result_val = Value::Namespace(result);
                
                // Overwrite the pending Promise in the cache with the resolved Namespace
                self.cache.borrow_mut().insert(norm_url, result_val.clone());
                result_val
            }
            Err(_) => {
                self.cache.borrow_mut().remove(&norm_url);
                Value::Nothing
            }
        }
    }
    /// Resolves `uri` against `base_url` (the URL of the importing document) and normalizes it.
    /// URIs with a scheme ignore the base. Absolute paths (starting with `/`) resolve against
    /// the root of a full-URI base (`/p` from `http://host/dir/doc` is `http://host/p`), and
    /// stay scheme-less otherwise. Without a base, relative paths resolve against the root `/`.
    fn resolve_url(&self, uri: &str, base_url: Option<&str>) -> Result<String, String> {
        let base = match base_url {
            Some(base) if Url::parse(uri).is_err() => base,
            _ => return self.normalize_url(uri),
        };

        if let Ok(base) = Url::parse(base) {
            // The base is a full URI (e.g. `http://host/dir/doc.texl`)
            base.join(uri).map(|u| u.to_string()).map_err(|e| e.to_string())
        } else if uri.starts_with('/') {
            self.normalize_url(uri)
        } else {
            // The base is a scheme-less path: resolve it under a dummy scheme, then strip it
            let dummy_base = Url::parse(&format!("app:///{}", base.trim_start_matches('/')))
                .map_err(|e| e.to_string())?;
            let joined = dummy_base.join(uri).map_err(|e| e.to_string())?;
            let mut norm = joined.path().to_string();
            if let Some(q) = joined.query() {
                norm.push('?');
                norm.push_str(q);
            }
            Ok(norm)
        }
    }

    fn normalize_url(&self, uri: &str) -> Result<String, String> {
        if let Ok(parsed) = Url::parse(uri) {
            Ok(parsed.to_string())
        } else {
            if let Ok(dummy) = Url::parse(&format!("app:///{}", uri.trim_start_matches('/'))) {
                let mut norm = dummy.path().to_string();
                if let Some(q) = dummy.query() {
                    norm.push('?');
                    norm.push_str(q);
                }
                Ok(norm)
            } else {
                Ok(uri.trim().to_string())
            }
        }
    }
}




#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::pin::pin;
    use std::sync::Arc;
    use std::task::{Context, Poll, Wake, Waker};
    use swan_core::value::Value;

    // --- Async Test Helpers ---

    struct NoopWake;
    impl Wake for NoopWake {
        fn wake(self: Arc<Self>) {}
    }

    /// Minimal busy-polling executor that drives a future to completion on the current thread.
    fn block_on<F: Future>(fut: F) -> F::Output {
        let waker = Waker::from(Arc::new(NoopWake));
        let mut cx = Context::from_waker(&waker);
        let mut fut = pin!(fut);
        loop {
            // Also pump the fallback spawner queue so nested promises resolve
            crate::helpers::run_pending();
            
            if let Poll::Ready(v) = fut.as_mut().poll(&mut cx) {
                return v;
            }
        }
    }

    /// Test helper to extract a namespace from a Value or panic
    fn unwrap_namespace(val: Value) -> Scope {
        match val {
            Value::Namespace(ns) => ns,
            other => panic!("Expected Namespace, got {:?}", other),
        }
    }

    // --- Tests ---

    #[test]
    fn test_engine_renders_basic_template() {
        let engine = Engine::new(None);
        let out = block_on(engine.render("Hello <% 10 + 20 %>!"));
        assert_eq!(out, "Hello 30!");
    }

    #[test]
    fn test_engine_injects_builtins() {
        let engine = Engine::new(None);
        engine.add_builtin("system_name", Value::Text("TexlCore".into()));
        engine.add_builtin("version", Value::Numb(2.0));

        let out = block_on(engine.render("<% system_name %> v<% version %>"));
        assert_eq!(out, "TexlCore v2");
    }

    #[test]
    fn test_engine_routes_loaders_by_scheme() {
        let engine = Engine::new(None);
        
        engine.add_loader("http", loader_fn(|uri| async move {
            Ok(format!("HTTP source for {}", uri))
        }));
        
        engine.add_default_loader(loader_fn(|uri| async move {
            Ok(format!("Default source for {}", uri))
        }));

        // HTTP scheme routed to the 'http' loader
        let http_res = block_on(engine.load("http://example.com/doc.texl"));
        let http_ns = unwrap_namespace(http_res);
        assert_eq!(http_ns.get("source"), Value::Text("HTTP source for http://example.com/doc.texl".into()));

        // Scheme-less URI routed to the default loader
        let file_res = block_on(engine.load("/local/file.texl"));
        let file_ns = unwrap_namespace(file_res);
        assert_eq!(file_ns.get("source"), Value::Text("Default source for /local/file.texl".into()));
    }

    #[test]
    fn test_engine_load_returns_complete_namespace() {
        let engine = Engine::new(None);
        engine.add_builtin("multiplier", Value::Numb(5.0));
        
        engine.add_default_loader(loader_fn(|_| async move {
            Ok("Result is <% result = 10 * multiplier, result %>".to_string())
        }));

        let load_result = block_on(engine.load("/math.texl"));
        let ns = unwrap_namespace(load_result);

        assert_eq!(ns.get("uri"), Value::Text("/math.texl".into()));
        assert_eq!(ns.get("rendering"), Value::Text("Result is 50".into()));
        
        // Ensure the document data contains the assignments
        let doc_data = unwrap_namespace(ns.get("data"));
        assert_eq!(doc_data.get("result"), Value::Numb(50.0));
    }

    #[test]
    fn test_engine_caches_internal_imports_and_deduplicates() {
        let engine = Engine::new(None);
        let call_count = Rc::new(Cell::new(0));
        let c = call_count.clone();

        engine.add_default_loader(loader_fn(move |uri| {
            c.set(c.get() + 1);
            let content = if uri.contains("main") {
                // CORRECTED: Use Swan's `~>` operator to chain the promise and extract the `.rendering` field
                r#"Main loads <% texl.import("/shared.texl") ~> (ns -> ns.rendering) %> twice: <% texl.import("/shared.texl") ~> (ns -> ns.rendering) %>"#
            } else {
                "Shared!"
            };
            let content_str = content.to_string();
            async move { Ok(content_str) }
        }));

        // Render the main template, also using the `~>` operator
        let rendered = block_on(engine.render(r#"<% texl.import("/main.texl") ~> (ns -> ns.rendering) %>"#));
        
        assert_eq!(rendered, "Main loads Shared! twice: Shared!");
        
        // main.texl loaded once, shared.texl loaded once, despite two `texl.import` calls
        assert_eq!(call_count.get(), 2);
    }

    /// Builds an engine whose loaders serve sources from `docs` (keyed by the URI the loader
    /// receives) and record every URI they are asked to load.
    fn engine_with_docs(docs: &[(&str, &str)]) -> (Engine, Rc<RefCell<Vec<String>>>) {
        let engine = Engine::new(None);
        let docs: Rc<HashMap<String, String>> = Rc::new(
            docs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
        );
        let requested = Rc::new(RefCell::new(Vec::new()));

        let make_loader = || {
            let docs = docs.clone();
            let requested = requested.clone();
            loader_fn(move |uri| {
                requested.borrow_mut().push(uri.clone());
                let res = docs.get(&uri).cloned().ok_or(format!("not found: {}", uri));
                async move { res }
            })
        };
        engine.add_default_loader(make_loader());
        engine.add_loader("http", make_loader());
        (engine, requested)
    }

    const RENDERING: &str = "~> (ns -> ns.rendering)";

    #[test]
    fn test_engine_resolves_relative_imports_against_importing_document() {
        let main = format!(r#"[main <% texl.import("parts/header.texl") {RENDERING} %>]"#);
        let header = format!(r#"[header <% texl.import("../footer.texl") {RENDERING} %>]"#);
        let footer = format!(r#"[footer <% texl.import("./parts/../parts/note.texl") {RENDERING} %>]"#);
        let (engine, requested) = engine_with_docs(&[
            ("/docs/main.texl", &main),
            ("/docs/parts/header.texl", &header),
            ("/docs/footer.texl", &footer),
            ("/docs/parts/note.texl", "note"),
        ]);

        let out = block_on(engine.render(&format!(r#"<% texl.import("/docs/main.texl") {RENDERING} %>"#)));

        assert_eq!(out, "[main [header [footer note]]]");
        assert_eq!(
            *requested.borrow(),
            vec!["/docs/main.texl", "/docs/parts/header.texl", "/docs/footer.texl", "/docs/parts/note.texl"]
        );
    }

    #[test]
    fn test_engine_load_resolves_relative_imports_against_loaded_document() {
        let main = format!(r#"<% texl.import("lib.texl") {RENDERING} %>"#);
        let (engine, requested) = engine_with_docs(&[("/a/b/main.texl", &main), ("/a/b/lib.texl", "lib")]);

        let ns = unwrap_namespace(block_on(engine.load("/a/b/main.texl")));

        assert_eq!(ns.get("rendering"), Value::Text("lib".into()));
        assert_eq!(*requested.borrow(), vec!["/a/b/main.texl", "/a/b/lib.texl"]);
    }

    #[test]
    fn test_engine_absolute_paths_and_uris_ignore_importing_document() {
        let main = format!(
            r#"<% texl.import("/top.texl") {RENDERING} %>,<% texl.import("http://example.com/x/remote.texl") {RENDERING} %>"#
        );
        let (engine, requested) = engine_with_docs(&[
            ("/docs/main.texl", &main),
            ("/top.texl", "top"),
            ("http://example.com/x/remote.texl", "remote"),
        ]);

        let out = block_on(engine.render(&format!(r#"<% texl.import("/docs/main.texl") {RENDERING} %>"#)));

        assert_eq!(out, "top,remote");
        assert_eq!(
            *requested.borrow(),
            vec!["/docs/main.texl", "/top.texl", "http://example.com/x/remote.texl"]
        );
    }

    #[test]
    fn test_engine_resolves_relative_imports_against_uri_documents() {
        let main = format!(r#"<% texl.import("../lib/util.texl?v=1") {RENDERING} %>"#);
        let (engine, requested) = engine_with_docs(&[
            ("http://example.com/site/pages/main.texl", &main),
            ("http://example.com/site/lib/util.texl?v=1", "util"),
        ]);

        let ns = unwrap_namespace(block_on(engine.load("http://example.com/site/pages/main.texl")));

        assert_eq!(ns.get("rendering"), Value::Text("util".into()));
        assert_eq!(
            *requested.borrow(),
            vec!["http://example.com/site/pages/main.texl", "http://example.com/site/lib/util.texl?v=1"]
        );
    }

    #[test]
    fn test_engine_resolves_absolute_paths_against_the_root_of_uri_documents() {
        let main = format!(r#"<% texl.import("/lib/util.texl") {RENDERING} %>"#);
        let (engine, requested) = engine_with_docs(&[
            ("http://example.com/site/pages/main.texl", &main),
            ("http://example.com/lib/util.texl", "util"),
        ]);

        let ns = unwrap_namespace(block_on(engine.load("http://example.com/site/pages/main.texl")));

        assert_eq!(ns.get("rendering"), Value::Text("util".into()));
        assert_eq!(
            *requested.borrow(),
            vec!["http://example.com/site/pages/main.texl", "http://example.com/lib/util.texl"]
        );
    }

    #[test]
    fn test_engine_relative_imports_without_document_resolve_against_root() {
        let (engine, requested) = engine_with_docs(&[("/dir/doc.texl", "doc")]);

        let out = block_on(engine.render(&format!(r#"<% texl.import("dir/doc.texl") {RENDERING} %>"#)));

        assert_eq!(out, "doc");
        assert_eq!(*requested.borrow(), vec!["/dir/doc.texl"]);
    }

    #[test]
    fn test_engine_scope_with_base_url_resolves_relative_imports_against_it() {
        let (engine, requested) = engine_with_docs(&[
            ("/docs/lib.texl", "lib"),
            ("/top.texl", "top"),
            ("http://example.com/site/lib.texl", "remote lib"),
            ("http://example.com/top.texl", "remote top"),
        ]);
        let source = format!(r#"<% texl.import("lib.texl") {RENDERING} %>,<% texl.import("/top.texl") {RENDERING} %>"#);

        // A scheme-less base URL is normalized like the URI of a loaded document
        let scope = engine.create_scope(Some("docs/main.texl".to_string()));
        assert_eq!(block_on(template::render(&source, &scope)), "lib,top");

        let scope = engine.create_scope(Some("http://example.com/site/main.texl".to_string()));
        assert_eq!(block_on(template::render(&source, &scope)), "remote lib,remote top");

        assert_eq!(
            *requested.borrow(),
            vec!["/docs/lib.texl", "/top.texl", "http://example.com/site/lib.texl", "http://example.com/top.texl"]
        );
    }

    #[test]
    fn test_engine_scope_with_base_url_renders_like_the_loaded_document() {
        let main = format!(r#"<% x = 1 %><% texl.import("lib.texl") {RENDERING} %>"#);
        let (engine, requested) = engine_with_docs(&[("/a/main.texl", &main), ("/a/lib.texl", "lib <% x %>")]);

        let scope = engine.create_scope(Some("/a/main.texl".to_string()));
        let rendering = block_on(template::render(&main, &scope));
        let ns = unwrap_namespace(block_on(engine.load("/a/main.texl")));

        assert_eq!(rendering, "lib 1");
        assert_eq!(ns.get("rendering"), Value::Text(rendering.into()));
        // The scope does not load or cache the document it stands for; the import is cached
        assert_eq!(*requested.borrow(), vec!["/a/lib.texl", "/a/main.texl"]);
    }

    #[test]
    fn test_engine_scope_without_base_url_resolves_relative_imports_against_root() {
        let (engine, requested) = engine_with_docs(&[("/dir/doc.texl", "doc")]);

        let scope = engine.create_scope(None);
        let out = block_on(template::render(&format!(r#"<% texl.import("dir/doc.texl") {RENDERING} %>"#), &scope));

        assert_eq!(out, "doc");
        assert_eq!(*requested.borrow(), vec!["/dir/doc.texl"]);
    }

    #[test]
    fn test_engine_caches_relative_imports_by_resolved_url() {
        // Same relative name from two different folders: two distinct documents
        let a = format!(r#"<% texl.import("common.texl") {RENDERING} %>"#);
        let b = format!(r#"<% texl.import("common.texl") {RENDERING} %>"#);
        // Different relative names resolving to the same document: one load
        let c = format!(r#"<% texl.import("../a/common.texl") {RENDERING} %>"#);
        let (engine, requested) = engine_with_docs(&[
            ("/a/main.texl", &a),
            ("/b/main.texl", &b),
            ("/c/main.texl", &c),
            ("/a/common.texl", "A"),
            ("/b/common.texl", "B"),
        ]);

        let out = block_on(engine.render(&format!(
            r#"<% texl.import("/a/main.texl") {RENDERING} %><% texl.import("/b/main.texl") {RENDERING} %><% texl.import("/c/main.texl") {RENDERING} %>"#
        )));

        assert_eq!(out, "ABA");
        assert_eq!(
            *requested.borrow(),
            vec!["/a/main.texl", "/a/common.texl", "/b/main.texl", "/b/common.texl", "/c/main.texl"]
        );
    }

    #[test]
    fn test_engine_handles_missing_loader_gracefully() {
        let engine = Engine::new(None);
        // We purposefully do not add an ftp loader or a default loader
        
        let out = block_on(engine.load("ftp://example.com/file.texl"));
        
        // Should gracefully fail to Value::Nothing per the import protocol
        assert_eq!(out, Value::Nothing);
    }
}
