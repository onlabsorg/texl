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
//!
//! # Asynchrony
//!
//! Imports run as background tasks. The host can install its own spawner with
//! [`Registry::set_spawner`]. Otherwise tasks go to a thread-local fallback queue that
//! the host must drive by calling [`run_pending`].

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Waker};

use swan_core::value::{Promise, Scope, Value};
use url::Url;

use crate::context::create_import_texl_namespace;
use crate::template;

/// An external, host-provided document loader.
///
/// Receives the URI exactly as given to [`import`] (not normalized) and returns a future
/// yielding the document source, or an error message. Build one with [`loader_fn`].
pub type LoaderFn = Rc<dyn Fn(&str) -> Pin<Box<dyn Future<Output = Result<String, String>>>>>;

/// A background task, as handed to a spawner. It is not `Send`: tasks run on the thread that created them.
pub type Task = Pin<Box<dyn Future<Output = ()>>>;

/// A host-provided function that runs a background task to completion.
///
/// The host decides how the task is driven (e.g. `wasm_bindgen_futures::spawn_local`
/// or a local executor). Build one with [`spawner_fn`].
pub type SpawnFn = Rc<dyn Fn(Task)>;

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

/// Builds a [`SpawnFn`] from a plain closure.
pub fn spawner_fn(f: impl Fn(Task) + 'static) -> SpawnFn {
    Rc::new(f)
}

thread_local! {
    /// Queue of tasks spawned via [`spawn_default`], awaiting [`run_pending`].
    static LOCAL_TASKS: RefCell<VecDeque<Task>> = RefCell::new(VecDeque::new());
}

/// Fallback spawner: queues the task on a thread-local queue.
///
/// The task does not run until [`run_pending`] is called.
pub fn spawn_default(task: Task) {
    LOCAL_TASKS.with(|q| q.borrow_mut().push_back(task));
}

