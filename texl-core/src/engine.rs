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

/// An external, host-provided document loader.
pub type LoaderFn = Rc<dyn Fn(&str) -> Pin<Box<dyn Future<Output = Result<String, String>>>>>;

/// Builds a [`LoaderFn`] from a plain closure returning a future.
pub fn loader_fn<F, Fut>(f: F) -> LoaderFn
where
    F: Fn(String) -> Fut + 'static,
    Fut: Future<Output = Result<String, String>> + 'static,
{
    Rc::new(move |uri: &str| Box::pin(f(uri.to_string())))
}

#[derive(Clone)]
pub struct Engine {
    base_scope: Scope,
    loaders: Rc<RefCell<HashMap<String, LoaderFn>>>,
    default_loader: Rc<RefCell<Option<LoaderFn>>>,
    cache: Rc<RefCell<HashMap<String, Value>>>,
    spawner: Rc<RefCell<Option<SpawnFn>>>,
}

impl Engine {
    /// 1. Initializes the Engine by passing the spawner to the constructor
    pub fn new(spawner: Option<SpawnFn>) -> Self {
        Self {
            base_scope: root_scope(),
            loaders: Rc::new(RefCell::new(HashMap::new())),
            default_loader: Rc::new(RefCell::new(None)),
            cache: Rc::new(RefCell::new(HashMap::new())),
            spawner: Rc::new(RefCell::new(spawner)),
        }
    }

    /// 2. Adds builtins to the scope without requiring the host to create one first
    pub fn add_builtin(&self, name: &str, value: Value) {
        self.base_scope.set(name, value);
    }

    /// 3a. Adds custom scheme loaders to the engine
    pub fn add_loader(&self, protocol: &str, loader: LoaderFn) {
        let scheme = protocol.to_lowercase();
        self.loaders.borrow_mut().insert(scheme, loader);
    }

    /// 3b. Adds the default fallback loader
    pub fn add_default_loader(&self, loader: LoaderFn) {
        *self.default_loader.borrow_mut() = Some(loader);
    }

    /// 4. Loads a document (public entrypoint for the host)
    pub async fn load(&self, uri: &str) -> Value {
        let norm_url = match self.normalize_url(uri) {
            Ok(url) => url,
            Err(err) => return Value::undef_unary("import.invalid_uri", Value::Text(err.into()), 0),
        };
        let doc_scope = self.create_scope();
        self.load_internal(uri.to_string(), doc_scope, norm_url).await
    }

    /// 5. Renders a template directly, creating a fresh scope automatically
    pub async fn render(&self, template: &str) -> String {
        let scope = self.create_scope();
        template::render(template, &scope).await
    }

    /// 6. Scope building is exposed, but generally handled internally
    pub fn create_scope(&self) -> Scope {
        // Create a child so document executions do not pollute the base_scope
        let child = self.base_scope.new_child();

        // Create a standalone `texl` namespace and attach it
        let texl_ns = Scope::new();
        texl_ns.set("import", self.create_import_builtin(child.clone()));
        child.set("texl", Value::Namespace(texl_ns));

        child
    }

    // --- Internal Helpers ---

    /// Creates the `texl.import` builtin bound to a specific document scope
    fn create_import_builtin(&self, parent_scope: Scope) -> Value {
        let engine = self.clone();
        Value::Builtin(BuiltinFn::new("texl.import", move |arg| {
            match arg {
                Value::Text(uri) => {
                    let norm_url = match engine.normalize_url(&uri) {
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
                    let uri_str = uri.to_string();
                    let norm_url_key = norm_url.clone();
                    let parent_scope_clone = parent_scope.clone();

                    let spawner = engine.spawner.borrow().clone();
                    let promise = promise_async(
                        spawner.as_ref(),
                        async move {
                            // 1. Create a child scope so the document inherits the caller's variables
                            let doc_scope = parent_scope_clone.new_child();
                            
                            // 2. Bind the texl namespace securely to the new document scope
                            let texl_ns = Scope::new();
                            texl_ns.set("import", engine_clone.create_import_builtin(doc_scope.clone()));
                            doc_scope.set("texl", Value::Namespace(texl_ns));

                            engine_clone.load_internal(uri_str, doc_scope, norm_url_key).await
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
    async fn load_internal(&self, doc_uri: String, doc_scope: Scope, norm_url: String) -> Value {
        let loader = if let Ok(parsed) = Url::parse(&doc_uri) {
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

        match loader(&doc_uri).await {
            Ok(source) => {
                let rendering = template::render(&source, &doc_scope).await;

                let result = Scope::new();
                result.set("url", Value::Text(norm_url.clone().into()));
                result.set("source", Value::Text(source.into()));
                result.set("context", Value::Namespace(doc_scope));
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

        assert_eq!(ns.get("url"), Value::Text("/math.texl".into()));
        assert_eq!(ns.get("rendering"), Value::Text("Result is 50".into()));
        
        // Ensure the evaluated context contains the assignments
        let doc_ctx = unwrap_namespace(ns.get("context"));
        assert_eq!(doc_ctx.get("result"), Value::Numb(50.0));
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

    #[test]
    fn test_engine_handles_missing_loader_gracefully() {
        let engine = Engine::new(None);
        // We purposefully do not add an ftp loader or a default loader
        
        let out = block_on(engine.load("ftp://example.com/file.texl"));
        
        // Should gracefully fail to Value::Nothing per the import protocol
        assert_eq!(out, Value::Nothing);
    }
}
