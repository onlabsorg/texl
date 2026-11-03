//! The texl engine: documents, loaders and imports.
//!
//! An [`Engine`] renders *documents*: templates identified by a URL. It holds
//! configuration only:
//!
//! - the **globals** added by the host with [`Engine::add_builtin`], copied into
//!   every new scope next to the Swan built-ins;
//! - a set of **loaders** that fetch the source of a document from its URL,
//!   one per URL scheme ([`Engine::add_loader`]) plus a default loader for
//!   paths without a scheme ([`Engine::add_default_loader`]).
//!
//! Each method does one thing, and the others are built from it:
//! [`Engine::fetch`] returns the source of a document, [`Engine::render`]
//! renders a source as the document at a URL, and [`Engine::load`] is
//! `fetch` followed by `render`.
//!
//! # Documents
//!
//! Rendering a document produces a namespace with five fields:
//!
//! | Field       | Type        | Content                                          |
//! |-------------|-------------|--------------------------------------------------|
//! | `uri`       | `Text`      | The normalized URL of the document               |
//! | `source`    | `Text`      | The template source                              |
//! | `data`      | `Namespace` | The names bound by the template                  |
//! | `text`      | `Text`      | The rendered text                                |
//! | `tags`      | `List`      | One namespace per tag, in source order           |
//!
//! Each item of `tags` is a namespace `{start, end, out_start, out_end,
//! value}` (see [`template::Tag`]): the byte span of the tag in `source`
//! (from `<%` to after `%>`), the byte span of its text in `text`, and the
//! value that was rendered. Tags that render nothing, such as assignments,
//! are included, so a host can find an undefined value whatever its tag
//! renders. Undefined values made by the document know its URL and text,
//! and [`document::locate`] gives their position as a byte offset in
//! `source`. A document holds every tag value as long as it lives; the
//! values are freed with it.
//!
//! The `data` field holds every name the template bound, with `=`, `:`,
//! `bind` or `label`, so a document can act as a library of values and
//! functions. It is a snapshot taken when the rendering ends: like every
//! namespace, it does not change afterwards.
//!
//! # Imports
//!
//! Every document has a `texl` namespace containing `texl.import`. Templates
//! call `texl.import(uri)` to import another document: it fetches the
//! document and renders it the same way. It returns a promise of the document
//! namespace (or of `Nothing`, if the document cannot be loaded). Relative
//! URIs resolve against the URL of the importing document.
//!
//! - Each document renders in its own scope from [`Engine::create_scope`],
//!   not in a child of its importer's scope: it sees the built-ins and the
//!   globals, but not the names of the document that imports it. `bind` and
//!   `label` bind names in the document's own scope, the scope of its tags,
//!   which no other document sees.
//! - Each [`Engine::render`] has its own import cache, dropped with it: once the
//!   host drops the returned document, the documents of the rendering are freed.
//!   A document that defines functions is in a reference cycle (its functions
//!   capture its scope); the cycle collector ([`swan_core::gc`]), which
//!   [`Engine::render`] runs when it is done, frees it. Within
//!   one rendering, a document is fetched and rendered at most once, and
//!   concurrent imports of it share that rendering. Nothing is cached across
//!   renders: the next rendering fetches its imports again.
//! - The cache key is the resolved URL without its fragment, so `doc.texl#a`
//!   and `doc.texl#b` are one document. The query is kept as it is.
//! - An import that would wait for a document that is itself waiting for the
//!   importer (a cycle, such as a document importing itself) returns
//!   `Undefined` instead of a promise.
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
//! let source = r#"<% texl.import("greeting.texl") ~> (doc -> doc.text) %>"#;
//! let Value::Namespace(doc) = block_on(engine.render("/main.texl", source)) else { panic!() };
//! assert_eq!(doc.get("text"), Value::Text("Hello Ada!".into()));
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
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::rc::{Rc, Weak};
use url::Url;

use swan_core::builtins::root_scope;
use swan_core::gc;
use swan_core::value::{BuiltinFn, Namespace, Promise, Scope, Value};

use crate::helpers::{spawn, SpawnFn};
use crate::document::{self, Document};
use crate::template;

/// A host-provided function that fetches the source of a document.
///
/// The engine calls it with the normalized URL of the document, without
/// fragment: a full URL (`https://host/dir/doc.texl`) for scheme loaders, or
/// an absolute, percent-encoded path (`/dir/doc.texl`) for the default loader.
/// The future resolves to the template source, or to an error message. The
/// engine discards the message: a failed load gives `Nothing`.
///
/// The engine does not cache sources. A host that wants to can wrap its
/// loaders.
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

/// Renders and loads documents. See the [module documentation](self).
///
/// `Engine` is a cheap handle: clones share the same globals and loaders.
/// It is single-threaded (not `Send`).
#[derive(Clone)]
pub struct Engine {
    builtins: Rc<RefCell<HashMap<String, Value>>>,
    loaders: Rc<RefCell<HashMap<String, LoaderFn>>>,
    default_loader: Rc<RefCell<Option<LoaderFn>>>,
    spawner: Rc<RefCell<Option<SpawnFn>>>,
}

/// The documents of one rendering: its import cache and its wait-for graph.
#[derive(Default)]
struct Imports {
    /// The documents of the rendering, by URL without fragment, as promises of their namespaces
    docs: HashMap<String, Promise>,
    /// The wait-for graph: `waits[x]` holds the URLs of the pending documents that `x` waits for
    waits: HashMap<String, HashSet<String>>,
}

impl Imports {
    /// Whether the document at `from` is the one at `to`, or waits for it, directly or not
    fn reaches(&self, from: &str, to: &str) -> bool {
        let mut stack = vec![from];
        let mut seen = HashSet::new();
        while let Some(url) = stack.pop() {
            if url == to {
                return true;
            }
            if seen.insert(url) {
                if let Some(next) = self.waits.get(url) {
                    stack.extend(next.iter().map(String::as_str));
                }
            }
        }
        false
    }

    /// Removes the edges into `url`, once its document has resolved
    fn resolved(&mut self, url: &str) {
        for targets in self.waits.values_mut() {
            targets.remove(url);
        }
    }
}