/// Polls queued fallback tasks until no further progress is possible.
///
/// Uses a no-op waker, so it cannot be woken by external events. It stops when a full
/// pass completes no task and spawns no new one. Tasks still waiting on external events
/// stay queued, and a later call retries them.
///
/// Hosts that don't install their own spawner (see [`Registry::set_spawner`]) must call
/// this to drive imports.
pub fn run_pending() {
    let mut cx = Context::from_waker(Waker::noop());
    loop {
        // Take the batch out first so tasks can call `spawn_default` while being polled.
        let batch: VecDeque<Task> = LOCAL_TASKS.with(|q| std::mem::take(&mut *q.borrow_mut()));
        if batch.is_empty() {
            break;
        }
        let mut progressed = false;
        let mut still_pending = 0;
        for mut task in batch {
            if task.as_mut().poll(&mut cx).is_ready() {
                progressed = true;
            } else {
                still_pending += 1;
                LOCAL_TASKS.with(|q| q.borrow_mut().push_back(task));
            }
        }
        let queued = LOCAL_TASKS.with(|q| q.borrow().len());
        // Nothing finished and nothing new was spawned: all remaining tasks wait on external events.
        if !progressed && queued == still_pending {
            break;
        }
    }
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
    ///
    /// The scheme is lowercased. Registering the same scheme again replaces the previous loader.
    pub fn register_loader(&self, protocol: impl Into<String>, loader: LoaderFn) {
        let scheme = protocol.into().to_lowercase();
        self.loaders.borrow_mut().insert(scheme, loader);
    }

    /// Sets the default loader for scheme-less/protocol-less URIs (e.g., "/path/to/file.texl").
    ///
    /// Used when the URI cannot be parsed as an absolute URL.
    pub fn set_default_loader(&self, loader: LoaderFn) {
        *self.default_loader.borrow_mut() = Some(loader);
    }

    /// Normalizes a URI into a unique caching key.
    ///
    /// - Absolute URLs are canonicalized by the `url` crate (lowercased scheme and host,
    ///   default port removed, dot segments resolved), e.g.
    ///   `HTTP://Example.COM:80/foo/../bar` becomes `http://example.com/bar`.
    /// - Scheme-less URIs are treated as absolute paths: a leading `/` is ensured, dot
    ///   segments are resolved, and any query is kept (a fragment is dropped), e.g.
    ///   `relative/file.texl` becomes `/relative/file.texl`.
    /// - Input that cannot be normalized either way is returned trimmed.
    ///
    /// The `Result` return type allows for future stricter validation; currently every
    /// input yields `Ok`.
    pub fn normalize_url(&self, uri: &str) -> Result<String, String> {
        if let Ok(parsed) = Url::parse(uri) {
            // Standard URL normalization (canonicalizes host, port, path, scheme)
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
    /// Replaces any previously installed spawner.
    pub fn set_spawner(&self, spawner: SpawnFn) {
        *self.spawner.borrow_mut() = Some(spawner);
    }

    /// Runs a background task via the host spawner, or the fallback queue if none is set.
    pub fn spawn(&self, task: Task) {
        let spawner = self.spawner.borrow().clone();
        match spawner {
            Some(s) => s(task),
            None => spawn_default(task),
        }
    }

    /// Fetches the raw document source for `uri` using the matching loader.
    ///
    /// The URI is parsed with the `url` crate. If it is absolute, the loader registered
    /// for its (lowercased) scheme is used; otherwise the default loader is used. The
    /// loader receives `uri` as given, not normalized.
    ///
    /// # Errors
    ///
    /// Returns an error if no loader is registered for the scheme, if no default loader is
    /// configured for a scheme-less URI, or if the loader itself fails.
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

/// Imports a texl document.
///
/// Returns immediately. The result is normally a Swan promise that resolves to a
/// `{url, source, context, rendering}` namespace (see the module docs), or to `Nothing`
/// if loading fails (Swan promises never reject). If `doc_uri` cannot be normalized,
/// it returns an `import.invalid_uri` undefined value instead of a promise.
///
/// `scope` is the parent scope of the imported document's scope.
///
/// - Loaded and in-flight documents are served from the cache, so each document is
///   fetched once and recursive imports don't loop. Awaiting an in-flight import of
///   yourself inside a `<% %>` tag cannot complete; assigning it without rendering it
///   is fine.
/// - Failed loads are evicted from the cache so they can be retried.
/// - Loading and rendering run as a background task started through
///   [`Registry::spawn`]; the promise resolves when that task finishes.
pub fn import(registry: Registry, doc_uri: &str, scope: &Scope) -> Value {
    let norm_url = match registry.normalize_url(doc_uri) {
        Ok(url) => url,
        Err(err) => return Value::undef_unary("import.invalid_uri", Value::Text(err.into()), 0),
    };

    let cached = registry.cache.borrow().get(&norm_url).cloned();
    if let Some(v) = cached {
        return v;
    }

    let promise = Promise::new();
    let promise_val = Value::Promise(promise.clone());
    registry.cache.borrow_mut().insert(norm_url.clone(), promise_val.clone());

    let task = run_import(registry.clone(), doc_uri.to_string(), norm_url, scope.clone(), promise);
    registry.spawn(Box::pin(task));
    promise_val
}

/// Background task behind [`import`]: loads, renders and caches one document, then
/// resolves `promise`.
///
/// On success, the document gets a child scope of `parent` with its own `texl`
/// namespace (so templates can call `texl.import`). The result namespace is a separate
/// scope holding only `url`, `source`, `context` and `rendering`, and it replaces the
/// pending promise in the cache. On failure, the cache entry is removed and the promise
/// resolves to `Nothing`.
async fn run_import(
    registry: Registry,
    doc_uri: String,
    norm_url: String,
    parent: Scope,
    promise: Promise,
) {
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
            registry.cache.borrow_mut().insert(norm_url, result_val.clone());
            promise.resolve(result_val);
        }
        Err(_) => {
            // Evict before resolving, so a retry triggered from a promise callback starts fresh.
            registry.cache.borrow_mut().remove(&norm_url);
            promise.resolve(Value::Nothing);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::pin::pin;
    use std::task::Poll;

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

        let result = await_promise(import(registry, "/templates/main.texl", &scope));
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

        let res1 = await_promise(import(registry.clone(), "http://example.com/cached.texl", &scope));
        assert_eq!(counter.get(), 1);

        // Different spelling, same normalized URL.
        let res2 = await_promise(import(registry.clone(), "HTTP://EXAMPLE.COM/cached.texl", &scope));
        assert_eq!(counter.get(), 1);

        assert!(ns_ptr_eq(&res1, &res2));
    }

    #[test]
    fn test_concurrent_imports_share_one_promise() {
        let registry = Registry::new();
        let counter = counting_loader(&registry, "shared");
        let scope = Scope::new();

        let p1 = import(registry.clone(), "/shared.texl", &scope);
        let p2 = import(registry.clone(), "/shared.texl", &scope);
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
        let first = await_promise(import(registry.clone(), "/retry.texl", &scope));
        assert_eq!(first, Value::Nothing);
        assert!(!registry.cache.borrow().contains_key("/retry.texl"));

        fail.set(false);
        match await_promise(import(registry.clone(), "/retry.texl", &scope)) {
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

        let promise_val = import(registry.clone(), "/rec.texl", &Scope::new());

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
            spawn_default(t);
        }));
        registry.set_default_loader(loader_fn(|_| async move { Ok::<String, String>("x".to_string()) }));

        match await_promise(import(registry, "/x.texl", &Scope::new())) {
            Value::Namespace(ns) => assert_eq!(ns.get("rendering"), Value::Text("x".into())),
            other => panic!("Expected a Namespace, got {:?}", other),
        }
        assert_eq!(spawned.get(), 1);
    }
}
