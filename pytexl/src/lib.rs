//! Python bindings of the texl engine.
//!
//! This crate builds `pytexl._pytexl`, the native module of the `pytexl` Python
//! package. The package itself (`python/pytexl`) wraps the classes of this module
//! in the documented Python API, and provides the file and HTTP loaders.
//!
//! - [`NativeEngine`] wraps a [`texl_core::Engine`] whose tasks run on the asyncio
//!   loop (see [`runtime`]). Loaders are Python callables.
//! - [`NativeContext`] is a scope of an engine that keeps its bindings across renders.
//! - [`convert`] converts values between Python and Swan; Swan promises and
//!   functions reach Python as [`convert::Promise`] and [`convert::Function`], and
//!   undefined values as [`convert::Undefined`].
//!
//! See the [Python guide and API reference](https://github.com/onlabsorg/texl/blob/main/docs/api/python.md).

mod convert;
mod runtime;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use swan_core::value::{Scope, Value};
use texl_core::helpers::spawner_fn;
use texl_core::{Engine, LoaderFn, loader_fn, template};
use url::Url;

use convert::{Function, Promise, Undefined, for_each_entry, py_to_swan};
use runtime::AwaitPy;

/// The native engine behind `pytexl.Engine`.
///
/// Args:
///     base_url (str): The absolute URL that `fetch`, `load` and `render` resolve
///         relative URIs against.
///
/// Raises:
///     ValueError: If `base_url` is not an absolute URL.
#[pyclass(unsendable, module = "pytexl._pytexl")]
struct NativeEngine {
    engine: Engine,
    base_url: Url,
}

#[pymethods]
impl NativeEngine {
    #[new]
    fn new(base_url: &str) -> PyResult<Self> {
        let base_url =
            Url::parse(base_url).map_err(|e| PyValueError::new_err(format!("invalid base URL {base_url}: {e}")))?;
        let engine = Engine::new(Some(spawner_fn(runtime::spawn)));
        Ok(NativeEngine { engine, base_url })
    }

    /// str: The base URL, normalized.
    #[getter]
    fn base_url(&self) -> String {
        self.base_url.to_string()
    }

    /// Binds a global, converted to a Swan value.
    fn set(&self, name: &str, value: &Bound<'_, PyAny>) {
        self.engine.add_builtin(name, py_to_swan(value));
    }

    /// Registers a Python loader for the URIs whose scheme is `scheme`.
    fn add_loader(&self, scheme: &str, loader: Py<PyAny>) {
        self.engine.add_loader(scheme, py_loader(loader));
    }

    /// Registers the Python loader of the URIs without a scheme.
    fn set_default_loader(&self, loader: Py<PyAny>) {
        self.engine.add_default_loader(py_loader(loader));
    }

    /// Resolves `uri` against the base URL.
    ///
    /// Raises:
    ///     ValueError: If `uri` cannot be resolved to a URL.
    fn resolve(&self, uri: &str) -> PyResult<String> {
        resolve_document_url(uri, &self.base_url).map_err(PyValueError::new_err)
    }

    /// Returns a promise of the source of the document at `uri` (resolved against the base
    /// URL), or of None if it cannot be fetched.
    ///
    /// Raises:
    ///     ValueError: If `uri` cannot be resolved to a URL.
    fn fetch(&self, uri: &str) -> PyResult<Promise> {
        let url = self.resolve(uri)?;
        let engine = self.engine.clone();
        Ok(Promise {
            inner: runtime::promise_of(async move {
                engine.fetch(&url).await.map_or(Value::Nothing, |source| Value::Text(source.into()))
            }),
        })
    }

    /// Returns a promise of the document at `uri` (resolved against the base URL), as a
    /// dict, or of None if it cannot be loaded.
    ///
    /// Raises:
    ///     ValueError: If `uri` cannot be resolved to a URL.
    fn load(&self, uri: &str) -> PyResult<Promise> {
        let url = self.resolve(uri)?;
        let engine = self.engine.clone();
        Ok(Promise { inner: runtime::promise_of(async move { engine.load(&url).await }) })
    }