impl Engine {
    /// Creates an engine with no loaders.
    ///
    /// Imports run as background tasks. `spawner` hands those tasks to the
    /// host's async runtime (for example `tokio::task::spawn_local`, or
    /// `wasm_bindgen_futures::spawn_local`). With `None`, tasks go onto a
    /// thread-local queue, and the host must call
    /// [`helpers::run_pending`](crate::helpers::run_pending) to run them.
    ///
    /// The engine starts with one global, `undef`: Swan's `undef` namespace
    /// with an `inspect` that gives positions in the document and its URI
    /// (see [`document::members`]).
    pub fn new(spawner: Option<SpawnFn>) -> Self {
        let undef = document::undef_namespace(root_scope().get("undef"));
        Self {
            builtins: Rc::new(RefCell::new(HashMap::from([("undef".to_string(), undef)]))),
            loaders: Rc::new(RefCell::new(HashMap::new())),
            default_loader: Rc::new(RefCell::new(None)),
            spawner: Rc::new(RefCell::new(spawner)),
        }
    }

    /// Adds a global: `name` is bound to `value` in every scope that the engine
    /// creates from now on, unless a template shadows it.
    ///
    /// Scopes created earlier, such as a scope from [`Engine::create_scope`] or a
    /// rendering in progress, do not see it. A later call with the same name
    /// replaces the value. Use it to pass data and host functions
    /// ([`Value::Builtin`]) to templates.
    pub fn add_builtin(&self, name: &str, value: Value) {
        self.builtins.borrow_mut().insert(name.to_string(), value);
    }

    /// Registers `loader` for the URLs whose scheme is `protocol` (for example
    /// `"https"` or `"file"`, without the colon). The scheme is
    /// case-insensitive. A later call for the same scheme replaces the loader.
    pub fn add_loader(&self, protocol: &str, loader: LoaderFn) {
        let scheme = protocol.to_lowercase();
        self.loaders.borrow_mut().insert(scheme, loader);
    }

    /// Registers the loader for URLs without a scheme, such as `/dir/doc.texl`
    /// or `doc.texl`. A later call replaces it.
    pub fn add_default_loader(&self, loader: LoaderFn) {
        *self.default_loader.borrow_mut() = Some(loader);
    }

    /// Returns the source of the document at `url`.
    ///
    /// The URL is normalized (a scheme-less path becomes absolute, so `doc.texl`
    /// is `/doc.texl`, and the fragment is removed), then passed to the loader
    /// of its scheme, or to the default loader if it has none. Nothing is
    /// cached: every call runs the loader.
    ///
    /// Returns `None` if no loader matches the URL or the loader fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use texl_core::{Engine, loader_fn};
    ///
    /// let engine = Engine::new(None);
    /// engine.add_loader("mem", loader_fn(|uri| async move { Ok(format!("source of {uri}")) }));
    ///
    /// assert_eq!(block_on(engine.fetch("mem://doc#part")), Some("source of mem://doc".to_string()));
    /// assert_eq!(block_on(engine.fetch("ftp://host/doc")), None);
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
    pub async fn fetch(&self, url: &str) -> Option<String> {
        let url = normalize_url(url);
        let loader = match Url::parse(&url) {
            Ok(parsed) => self.loaders.borrow().get(parsed.scheme()).cloned(),
            Err(_) => self.default_loader.borrow().clone(),
        }?;
        loader(&url).await.ok()
    }

    /// Renders `source` as the document at `url`.
    ///
    /// The source is rendered in a new scope from [`Engine::create_scope`], whose
    /// `texl.import` resolves relative URIs against `url`. The rendering gets a
    /// new import cache, where the document itself is pending until it is
    /// rendered: an import of `url` from inside the rendering is a cycle, and
    /// returns `Undefined`. Nothing is fetched for `url`.
    ///
    /// When the rendering is done, it runs the cycle collector
    /// ([`swan_core::gc::collect`]), which frees the documents the host no
    /// longer holds.
    ///
    /// `url` is normalized like the URL of [`Engine::fetch`]. A host with no real
    /// location for the source (standard input, an unsaved buffer, a test)
    /// passes a URL of its choice, such as the path of a file in the working
    /// directory.
    ///
    /// Returns the document namespace `{uri, source, data, text, tags}` (see the
    /// [module documentation](self)).
    ///
    /// # Examples
    ///
    /// ```
    /// use swan_core::value::Value;
    /// use texl_core::{Engine, loader_fn};
    ///
    /// let engine = Engine::new(None);
    /// engine.add_default_loader(loader_fn(|uri| async move {
    ///     match uri.as_str() {
    ///         "/docs/name.texl" => Ok("Ada".to_string()),
    ///         _ => Err(format!("{uri}: not found")),
    ///     }
    /// }));
    ///
    /// let source = r#"Hello <% texl.import("name.texl") ~> (doc -> doc.text) %>!"#;
    /// let Value::Namespace(doc) = block_on(engine.render("docs/main.texl", source)) else { panic!() };
    /// assert_eq!(doc.get("uri"), Value::Text("/docs/main.texl".into()));
    /// assert_eq!(doc.get("text"), Value::Text("Hello Ada!".into()));
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
    pub async fn render(&self, url: &str, source: &str) -> Value {
        let url = normalize_url(url);
        let imports = Rc::new(RefCell::new(Imports::default()));

        // The root document is pending while it renders, so importing it is a cycle
        let root = Promise::new();
        imports.borrow_mut().docs.insert(url.clone(), root.clone());

        let doc = self.render_document(&imports, &url, source).await;
        imports.borrow_mut().resolved(&url);
        root.resolve(doc.clone());

        // Free the documents and values that only reference cycles keep alive
        gc::collect();
        doc
    }

    /// Fetches and renders the document at `url`: [`Engine::fetch`] followed by
    /// [`Engine::render`].
    ///
    /// # Returns
    ///
    /// - The document namespace `{uri, source, data, text, tags}` (see the
    ///   [module documentation](self)).
    /// - `Nothing` if no loader matches the URL or the loader fails. The
    ///   document is not rendered.
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
    /// assert_eq!(doc.get("text"), Value::Text("x is 1".into()));
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
    pub async fn load(&self, url: &str) -> Value {
        match self.fetch(url).await {
            Some(source) => self.render(url, &source).await,
            None => Value::Nothing,
        }
    }

