//! Document importing for texl.
//!
//! This module provides [`import`], which loads a texl document from a URI, renders it,
//! and returns a Swan promise that resolves to a namespace with these fields:
//!
//! - `url`: the normalized URL of the document (also its cache key),
//! - `source`: the fetched source text,
//! - `context`: the document scope after rendering,
//! - `rendering`: the rendered text.
//!
//! # Loaders
//!
//! Fetching is delegated to host-provided loaders ([`LoaderFn`]); `texl-core` implements
//! none itself. Loaders are registered per URL scheme (`http`, `https`, `file`, ...) with
//! [`Registry::register_loader`]. A default loader, set with
//! [`Registry::set_default_loader`], handles scheme-less URIs such as `/path/to/file.texl`
//! (a CLI host would typically map it to the local file system).
//!
//! # Caching
//!
//! The [`Registry`] caches documents by normalized URL (see [`Registry::normalize_url`]).
//! A document is fetched at most once, and recursive imports do not loop, because the
//! in-flight promise is cached before loading starts.

use std::cell::RefCell;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use swan_core::value::{Scope, Value};
use url::Url;

use crate::context::create_import_texl_namespace;
use crate::helpers::{SpawnFn, Task};
use crate::template;

/// An external, host-provided document loader.
///
/// Receives the URI exactly as given to [`import`] (not normalized) and returns a future
/// yielding the document source, or an error message. Build one with [`loader_fn`].
pub type LoaderFn = Rc<dyn Fn(&str) -> Pin<Box<dyn Future<Output = Result<String, String>>>>>;

/// Builds a [`LoaderFn`] from a plain closure returning a future.
///
/// The closure receives the URI as an owned `String`, so the returned future need not
/// borrow from it.
pub fn loader_fn<F, Fut>(f: F) -> LoaderFn
where
    F: Fn(String) -> Fut + 'static,
    Fut: Future<Output = Result<String, String>> + 'static,
{
    Rc::new(move |uri: &str| -> Pin<Box<dyn Future<Output = Result<String, String>>>> {
        Box::pin(f(uri.to_string()))
    })
}

/// Host loader registry and document cache.
///
/// Cloning a `Registry` is cheap, and all clones share the same loaders, cache and
/// spawner. This lets the registry be captured by the `texl.import` builtin and by
/// imported documents' own `texl` namespaces.
#[derive(Clone, Default)]
pub struct Registry {
    /// Scheme-specific loaders, keyed by lowercase scheme.
    loaders: Rc<RefCell<HashMap<String, LoaderFn>>>,
    /// Loader for scheme-less URIs.
    default_loader: Rc<RefCell<Option<LoaderFn>>>,
    /// Maps normalized URL to the imported document's Namespace, or to its pending Promise while loading.
    cache: Rc<RefCell<HashMap<String, Value>>>,
    /// Optional host spawner; when `None`, tasks go to the fallback queue.
    spawner: Rc<RefCell<Option<SpawnFn>>>,
}

impl Registry {
    /// Creates an empty registry with no loaders, an empty cache and no spawner.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a custom loader for a specific scheme/protocol (e.g., "http", "https", "file", "ftp").
    pub fn register_loader(&self, protocol: impl Into<String>, loader: LoaderFn) {
        let scheme = protocol.into().to_lowercase();
        self.loaders.borrow_mut().insert(scheme, loader);
    }

    /// Sets the default loader for scheme-less/protocol-less URIs (e.g., "/path/to/file.texl").
    pub fn set_default_loader(&self, loader: LoaderFn) {
        *self.default_loader.borrow_mut() = Some(loader);
    }