    /// Returns a promise of `source` rendered as the document at `uri` (resolved against
    /// the base URL), as a dict.
    ///
    /// Raises:
    ///     ValueError: If `uri` cannot be resolved to a URL.
    fn render(&self, uri: &str, source: String) -> PyResult<Promise> {
        let url = self.resolve(uri)?;
        let engine = self.engine.clone();
        Ok(Promise { inner: runtime::promise_of(async move { engine.render(&url, &source).await }) })
    }

    /// Creates a native context, with the bindings of the mapping `vars`. It cannot import.
    #[pyo3(signature = (vars=None))]
    fn create_context(&self, vars: Option<&Bound<'_, PyAny>>) -> PyResult<NativeContext> {
        let scope = self.engine.create_scope();
        if let Some(vars) = vars {
            for_each_entry(vars, |name, value| scope.set(name, value))?;
        }
        Ok(NativeContext { scope })
    }
}

/// The native context behind `pytexl.Context`.
#[pyclass(unsendable, module = "pytexl._pytexl")]
struct NativeContext {
    scope: Scope,
}

#[pymethods]
impl NativeContext {
    /// Returns the value of `name`, looked up in the context, the globals and the built-ins.
    fn get(&self, py: Python<'_>, name: &str) -> Py<PyAny> {
        convert::swan_to_py(py, &self.scope.get(name))
    }

    /// Binds `name` in the context.
    fn set(&self, name: &str, value: &Bound<'_, PyAny>) {
        self.scope.set(name, py_to_swan(value));
    }

    /// Returns the names bound in the context, sorted.
    fn names(&self) -> Vec<String> {
        self.scope.namespace().names()
    }

    /// Returns a promise of the rendering of `template`, in this context.
    fn render(&self, template: String) -> Promise {
        let scope = self.scope.clone();
        Promise { inner: runtime::promise_of(async move { Value::Text(template::render(&template, &scope).await.into()) }) }
    }
}

/// Wraps a Python loader: a callable that takes the URI and returns the source, or an
/// awaitable of it. An exception, or a result that is not a string, is a failed load.
fn py_loader(loader: Py<PyAny>) -> LoaderFn {
    let loader = std::rc::Rc::new(loader);
    loader_fn(move |uri| {
        let called = Python::attach(|py| loader.bind(py).call1((uri.as_str(),)).map(Bound::unbind));
        async move {
            let result = called.map_err(|e| format!("{uri}: {e}"))?;
            let is_awaitable = Python::attach(|py| result.bind(py).hasattr("__await__").unwrap_or(false));
            let source = if is_awaitable {
                AwaitPy::new(result).await.ok_or_else(|| format!("{uri}: the loader failed"))?
            } else {
                result
            };
            Python::attach(|py| source.bind(py).extract::<String>())
                .map_err(|_| format!("{uri}: the loader did not return a string"))
        }
    })
}

/// Resolves the URI of a document loaded by the host against `base`.
fn resolve_document_url(uri: &str, base: &Url) -> Result<String, String> {
    base.join(uri).map(String::from).map_err(|e| format!("{uri}: {e}"))
}

/// Frees the Swan scopes and values that only reference cycles keep alive, and
/// returns the number of scopes and promises it cleared. Documented in `pytexl.collect`.
#[pyfunction]
fn collect() -> usize {
    swan_core::gc::collect()
}

/// The native module of the `pytexl` package.
#[pymodule(gil_used = true)]
fn _pytexl(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<NativeEngine>()?;
    m.add_class::<NativeContext>()?;
    m.add_class::<Promise>()?;
    m.add_class::<Function>()?;
    m.add_class::<Undefined>()?;
    m.add_function(wrap_pyfunction!(collect, m)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    #[test]
    fn test_resolve_document_url_resolves_relative_uris_against_the_base() {
        let base = url("file:///home/ada/project/");
        assert_eq!(resolve_document_url("docs/main.texl", &base), Ok("file:///home/ada/project/docs/main.texl".into()));
        assert_eq!(resolve_document_url("/main.texl", &base), Ok("file:///main.texl".into()));
        assert_eq!(resolve_document_url("my docs/a.texl", &base), Ok("file:///home/ada/project/my%20docs/a.texl".into()));
        assert_eq!(
            resolve_document_url("https://example.com/a.texl", &base),
            Ok("https://example.com/a.texl".into())
        );
    }
}