    /// Creates a new scope with the built-ins and the globals of the engine.
    ///
    /// The scope is a new [`root_scope`], where `bind` and `label` bind names.
    /// The globals of the engine are copied into its parent, so they are not
    /// bindings of the scope itself. It has its own `texl` namespace. The `texl`
    /// namespace of a scope from this method alone has no `import`: such a
    /// scope cannot import documents. [`Engine::render`] adds it.
    ///
    /// Nothing else refers to the new scope, so it is freed with the last
    /// reference to it.
    ///
    /// Use it to render several templates in the same scope with
    /// [`template::render`] and share names between them.
    ///
    /// # Examples
    ///
    /// ```
    /// use swan_core::value::Value;
    /// use texl_core::{Engine, template};
    ///
    /// let engine = Engine::new(None);
    /// let scope = engine.create_scope();
    /// block_on(template::render("<% x = 20 %>", &scope));
    /// assert_eq!(block_on(template::render("x + 1 = <% x + 1 %>", &scope)).text, "x + 1 = 21");
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
    pub fn create_scope(&self) -> Scope {
        let scope = root_scope();

        // The parent of a root scope is the layer holding `bind` and `label`, private to it
        if let Some(globals) = scope.parent() {
            for (name, value) in self.builtins.borrow().iter() {
                globals.set(name.as_str(), value.clone());
            }
        }

        scope.set("texl", Value::Namespace(Namespace::new()));
        scope
    }

    // --- Internal Helpers ---

    /// Renders `source` as the document at the normalized `url`, importing with `imports`
    async fn render_document(&self, imports: &Rc<RefCell<Imports>>, url: &str, source: &str) -> Value {
        let scope = self.create_scope();
        let texl: Namespace = [("import", self.import_builtin(imports.clone(), url.to_string()))]
            .into_iter().collect();
        scope.set("texl", Value::Namespace(texl));

        let rendering = template::render(Document::new(Some(url), source), &scope).await;
        let tags = rendering.tags.iter().map(|tag| Value::Namespace(tag.to_namespace())).collect();

        let doc: Namespace = [
            ("uri", Value::Text(url.into())),
            ("source", Value::Text(source.into())),
            ("data", Value::Namespace(rendering.data)),
            ("text", Value::Text(rendering.text.into())),
            ("tags", Value::list(tags)),
        ].into_iter().collect();
        Value::Namespace(doc)
    }

    /// Creates the `texl.import` builtin of the document at `importer`.
    ///
    /// The builtin holds `imports` weakly: the documents in `imports` hold it through their
    /// `texl` namespaces. The rendering and its running imports hold `imports`; once they are
    /// done, a call (from a function in a document's `data`) starts a new import cache.
    fn import_builtin(&self, imports: Rc<RefCell<Imports>>, importer: String) -> Value {
        let engine = self.clone();
        let imports = Rc::downgrade(&imports);
        Value::Builtin(BuiltinFn::new("texl.import", move |arg| match arg {
            Value::Text(uri) => match resolve_url(&uri, &importer) {
                Ok(url) => {
                    let imports = Weak::upgrade(&imports).unwrap_or_default();
                    engine.import(&imports, &importer, url)
                }
                Err(err) => Value::undef_unary("import.invalid_uri", Value::Text(err.into())),
            },
            other => Value::undef_unary("texl.import", other),
        }))
    }

    /// Imports the document at the normalized `url` for the document at `importer`
    fn import(&self, imports: &Rc<RefCell<Imports>>, importer: &str, url: String) -> Value {
        let mut state = imports.borrow_mut();

        if let Some(doc) = state.docs.get(&url).cloned() {
            if doc.0.borrow().value.is_none() {
                // Waiting for a document that waits for the importer would never end
                if state.reaches(&url, importer) {
                    return Value::undef_unary("import.cycle", Value::Text(url.into()));
                }
                state.waits.entry(importer.to_string()).or_default().insert(url);
            }
            return Value::Promise(doc);
        }

        // Cache the promise before the import starts, so concurrent imports share it
        let doc = Promise::new();
        state.docs.insert(url.clone(), doc.clone());
        state.waits.entry(importer.to_string()).or_default().insert(url.clone());
        drop(state);

        let engine = self.clone();
        let imports = imports.clone();
        let promise = doc.clone();
        let task = Box::pin(async move {
            let result = match engine.fetch(&url).await {
                Some(source) => engine.render_document(&imports, &url, &source).await,
                None => {
                    // A failed load is not cached: a later import tries again
                    imports.borrow_mut().docs.remove(&url);
                    Value::Nothing
                }
            };
            imports.borrow_mut().resolved(&url);
            promise.resolve(result);
        });
        spawn(self.spawner.borrow().as_ref(), task);

        Value::Promise(doc)
    }
}

/// The members of a document namespace (see the [module documentation](self)).
const DOCUMENT_MEMBERS: [&str; 5] = ["data", "source", "tags", "text", "uri"];

/// True if `ns` has exactly the members of a document: `uri`, `source`,
/// `data`, `text` and `tags`.
pub fn is_document(ns: &Namespace) -> bool {
    ns.names() == DOCUMENT_MEMBERS
}

/// Returns a copy of the document `doc` whose tag positions are converted
/// with `convert(text, byte_offset)`: `start` and `end` against `source`,
/// `out_start` and `out_end` against `text`. Bindings use it to give hosts
/// positions in their own unit, with [`document::utf16_offset`] or
/// [`document::char_offset`], and to convert them back to bytes when a
/// document comes back from the host, with [`document::utf16_to_byte_offset`]
/// or [`document::char_to_byte_offset`], so that a document round-trips.
/// Returns `None` if `doc` is not a document.
pub fn convert_tag_positions(doc: &Namespace, convert: impl Fn(&str, usize) -> usize) -> Option<Namespace> {
    if !is_document(doc) { return None; }
    let (Value::Text(source), Value::Text(text), Value::List(tags)) =
        (doc.get("source"), doc.get("text"), doc.get("tags")) else { return None };
    let conv = |ns: &Namespace, name: &str, against: &str| match ns.get(name) {
        Value::Numb(n) => Value::Numb(convert(against, n as usize) as f64),
        other => other,
    };
    let tags: Vec<Value> = tags.iter().map(|tag| match tag {
        Value::Namespace(t) => {
            let mut t = t.clone();
            t.insert("start", conv(&t, "start", &source));
            t.insert("end", conv(&t, "end", &source));
            t.insert("out_start", conv(&t, "out_start", &text));
            t.insert("out_end", conv(&t, "out_end", &text));
            Value::Namespace(t)
        }
        other => other.clone(),
    }).collect();
    let mut doc = doc.clone();
    doc.insert("tags", Value::list(tags));
    Some(doc)
}