    /// Normalizes a URI into a unique caching key.
    pub fn normalize_url(&self, uri: &str) -> Result<String, String> {
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

    /// Installs a host spawner. Without one, tasks go to the fallback queue driven by `run_pending()`.
    pub fn set_spawner(&self, spawner: SpawnFn) {
        *self.spawner.borrow_mut() = Some(spawner);
    }

    /// Gets a cloned reference to the active spawner, if any.
    pub fn spawner(&self) -> Option<SpawnFn> {
        self.spawner.borrow().clone()
    }

    /// Runs a background task via the host spawner, or the fallback queue if none is set.
    pub fn spawn(&self, task: Task) {
        crate::helpers::spawn(self.spawner.borrow().as_ref(), task);
    }

    /// Fetches the cached value for a normalized URL, if it exists.
    pub fn get_cached(&self, norm_url: &str) -> Option<Value> {
        self.cache.borrow().get(norm_url).cloned()
    }

    /// Inserts a value into the document cache.
    pub fn set_cached(&self, norm_url: String, val: Value) {
        self.cache.borrow_mut().insert(norm_url, val);
    }

    /// Fetches the raw document source for `uri` using the matching loader.
    pub async fn load(&self, uri: &str) -> Result<String, String> {
        let loader = if let Ok(parsed) = Url::parse(uri) {
            let scheme = parsed.scheme().to_lowercase();
            self.loaders
                .borrow()
                .get(&scheme)
                .cloned()
                .ok_or_else(|| format!("No loader registered for protocol scheme '{}'", scheme))?
        } else {
            self.default_loader
                .borrow()
                .clone()
                .ok_or_else(|| "No default loader configured for relative/scheme-less URIs".to_string())?
        };

        loader(uri).await
    }
}

/// Asynchronously imports a texl document.
///
/// Loads, renders and updates the registry cache for the specified `doc_uri`. 
/// Returns a namespace on success, or `Nothing` on failure.
///
/// If a failure occurs, the entry is evicted from the cache to allow retries.
pub async fn import(registry: Registry, doc_uri: String, parent: Scope) -> Value {
    let norm_url = match registry.normalize_url(&doc_uri) {
        Ok(url) => url,
        Err(err) => return Value::undef_unary("import.invalid_uri", Value::Text(err.into()), 0),
    };

    match registry.load(&doc_uri).await {
        Ok(source) => {
            // The document scope gets its own `texl` binding *before* rendering,
            // so templates can call `texl.import`.
            let doc_scope = parent.new_child();
            doc_scope.set("texl", create_import_texl_namespace(&doc_scope, registry.clone()));

            let rendering = template::render(&source, &doc_scope).await;

            // The result namespace holds exactly these four fields. It is not a child of
            // doc_scope, so document variables don't leak into it.
            let result = Scope::new();
            result.set("url", Value::Text(norm_url.clone().into()));
            result.set("source", Value::Text(source.into()));
            result.set("context", Value::Namespace(doc_scope));
            result.set("rendering", Value::Text(rendering.into()));

            let result_val = Value::Namespace(result);
            registry.set_cached(norm_url, result_val.clone());
            result_val
        }
        Err(_) => {
            // Evict before resolving, so a retry starts fresh.
            registry.cache.borrow_mut().remove(&norm_url);
            Value::Nothing
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::helpers::{run_pending, spawner_fn};
    use std::cell::Cell;
    use std::pin::pin;
    use std::task::{Context, Poll, Waker};

    // ---- helpers ----------------------------------------------------------

    fn block_on<F: Future>(fut: F) -> F::Output {
        let mut cx = Context::from_waker(Waker::noop());
        let mut fut = pin!(fut);
        loop {
            if let Poll::Ready(v) = fut.as_mut().poll(&mut cx) {
                return v;
            }
        }
    }

    /// Drives the fallback queue, then returns the promise's resolved value.
    fn await_promise(val: Value) -> Value {
        match val {
            Value::Promise(p) => {
                let slot: Rc<RefCell<Option<Value>>> = Rc::new(RefCell::new(None));
                let slot2 = slot.clone();
                let _ = p.then(move |v| {
                    *slot2.borrow_mut() = Some(v);
                    Value::Nothing
                });
                run_pending();
                let out = slot.borrow().clone();
                out.expect("promise did not resolve")
            }
            other => other,
        }
    }

    fn ns_ptr_eq(a: &Value, b: &Value) -> bool {
        match (a, b) {
            (Value::Namespace(x), Value::Namespace(y)) => Rc::ptr_eq(&x.0, &y.0),
            _ => false,
        }
    }

    /// Sets a default loader that always returns `source`; returns its call counter.
    fn counting_loader(registry: &Registry, source: &'static str) -> Rc<Cell<u32>> {
        let counter = Rc::new(Cell::new(0u32));
        let c = counter.clone();
        registry.set_default_loader(loader_fn(move |_| {
            c.set(c.get() + 1);
            async move { Ok::<String, String>(source.to_string()) }
        }));
        counter
    }

    /// Test helper simulating context::create's caching and promise-wrapping 
    fn test_import(registry: Registry, doc_uri: &str, scope: &Scope) -> Value {
        let norm_url = registry.normalize_url(doc_uri).unwrap();
        if let Some(v) = registry.get_cached(&norm_url) {
            return v;
        }
        let registry_clone = registry.clone();
        let doc_uri_str = doc_uri.to_string();
        let scope_clone = scope.clone();
        let promise = crate::helpers::promise_async(registry.spawner().as_ref(), async move {
            import(registry_clone, doc_uri_str, scope_clone).await
        });
        registry.set_cached(norm_url, promise.clone());
        promise
    }

    // ---- registry / normalization ----------------------------------------

    #[test]
    fn test_normalize_url_absolute_and_relative() {
        let registry = Registry::new();
        assert_eq!(
            registry.normalize_url("HTTP://Example.COM:80/foo/../bar").unwrap(),
            "http://example.com/bar"
        );
        assert_eq!(
            registry.normalize_url("https://example.com/a/b/c").unwrap(),
            "https://example.com/a/b/c"
        );
        assert_eq!(
            registry.normalize_url("/path/to/file.texl").unwrap(),
            "/path/to/file.texl"
        );
        assert_eq!(
            registry.normalize_url("relative/file.texl").unwrap(),
            "/relative/file.texl"
        );
    }

    #[test]
    fn test_loader_dispatch_by_scheme() {
        let registry = Registry::new();
        registry.register_loader("http", loader_fn(|uri| async move {
            Ok::<String, String>(format!("http source from {}", uri))
        }));
        registry.register_loader("file", loader_fn(|uri| async move {
            Ok::<String, String>(format!("file content from {}", uri))
        }));
        registry.set_default_loader(loader_fn(|uri| async move {
            Ok::<String, String>(format!("default source from {}", uri))
        }));

        assert_eq!(
            block_on(registry.load("http://example.com/test.texl")).unwrap(),
            "http source from http://example.com/test.texl"
        );
        assert_eq!(
            block_on(registry.load("file:///tmp/doc.texl")).unwrap(),
            "file content from file:///tmp/doc.texl"
        );
        assert_eq!(
            block_on(registry.load("/local/file.texl")).unwrap(),
            "default source from /local/file.texl"
        );
    }

    #[test]
    fn test_unregistered_scheme_and_missing_default_loader() {
        let registry = Registry::new();

        let err_scheme = block_on(registry.load("ftp://example.com/file.texl"));
        assert!(err_scheme.unwrap_err().contains("No loader registered for protocol scheme 'ftp'"));

        let err_default = block_on(registry.load("/path/file.texl"));
        assert!(err_default.unwrap_err().contains("No default loader configured"));
    }

    // ---- import -----------------------------------------------------------

    #[test]
    fn test_import_successful_rendering_and_namespace_structure() {
        let registry = Registry::new();
        registry.set_default_loader(loader_fn(|_| async move {
            Ok::<String, String>("Hello <% name %>! Result: <% 10 + 20 %><% x = 100 %>".to_string())
        }));

        let scope = Scope::new();
        scope.set("name", Value::Text("Texl".into()));

        let result = await_promise(test_import(registry, "/templates/main.texl", &scope));
        let ns = match result {
            Value::Namespace(ns) => ns,
            other => panic!("Expected a Namespace, got {:?}", other),
        };

        assert_eq!(ns.get("url"), Value::Text("/templates/main.texl".into()));
        assert_eq!(
            ns.get("source"),
            Value::Text("Hello <% name %>! Result: <% 10 + 20 %><% x = 100 %>".into())
        );
        assert_eq!(ns.get("rendering"), Value::Text("Hello Texl! Result: 30".into()));

        match ns.get("context") {
            Value::Namespace(ctx) => {
                assert_eq!(ctx.get("name"), Value::Text("Texl".into()));
                assert!(matches!(ctx.get("texl"), Value::Namespace(_)));
                assert!(matches!(ctx.get("x"), Value::Numb(100.0)));
            }
            other => panic!("Expected context to be a Namespace, got {:?}", other),
        }
    }

    #[test]
    fn test_import_caching_and_deduplication() {
        let counter = Rc::new(Cell::new(0u32));
        let c = counter.clone();
        let registry = Registry::new();
        registry.register_loader("http", loader_fn(move |_| {
            c.set(c.get() + 1);
            async move { Ok::<String, String>("Doc".to_string()) }
        }));
        let scope = Scope::new();

        let res1 = await_promise(test_import(registry.clone(), "http://example.com/cached.texl", &scope));
        assert_eq!(counter.get(), 1);

        // Different spelling, same normalized URL.
        let res2 = await_promise(test_import(registry.clone(), "HTTP://EXAMPLE.COM/cached.texl", &scope));
        assert_eq!(counter.get(), 1);

        assert!(ns_ptr_eq(&res1, &res2));
    }

    #[test]
    fn test_concurrent_imports_share_one_promise() {
        let registry = Registry::new();
        let counter = counting_loader(&registry, "shared");
        let scope = Scope::new();

        let p1 = test_import(registry.clone(), "/shared.texl", &scope);
        let p2 = test_import(registry.clone(), "/shared.texl", &scope);
        assert!(matches!(p1, Value::Promise(_)));
        assert!(matches!(p2, Value::Promise(_)));
        assert_eq!(counter.get(), 0); // nothing has run yet

        let r1 = await_promise(p1);
        assert_eq!(counter.get(), 1);
        let r2 = await_promise(p2);
        assert_eq!(counter.get(), 1);
        assert!(ns_ptr_eq(&r1, &r2));
    }

    #[test]
    fn test_import_error_handling_and_cache_eviction() {
        let fail = Rc::new(Cell::new(true));
        let f = fail.clone();
        let registry = Registry::new();
        registry.set_default_loader(loader_fn(move |_| {
            let should_fail = f.get();
            async move {
                if should_fail {
                    Err("Network connection failed".to_string())
                } else {
                    Ok("Recovered content".to_string())
                }
            }
        }));
        let scope = Scope::new();

        // Promises never reject: a failed load resolves to Nothing.
        let first = await_promise(test_import(registry.clone(), "/retry.texl", &scope));
        assert_eq!(first, Value::Nothing);
        assert!(!registry.cache.borrow().contains_key("/retry.texl"));

        fail.set(false);
        match await_promise(test_import(registry.clone(), "/retry.texl", &scope)) {
            Value::Namespace(ns) => {
                assert_eq!(ns.get("rendering"), Value::Text("Recovered content".into()))
            }
            other => panic!("Expected a Namespace on retry, got {:?}", other),
        }
    }

    #[test]
    fn test_recursive_import_does_not_loop() {
        let registry = Registry::new();
        // The document imports itself without rendering the result.
        let counter = counting_loader(&registry, r#"<% p = texl.import("/rec.texl") %>done"#);

        let promise_val = test_import(registry.clone(), "/rec.texl", &Scope::new());

        // The in-flight promise is cached immediately.
        assert!(matches!(
            registry.cache.borrow().get("/rec.texl"),
            Some(Value::Promise(_))
        ));

        match await_promise(promise_val) {
            Value::Namespace(ns) => assert_eq!(ns.get("rendering"), Value::Text("done".into())),
            other => panic!("Expected a Namespace, got {:?}", other),
        }
        assert_eq!(counter.get(), 1);
    }

    #[test]
    fn test_custom_spawner_is_used() {
        let spawned = Rc::new(Cell::new(0u32));
        let s = spawned.clone();
        let registry = Registry::new();
        registry.set_spawner(spawner_fn(move |t| {
            s.set(s.get() + 1);
            crate::helpers::spawn_default(t);
        }));
        registry.set_default_loader(loader_fn(|_| async move { Ok::<String, String>("x".to_string()) }));

        match await_promise(test_import(registry, "/x.texl", &Scope::new())) {
            Value::Namespace(ns) => assert_eq!(ns.get("rendering"), Value::Text("x".into())),
            other => panic!("Expected a Namespace, got {:?}", other),
        }
        assert_eq!(spawned.get(), 1);
    }
}