/// Resolves `uri` against `base_url` (the URL of the importing document) and normalizes it.
/// URIs with a scheme ignore the base. Absolute paths (starting with `/`) resolve against
/// the root of a full-URL base (`/p` from `http://host/dir/doc` is `http://host/p`), and
/// stay scheme-less otherwise. The fragment is removed.
fn resolve_url(uri: &str, base_url: &str) -> Result<String, String> {
    if Url::parse(uri).is_ok() {
        return Ok(normalize_url(uri));
    }

    if let Ok(base) = Url::parse(base_url) {
        // The base is a full URL (e.g. `http://host/dir/doc.texl`)
        let mut joined = base.join(uri).map_err(|e| e.to_string())?;
        joined.set_fragment(None);
        Ok(joined.to_string())
    } else if uri.starts_with('/') {
        Ok(normalize_url(uri))
    } else {
        // The base is a scheme-less path: resolve it under a dummy scheme, then strip it
        let dummy_base = Url::parse(&format!("app:///{}", base_url.trim_start_matches('/')))
            .map_err(|e| e.to_string())?;
        let joined = dummy_base.join(uri).map_err(|e| e.to_string())?;
        Ok(path_and_query(&joined))
    }
}

/// Normalizes a URL: a full URL is parsed and printed again, a scheme-less path becomes
/// absolute and percent-encoded. The fragment is removed; the query is kept as it is.
fn normalize_url(uri: &str) -> String {
    match Url::parse(uri) {
        Ok(mut parsed) => {
            parsed.set_fragment(None);
            parsed.to_string()
        }
        Err(_) => match Url::parse(&format!("app:///{}", uri.trim_start_matches('/'))) {
            Ok(dummy) => path_and_query(&dummy),
            Err(_) => uri.trim().to_string(),
        },
    }
}

/// The path of `url`, followed by its query if it has one
fn path_and_query(url: &Url) -> String {
    let mut norm = url.path().to_string();
    if let Some(q) = url.query() {
        norm.push('?');
        norm.push_str(q);
    }
    norm
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::helpers::{promise_async, run_pending};
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
            run_pending();

            if let Poll::Ready(v) = fut.as_mut().poll(&mut cx) {
                return v;
            }
        }
    }

    /// Test helper to extract a namespace from a Value or panic
    fn unwrap_namespace(val: Value) -> Namespace {
        match val {
            Value::Namespace(ns) => ns,
            other => panic!("Expected Namespace, got {:?}", other),
        }
    }

    /// The rendering of a document namespace
    fn rendering(doc: Value) -> String {
        match unwrap_namespace(doc).get("text") {
            Value::Text(text) => text.to_string(),
            other => panic!("Expected Text, got {:?}", other),
        }
    }

    /// The `data` field of a document namespace
    fn data(doc: Value) -> Namespace {
        unwrap_namespace(unwrap_namespace(doc).get("data"))
    }

    /// Builds an engine whose loaders serve sources from `docs` (keyed by the URL the loader
    /// receives) and record every URL they are asked to load.
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

    const RENDERING: &str = "~> (ns -> ns.text)";

    // --- Tests ---

    #[test]
    fn test_engine_renders_basic_template() {
        let engine = Engine::new(None);
        let doc = block_on(engine.render("/main.texl", "Hello <% 10 + 20 %>!"));
        assert_eq!(rendering(doc), "Hello 30!");
    }

    #[test]
    fn test_engine_render_returns_complete_namespace() {
        let engine = Engine::new(None);
        let ns = unwrap_namespace(block_on(engine.render("docs/main.texl#top", "<% x = 1 %>x is <% x %>")));

        assert_eq!(ns.get("uri"), Value::Text("/docs/main.texl".into()));
        assert_eq!(ns.get("source"), Value::Text("<% x = 1 %>x is <% x %>".into()));
        assert_eq!(ns.get("text"), Value::Text("x is 1".into()));
        assert_eq!(unwrap_namespace(ns.get("data")).get("x"), Value::Numb(1.0));
    }

    #[test]
    fn test_engine_injects_builtins() {
        let engine = Engine::new(None);
        let scope = engine.create_scope();
        engine.add_builtin("system_name", Value::Text("TexlCore".into()));
        engine.add_builtin("version", Value::Numb(2.0));

        let doc = block_on(engine.render("/main.texl", "<% system_name %> v<% version %>"));
        assert_eq!(rendering(doc), "TexlCore v2");
        // Scopes created earlier do not see them
        assert_eq!(scope.get("version"), Value::Nothing);
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
        let http_ns = unwrap_namespace(block_on(engine.load("http://example.com/doc.texl")));
        assert_eq!(http_ns.get("source"), Value::Text("HTTP source for http://example.com/doc.texl".into()));

        // Scheme-less URL routed to the default loader
        let file_ns = unwrap_namespace(block_on(engine.load("/local/file.texl")));
        assert_eq!(file_ns.get("source"), Value::Text("Default source for /local/file.texl".into()));
    }

    #[test]
    fn test_engine_fetch_returns_the_source_or_none() {
        let (engine, requested) = engine_with_docs(&[("/a.texl", "A"), ("http://example.com/b.texl", "B")]);

        assert_eq!(block_on(engine.fetch("a.texl#part")), Some("A".to_string()));
        assert_eq!(block_on(engine.fetch("http://example.com/b.texl")), Some("B".to_string()));
        assert_eq!(block_on(engine.fetch("/missing.texl")), None);
        assert_eq!(block_on(engine.fetch("ftp://example.com/c.texl")), None);
        assert_eq!(*requested.borrow(), vec!["/a.texl", "http://example.com/b.texl", "/missing.texl"]);
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
        assert_eq!(ns.get("text"), Value::Text("Result is 50".into()));

        // Ensure the document data contains the assignments
        let doc_data = unwrap_namespace(ns.get("data"));
        assert_eq!(doc_data.get("result"), Value::Numb(50.0));
    }

    #[test]
    fn test_engine_load_is_fetch_and_render() {
        let main = format!(r#"<% x = 2 %>main <% texl.import("lib.texl") {RENDERING} %>"#);
        let (engine, _) = engine_with_docs(&[("/a/main.texl", &main), ("/a/lib.texl", "lib")]);

        let loaded = unwrap_namespace(block_on(engine.load("/a/main.texl")));
        let source = block_on(engine.fetch("/a/main.texl")).unwrap();
        let rendered = unwrap_namespace(block_on(engine.render("/a/main.texl", &source)));

        for field in ["uri", "source", "text"] {
            assert_eq!(loaded.get(field), rendered.get(field));
        }
        assert_eq!(rendered.get("text"), Value::Text("main lib".into()));
        assert_eq!(unwrap_namespace(rendered.get("data")).get("x"), Value::Numb(2.0));
    }

    #[test]
    fn test_engine_load_of_a_missing_document_returns_nothing() {
        let (engine, requested) = engine_with_docs(&[]);

        assert_eq!(block_on(engine.load("/missing.texl")), Value::Nothing);
        // Missing loader: no loader is called at all
        assert_eq!(block_on(engine.load("ftp://example.com/file.texl")), Value::Nothing);
        assert_eq!(*requested.borrow(), vec!["/missing.texl"]);
    }

    #[test]
    fn test_engine_sees_changed_sources_in_the_next_render() {
        let source = Rc::new(RefCell::new("first".to_string()));
        let engine = Engine::new(None);
        let s = source.clone();
        engine.add_default_loader(loader_fn(move |_| {
            let text = s.borrow().clone();
            async move { Ok(text) }
        }));
        let main = format!(r#"<% texl.import("lib.texl") {RENDERING} %>"#);

        assert_eq!(rendering(block_on(engine.render("/main.texl", &main))), "first");
        *source.borrow_mut() = "second".to_string();
        assert_eq!(rendering(block_on(engine.render("/main.texl", &main))), "second");
        assert_eq!(rendering(block_on(engine.load("/lib.texl"))), "second");
    }

    #[test]
    fn test_engine_deduplicates_imports_within_a_rendering() {
        let main = format!(r#"Main loads <% texl.import("/shared.texl") {RENDERING} %> twice: <% texl.import("/shared.texl") {RENDERING} %>"#);
        let (engine, requested) = engine_with_docs(&[("/main.texl", &main), ("/shared.texl", "Shared!")]);

        let doc = block_on(engine.load("/main.texl"));

        assert_eq!(rendering(doc), "Main loads Shared! twice: Shared!");
        assert_eq!(*requested.borrow(), vec!["/main.texl", "/shared.texl"]);
    }

    #[test]
    fn test_engine_diamond_imports_share_one_rendering() {
        // A starts B and C at once; both import D while it is pending
        let a = r#"<% pb = texl.import("b.texl"), pc = texl.import("c.texl") %><% pb ~> (doc -> bind "b" doc) %><% pc ~> (doc -> bind "c" doc) %>"#;
        let bc = r#"<% texl.import("d.texl") ~> (doc -> bind "d" doc) %>"#;
        let (engine, requested) = engine_with_docs(&[("/b.texl", bc), ("/c.texl", bc), ("/d.texl", "D")]);

        let a_data = data(block_on(engine.render("/a.texl", a)));
        let d_of = |name: &str| unwrap_namespace(data(a_data.get(name)).get("d"));

        assert!(d_of("b").ptr_eq(&d_of("c")));
        assert_eq!(d_of("b").get("text"), Value::Text("D".into()));
        assert_eq!(requested.borrow().iter().filter(|url| *url == "/d.texl").count(), 1);
    }

    #[test]
    fn test_engine_self_import_of_the_root_is_undefined() {
        let (engine, requested) = engine_with_docs(&[]);

        let doc = block_on(engine.render("/a/main.texl", r#"<% x = texl.import("../a/main.texl#top") %>done"#));

        assert!(matches!(data(doc.clone()).get("x"), Value::Undefined(_)));
        assert_eq!(rendering(doc), "done");
        assert!(requested.borrow().is_empty());
    }

    #[test]
    fn test_engine_import_cycle_is_undefined() {
        let a = r#"<% texl.import("b.texl") ~> (doc -> bind "b" doc) %>"#;
        let b = r#"<% x = texl.import("a.texl") %>b"#;
        let (engine, _) = engine_with_docs(&[("/a.texl", a), ("/b.texl", b)]);

        let b_doc = data(block_on(engine.load("/a.texl"))).get("b");

        assert_eq!(rendering(b_doc.clone()), "b");
        assert!(matches!(data(b_doc).get("x"), Value::Undefined(_)));
    }

    #[test]
    fn test_engine_concurrent_import_cycle_completes() {
        // A starts B and C at once, then B imports C and C imports B
        let a = r#"<% pb = texl.import("b.texl"), pc = texl.import("c.texl") %><% pb ~> (doc -> bind "b" doc) %><% pc ~> (doc -> bind "c" doc) %>"#;
        let b = r#"<% x = texl.import("c.texl") %><% x %>"#;
        let c = r#"<% x = texl.import("b.texl") %><% x %>"#;
        let (engine, _) = engine_with_docs(&[("/b.texl", b), ("/c.texl", c)]);

        let a_data = data(block_on(engine.render("/a.texl", a)));
        let mut renderings = vec![rendering(a_data.get("b")), rendering(a_data.get("c"))];
        renderings.sort();

        assert_eq!(renderings, vec!["[[Namespace of 5]]", "[[Undefined]]"]);
    }

    #[test]
    fn test_engine_imported_documents_cannot_read_their_importer() {
        let main = format!(r#"<% secret = 1 %>[<% texl.import("lib.texl") {RENDERING} %>][<% leak %>]"#);
        let lib = r#"<% secret %><% bind "leak" 2 %>"#;
        let (engine, _) = engine_with_docs(&[("/main.texl", &main), ("/lib.texl", lib)]);

        assert_eq!(rendering(block_on(engine.load("/main.texl"))), "[][]");
    }

    #[test]
    fn test_engine_bind_is_global_to_the_document() {
        let engine = Engine::new(None);
        engine.add_builtin("x", Value::Numb(0.0));

        let doc = block_on(engine.render("/a.texl", r#"<% (() -> bind "x" 1)() %><% x %>"#));
        assert_eq!(rendering(doc.clone()), "1");
        // Bound in the scope of the document, so it is in `data`
        assert_eq!(data(doc).get("x"), Value::Numb(1.0));

        // Names bound by one rendering are not seen by the next
        let doc = block_on(engine.render("/b.texl", "<% x %>"));
        assert_eq!(rendering(doc), "0");
    }

    #[test]
    fn test_engine_renderings_are_freed() {
        let main = r#"<% x = 1, ns = {y: x} %><% texl.import("lib.texl") ~> (doc -> bind "lib" doc) %><% texl.import("other.texl") %>"#;
        let (engine, _) = engine_with_docs(&[("/lib.texl", "<% y = 2, point = {x: 1} %>lib"), ("/other.texl", "other")]);
        engine.add_builtin("global", Value::Text("kept".into()));

        // The first scope of the thread also creates the built-ins, which are never freed
        drop(engine.create_scope());
        let before = Scope::live_count();

        let doc = block_on(engine.render("/main.texl", main));
        assert_eq!(rendering(data(doc.clone()).get("lib")), "lib");
        drop(doc);
        run_pending();

        // Documents that do not store functions hold no scope: data is a plain namespace
        assert_eq!(Scope::live_count(), before);
    }

    #[test]
    fn test_engine_render_collects_documents_that_define_functions() {
        let main = r#"<% fact = n -> n <= 1 ? 1 ; n * fact (n - 1) %><% texl.import("lib.texl") ~> (doc -> bind "lib" doc) %><% lib.data.twice 2 %> <% fact 4 %>"#;
        let (engine, _) = engine_with_docs(&[("/lib.texl", "<% twice = x -> x * 2 %>")]);
        drop(engine.create_scope());
        let before = Scope::live_count();

        // The functions capture the scopes of their documents: reference cycles
        let doc = block_on(engine.render("/main.texl", main));
        assert_eq!(rendering(doc.clone()), "4 24");
        let twice = data(data(doc.clone()).get("lib")).get("twice");
        drop(doc);
        run_pending();
        assert!(Scope::live_count() > before);

        // The next rendering collects them, except the library held through `twice`
        drop(block_on(engine.render("/other.texl", "other")));
        assert!(Scope::live_count() > before);
        assert_eq!(swan_core::evaluator::eval_apply(twice.clone(), Value::Numb(5.0), 0), Value::Numb(10.0));
        drop(twice);
        drop(block_on(engine.render("/other.texl", "other")));
        assert_eq!(Scope::live_count(), before);
    }

    #[test]
    fn test_engine_data_is_a_snapshot() {
        let engine = Engine::new(None);
        let doc = block_on(engine.render("/a.texl", r#"<% x = 1, later = () -> bind "y" 2 %>"#));
        let data = data(doc);

        // Binding a name after the rendering does not change `data`
        swan_core::evaluator::eval_apply(data.get("later"), Value::Nothing, 0);
        assert_eq!(data.get("y"), Value::Nothing);
        assert_eq!(data.get("x"), Value::Numb(1.0));
    }

    #[test]
    fn test_engine_import_after_the_rendering_starts_a_new_cache() {
        let main = r#"<% get = () -> texl.import("lib.texl") %>"#;
        let (engine, requested) = engine_with_docs(&[("/lib.texl", "lib")]);

        let doc = block_on(engine.render("/main.texl", main));
        let get = data(doc).get("get");
        let call = || block_on(async {
            let Value::Promise(p) = swan_core::evaluator::eval_apply(get.clone(), Value::Nothing, 0) else { panic!() };
            crate::template::render("<% p %>", &{ let s = Scope::new(); s.set("p", Value::Promise(p)); s }).await.text
        });

        assert_eq!(call(), "[[Namespace of 5]]");
        assert_eq!(call(), "[[Namespace of 5]]");
        assert_eq!(*requested.borrow(), vec!["/lib.texl", "/lib.texl"]);
    }

    #[test]
    fn test_engine_fragments_are_one_document_queries_are_not() {
        let main = format!(
            r#"<% texl.import("x.texl#a") {RENDERING} %><% texl.import("x.texl#b") {RENDERING} %><% texl.import("y.texl?b=1&a=2") {RENDERING} %><% texl.import("y.texl?a=2&b=1") {RENDERING} %>"#
        );
        let (engine, requested) = engine_with_docs(&[
            ("/x.texl", "x"),
            ("/y.texl?b=1&a=2", "y1"),
            ("/y.texl?a=2&b=1", "y2"),
        ]);

        assert_eq!(rendering(block_on(engine.render("/main.texl", &main))), "xxy1y2");
        assert_eq!(*requested.borrow(), vec!["/x.texl", "/y.texl?b=1&a=2", "/y.texl?a=2&b=1"]);
    }

    #[test]
    fn test_engine_concurrent_renders_do_not_interfere() {
        let (engine, _) = engine_with_docs(&[("/a/lib.texl", "A"), ("/b/lib.texl", "B")]);
        let source = format!(r#"<% texl.import("lib.texl") {RENDERING} %>"#);

        let render = |url: &'static str| {
            let engine = engine.clone();
            let source = source.clone();
            promise_async(None, async move { engine.render(url, &source).await })
        };
        let (a, b) = (render("/a/main.texl"), render("/b/main.texl"));
        run_pending();

        let result = |p: Value| match p {
            Value::Promise(p) => p.0.borrow().value.clone().unwrap(),
            other => panic!("Expected Promise, got {:?}", other),
        };
        assert_eq!(rendering(result(a)), "A");
        assert_eq!(rendering(result(b)), "B");
    }

    #[test]
    fn test_engine_bare_scope_cannot_import() {
        let engine = Engine::new(None);
        let (one, two) = (engine.create_scope(), engine.create_scope());

        let texl = unwrap_namespace(one.get("texl"));
        assert_eq!(texl.get("import"), Value::Nothing);
        // Each scope has its own `texl` namespace
        assert!(!texl.ptr_eq(&unwrap_namespace(two.get("texl"))));
    }

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

        let doc = block_on(engine.load("docs/main.texl"));

        assert_eq!(rendering(doc), "[main [header [footer note]]]");
        assert_eq!(
            *requested.borrow(),
            vec!["/docs/main.texl", "/docs/parts/header.texl", "/docs/footer.texl", "/docs/parts/note.texl"]
        );
    }

    #[test]
    fn test_engine_absolute_paths_and_uris_ignore_importing_document() {
        let main = format!(
            r#"<% texl.import("/top.texl") {RENDERING} %>,<% texl.import("http://example.com/x/remote.texl") {RENDERING} %>"#
        );
        let (engine, requested) = engine_with_docs(&[
            ("/top.texl", "top"),
            ("http://example.com/x/remote.texl", "remote"),
        ]);

        let doc = block_on(engine.render("/docs/main.texl", &main));

        assert_eq!(rendering(doc), "top,remote");
        assert_eq!(*requested.borrow(), vec!["/top.texl", "http://example.com/x/remote.texl"]);
    }

    #[test]
    fn test_engine_resolves_relative_imports_against_uri_documents() {
        let main = format!(r#"<% texl.import("../lib/util.texl?v=1#x") {RENDERING} %>"#);
        let (engine, requested) = engine_with_docs(&[
            ("http://example.com/site/pages/main.texl", &main),
            ("http://example.com/site/lib/util.texl?v=1", "util"),
        ]);

        let doc = block_on(engine.load("http://example.com/site/pages/main.texl"));

        assert_eq!(rendering(doc), "util");
        assert_eq!(
            *requested.borrow(),
            vec!["http://example.com/site/pages/main.texl", "http://example.com/site/lib/util.texl?v=1"]
        );
    }

    #[test]
    fn test_engine_resolves_absolute_paths_against_the_root_of_uri_documents() {
        let main = format!(r#"<% texl.import("/lib/util.texl") {RENDERING} %>"#);
        let (engine, requested) = engine_with_docs(&[("http://example.com/lib/util.texl", "util")]);

        let doc = block_on(engine.render("http://example.com/site/pages/main.texl", &main));

        assert_eq!(rendering(doc), "util");
        assert_eq!(*requested.borrow(), vec!["http://example.com/lib/util.texl"]);
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

        let doc = block_on(engine.render("/main.texl", &format!(
            r#"<% texl.import("/a/main.texl") {RENDERING} %><% texl.import("/b/main.texl") {RENDERING} %><% texl.import("/c/main.texl") {RENDERING} %>"#
        )));

        assert_eq!(rendering(doc), "ABA");
        assert_eq!(
            *requested.borrow(),
            vec!["/a/main.texl", "/a/common.texl", "/b/main.texl", "/b/common.texl", "/c/main.texl"]
        );
    }

    #[test]
    fn test_engine_failed_imports_are_not_cached() {
        let main = format!(r#"[<% texl.import("missing.texl") {RENDERING} %>][<% texl.import("missing.texl") {RENDERING} %>]"#);
        let (engine, requested) = engine_with_docs(&[]);

        assert_eq!(rendering(block_on(engine.render("/main.texl", &main))), "[][]");
        assert_eq!(*requested.borrow(), vec!["/missing.texl", "/missing.texl"]);
    }

    // --- Tags and undefined values ---

    /// The `tags` field of a document namespace, as namespaces
    fn tags(doc: &Value) -> Vec<Namespace> {
        let Value::Namespace(ns) = doc else { panic!() };
        let Value::List(items) = ns.get("tags") else { panic!("no tags") };
        items.iter().map(|t| unwrap_namespace(t.clone())).collect()
    }

    fn numb(ns: &Namespace, name: &str) -> usize {
        match ns.get(name) { Value::Numb(n) => n as usize, other => panic!("{name}: {other:?}") }
    }

    #[test]
    fn test_engine_tags_spans() {
        let source = "a<% x = 1 %>b<% x + 1 %>c<%%>";
        let engine = Engine::new(None);
        let doc = block_on(engine.render("/main.texl", source));
        assert_eq!(rendering(doc.clone()), "ab2c");
        let tags = tags(&doc);
        assert_eq!(tags.len(), 3);
        let spans: Vec<_> = tags.iter()
            .map(|t| (numb(t, "start"), numb(t, "end"), numb(t, "out_start"), numb(t, "out_end")))
            .collect();
        assert_eq!(spans, vec![(1, 12, 1, 1), (13, 24, 2, 3), (25, 29, 4, 4)]);
        assert_eq!(&source[1..12], "<% x = 1 %>");
        assert_eq!(tags[0].get("value"), Value::Nothing);
        assert_eq!(tags[1].get("value"), Value::Numb(2.0));
    }

    #[test]
    fn test_engine_tag_rendering_nothing_keeps_its_undefined_value() {
        let source = r#"<% y = "a" - 1 %>x"#;
        let engine = Engine::new(None);
        let doc = block_on(engine.render("/main.texl", source));
        assert_eq!(rendering(doc.clone()), "x");
        // The assignment renders nothing, but `y` holds the undefined value
        let Value::Undefined(u) = data(doc.clone()).get("y") else { panic!() };
        assert_eq!(u.pos(), Some(9)); // in the expression
        let loc = document::locate(&u).unwrap();
        assert_eq!(loc.pos, 11);
        assert_eq!(&source[11..12], "-");
        assert_eq!(loc.uri.as_deref(), Some("/main.texl"));
        assert_eq!(&*loc.text, source);
        assert_eq!(loc.tag_start, Some(0));
        // A tag whose value is undefined renders `[[Undefined]]` and keeps the value
        let doc = block_on(engine.render("/main.texl", r#"<% (1 %>"#));
        assert_eq!(rendering(doc.clone()), "[[Undefined]]");
        let Value::Undefined(u) = tags(&doc)[0].get("value") else { panic!() };
        assert_eq!(u.describe(), "missing closing `)`");
    }

    #[test]
    fn test_engine_positions_after_non_ascii_text() {
        let source = r#"µ° è <% 2 * ("a" - 1) %>"#;
        let engine = Engine::new(None);
        let doc = block_on(engine.render("/main.texl", source));
        let tags = tags(&doc);
        assert_eq!(numb(&tags[0], "start"), source.find("<%").unwrap());
        let Value::Undefined(u) = tags[0].get("value") else { panic!() };
        let loc = document::locate(u.cause().unwrap()).unwrap();
        assert_eq!(&source[loc.pos..loc.pos + 1], "-");
        assert_eq!(document::char_offset(&loc.text, loc.pos), source.chars().position(|c| c == '-').unwrap());
    }

    #[test]
    fn test_engine_values_are_located_where_they_were_made() {
        let engine = Engine::new(None);
        // Made in one tag, shown by another: located in the first
        let source = r#"<% x = "a" - 1 %>, <% x %>"#;
        let doc = block_on(engine.render("/main.texl", source));
        let Value::Undefined(u) = tags(&doc)[1].get("value") else { panic!() };
        let loc = document::locate(&u).unwrap();
        assert_eq!((loc.tag_start, loc.pos), (Some(0), 11));
        assert_eq!(&source[11..12], "-");
        // Code run by `parse` is located in its own text, with no document
        let doc = block_on(engine.render("/main.texl", r#"<% (parse "1 + TRUE") {} %>"#));
        let Value::Undefined(u) = tags(&doc)[0].get("value") else { panic!() };
        let loc = document::locate(&u).unwrap();
        assert_eq!((loc.uri, &*loc.text, loc.pos, loc.tag_start), (None, "1 + TRUE", 2, None));
    }

    #[test]
    fn test_engine_undef_inspect_gives_document_positions() {
        let engine = Engine::new(None);
        let source = r#"ab <% i = undef.inspect(2 * ("a" - 1)), i.pos, i.uri, i.rhv.pos %>"#;
        let doc = block_on(engine.render("/main.texl", source));
        let star = source.find('*').unwrap();
        let minus = source.find('-').unwrap();
        assert_eq!(rendering(doc), format!("ab {star}, /main.texl, {minus}"));
        // `undef.from` is still there, and `origin` is not shown
        let doc = block_on(engine.render("/main.texl", r#"<% i = undef.inspect(-"a"), i.origin, undef.from 1 %>"#));
        assert_eq!(rendering(doc), "[[Undefined]]");
        let doc = block_on(engine.render("/main.texl", r#"<% (undef.inspect(undef.from 1)).val %>"#));
        assert_eq!(rendering(doc), "1");
    }

    #[test]
    fn test_engine_promise_tag_value_is_resolved() {
        let (engine, _) = engine_with_docs(&[("/lib.texl", "<% z = 1 %>lib")]);
        let doc = block_on(engine.render("/main.texl", r#"<% texl.import "lib.texl" %>"#));
        let Value::Namespace(lib) = tags(&doc)[0].get("value") else { panic!("not resolved") };
        assert_eq!(lib.get("text"), Value::Text("lib".into()));
    }

    #[test]
    fn test_engine_imported_documents_have_tags_and_their_own_source() {
        let lib = r#"è <% f = x -> x - 1 %><% "b" * 2 %>"#;
        let (engine, _) = engine_with_docs(&[("/lib.texl", lib)]);
        let main = r#"<% texl.import "lib.texl" ~> (l -> (bind "f" (l.data.f), l)) %><% f "z" %>"#;
        let doc = block_on(engine.render("/main.texl", main));
        let lib_doc = tags(&doc)[0].get("value");
        let lib_tags = tags(&lib_doc);
        assert_eq!(lib_tags.len(), 2);
        assert_eq!(numb(&lib_tags[1], "start"), lib.rfind("<%").unwrap());
        let Value::Undefined(u) = lib_tags[1].get("value") else { panic!() };
        assert_eq!(document::locate(&u).unwrap().uri.as_deref(), Some("/lib.texl"));
        // An error inside a function of the library points into the library
        let v = tags(&doc)[1].get("value"); let Value::Undefined(u) = v else { panic!("{v:?}") };
        let loc = document::locate(&u).unwrap();
        assert_eq!(loc.uri.as_deref(), Some("/lib.texl"));
        assert_eq!(&lib[loc.pos..loc.pos + 1], "-");
        assert_eq!(loc.tag_start, Some(lib.find("<%").unwrap()));
    }

    #[test]
    fn test_engine_builtin_errors_point_to_the_call() {
        // In the tag that calls the built-in
        let source = "x <% n = 1, text.size n %>";
        let engine = Engine::new(None);
        let doc = block_on(engine.render("/main.texl", source));
        let Value::Undefined(u) = tags(&doc)[0].get("value") else { panic!() };
        let loc = document::locate(&u).unwrap();
        assert_eq!(loc.uri.as_deref(), Some("/main.texl"));
        assert_eq!(&source[loc.pos..loc.pos + 1], "n"); // the argument
        assert_eq!(loc.pos, source.rfind('n').unwrap());

        // Inside a function imported from a library: in the library
        let lib = r#"<% f = x -> math.sqrt x %>"#;
        let (engine, _) = engine_with_docs(&[("/lib.texl", lib)]);
        let main = r#"<% texl.import "lib.texl" ~> (l -> (bind "f" (l.data.f), l)) %><% f (-1) %>"#;
        let doc = block_on(engine.render("/main.texl", main));
        let v = tags(&doc)[1].get("value"); let Value::Undefined(u) = v else { panic!("{v:?}") };
        assert_eq!(u.describe(), "`math.sqrt` is not defined for the number `-1`");
        let loc = document::locate(&u).unwrap();
        assert_eq!(loc.uri.as_deref(), Some("/lib.texl"));
        assert_eq!(loc.pos, lib.rfind('x').unwrap());

        // `texl.import` errors point to the import
        let source = r#"<% texl.import 5 %>"#;
        let doc = block_on(Engine::new(None).render("/main.texl", source));
        let Value::Undefined(u) = tags(&doc)[0].get("value") else { panic!() };
        assert_eq!(document::locate(&u).unwrap().pos, source.find('5').unwrap());
    }
}
