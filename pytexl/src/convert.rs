//! Conversions between Python values and Swan values.
//!
//! | Python                                    | Swan                                        |
//! |-------------------------------------------|---------------------------------------------|
//! | `None`                                    | `Nothing`                                   |
//! | `bool`                                    | `Bool`                                      |
//! | `int`, `float`                            | `Numb`                                      |
//! | `str`                                     | `Text`                                      |
//! | `list`                                    | `List`                                      |
//! | `tuple`                                   | `Tuple`                                     |
//! | mapping (`dict`, ...)                     | `Namespace` of its string keys              |
//! | awaitable (coroutine, future, task)       | `Promise` (an exception gives `Undefined`)  |
//! | callable                                  | `Func` (an exception gives `Undefined`)     |
//! | [`Promise`], [`Function`], [`Undefined`]  | the Swan value they wrap                    |
//! | anything else                             | `Nothing`                                   |
//!
//! In the other direction, `Nothing` becomes `None`, integral numbers become `int` and the
//! others `float`, lists become `list`, tuples become `tuple`, namespaces become `dict` (own
//! bindings only), promises become [`Promise`], functions become [`Function`] and undefined
//! values become [`Undefined`], which keeps the details of the error. Positions are converted
//! to code points (Python string indices): the `pos` of an [`Undefined`], and the tag spans of
//! a document namespace (see [`texl_core::engine::convert_tag_positions`]). A mapping with the
//! members of a document gets its tag spans converted back to bytes, so a document passed
//! back to Swan keeps its positions.


use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyBool, PyDict, PyFloat, PyInt, PyList, PyString, PyTuple};
use swan_core::evaluator::apply;
use pyo3::basic::CompareOp;
use texl_core::document::{char_offset, char_to_byte_offset, locate, members};
use swan_core::value::{BuiltinFn, Namespace, Undef, Value};

use crate::runtime::{self, AwaitPy};

/// Integers up to this magnitude are exact as `f64`, and come back to Python as `int`.
const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

/// A Swan promise. Await it to get its value, converted to a Python value.
///
/// Promises never fail: a promise of an operation that fails resolves to an
/// `Undefined`, and one whose work cannot run (a coroutine that raises, a
/// failed load) to `None`. Await a promise inside a running asyncio event
/// loop.
///
/// Example:
///     >>> context = pytexl.Engine().create_context()
///     >>> await context.render("<% p = promise.create() %>")
///     ''
///     >>> context.get("p")
///     <pytexl.Promise pending>
#[pyclass(unsendable, module = "pytexl", name = "Promise")]
pub struct Promise {
    pub inner: swan_core::value::Promise,
}

#[pymethods]
impl Promise {
    fn __await__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let future = runtime::asyncio(py)?.call_method0("get_running_loop")?.call_method0("create_future")?;
        let target = future.clone().unbind();
        self.inner.then(move |value| {
            Python::attach(|py| {
                let future = target.bind(py);
                if !future.call_method0("done").and_then(|d| d.is_truthy()).unwrap_or(true) {
                    let _ = future.call_method1("set_result", (swan_to_py(py, &value),));
                }
            });
            Value::Nothing
        });
        runtime::kick();
        future.call_method0("__await__")
    }

    /// bool: True once the promise has a value.
    #[getter]
    fn done(&self) -> bool {
        self.inner.0.borrow().value.is_some()
    }

    fn __repr__(&self) -> &'static str {
        if self.done() { "<pytexl.Promise resolved>" } else { "<pytexl.Promise pending>" }
    }
}

/// A Swan function. Call it like a Python function.
///
/// The arguments are converted to Swan values: no argument is passed as
/// `Nothing`, one argument as itself, and several arguments as a tuple. The
/// result is converted back to a Python value. Calling a Swan function never
/// raises: an invalid call returns an :class:`Undefined`.
///
/// Example:
///     >>> doc = await pytexl.Engine().load("lib/format.texl")
///     >>> doc.data["plural"](3, "apple")
///     '3 apples'
#[pyclass(unsendable, module = "pytexl", name = "Function")]
pub struct Function {
    pub inner: Value,
}

#[pymethods]
impl Function {
    #[pyo3(signature = (*args))]
    fn __call__(&self, py: Python<'_>, args: &Bound<'_, PyTuple>) -> Py<PyAny> {
        let arg = py_args_to_swan(args);
        swan_to_py(py, &apply(self.inner.clone(), arg))
    }

    fn __repr__(&self) -> String {
        match &self.inner {
            Value::Builtin(f) => format!("<pytexl.Function {}>", f.name),
            _ => "<pytexl.Function>".to_string(),
        }
    }
}

/// A Swan undefined value: the result of an invalid operation, such as `"a" - 1`.
///
/// Swan does not raise exceptions: an operation that fails produces an
/// undefined value, which records what went wrong. Its attributes are the
/// members of `undef.inspect` in Swan. `description` explains the error in
/// plain words, `type` names its kind, and the other attributes depend on it;
/// those that do not apply are `None`:
///
/// ===================  ================================  ==========================================
/// `type`               Attributes                        Meaning
/// ===================  ================================  ==========================================
/// `"UnaryOperation"`   `op`, `val`, `pos`                operation `op` failed on the operand `val`
/// `"BinaryOperation"`  `op`, `lhv`, `rhv`, `pos`         operation `op` failed on `lhv` and `rhv`
/// `"Termination"`      `expected`, `pos`                 the closing bracket `expected` is missing
/// `"Token"`            `token`, `pos`                    a token is in the wrong place
/// `"Name"`             `pos`                             a name was expected
/// `"Value"`            `val`                             made on purpose, by `undef.from`
/// `"HostError"`        `error`, `pos`                    a host function or awaitable raised `error`
/// ===================  ================================  ==========================================
///
/// Operands are converted to Python values; an operand that is itself
/// undefined is an `Undefined`, and `cause` is the first of them, so
/// following `cause` leads to the root cause. `pos` is the position of the
/// error in the source of the code that made it, in code points (a Python
/// string index); `source` is that source, so `source[pos]` is where the
/// error is, and `uri` is the URL of that document. An error of a built-in
/// or host function is at the call that returned it. All three are `None`
/// for values made on purpose (`undef.from`), and `uri` is `None` for a
/// template rendered in a context. An `Undefined` is falsy, and passing it
/// back to Swan gives the original undefined value.
///
/// Attributes:
///     type (str): The kind of error.
///     description (str): The error in plain words.
///     op (str | None): The name of the failed operation.
///     val (Any): The operand of a unary operation, or the value of `undef.from`.
///     lhv (Any): The left operand of a binary operation.
///     rhv (Any): The right operand of a binary operation.
///     pos (int | None): The position of the error in the source, in code points.
///     source (str | None): The text `pos` is an index of: the whole document, or the code run by `parse`.
///     uri (str | None): The URL of the document the error is in.
///     expected (str | None): The missing closing bracket.
///     token (str | None): The misplaced token.
///     error (str | None): The exception of a host function or awaitable, such as ``"ValueError: boom"``.
///     cause (Undefined | None): The first operand that is itself undefined.
///
/// Example:
///     >>> context = pytexl.Engine().create_context()
///     >>> await context.render('<% err = "hello" - 1 %>')
///     Rendering(text='', data={...}, tags=[...])
///     >>> err = context.get("err")
///     >>> err
///     <pytexl.Undefined BinaryOperation pos=17: cannot subtract the number `1` from the text `"hello"`>
///     >>> err.op, err.lhv, err.rhv
///     ('subtraction', 'hello', 1)
#[pyclass(unsendable, module = "pytexl", name = "Undefined")]
pub struct Undefined {
    pub inner: Undef,
}

/// The position of `undef` in its document (or expression), in code points,
/// if it has one.
fn char_pos(undef: &Undef) -> Option<usize> {
    locate(undef).map(|loc| char_offset(&loc.text, loc.pos))
}

impl Undefined {
    /// The members of `undef.inspect`, converted to Python values, with `pos` in code points.
    fn members(&self, py: Python<'_>) -> Vec<(&'static str, Py<PyAny>)> {
        members(&self.inner).into_iter().map(|(name, value)| {
            // A position that cannot be located in a document or expression is `None`
            let value = match name {
                "pos" => char_pos(&self.inner).into_pyobject(py).unwrap().into_any().unbind(),
                _ => swan_to_py(py, &value),
            };
            (name, value)
        }).collect()
    }

    /// The member `name`, or `None` if this kind of error has no such member.
    fn member(&self, py: Python<'_>, name: &str) -> Py<PyAny> {
        self.members(py).into_iter().find(|(n, _)| *n == name).map(|(_, v)| v).unwrap_or_else(|| py.None())
    }
}

#[pymethods]
impl Undefined {
    /// str: The kind of error, such as ``"BinaryOperation"``.
    #[getter(r#type)]
    fn kind(&self) -> &'static str {
        self.inner.kind()
    }

    /// str: The error in plain words, such as ``"cannot add the text `"a"` and the number `1`"``.
    #[getter]
    fn description(&self) -> String {
        self.inner.describe()
    }

    /// str | None: The name of the failed operation, such as ``"subtraction"``.
    #[getter]
    fn op(&self, py: Python<'_>) -> Py<PyAny> {
        self.member(py, "op")
    }

    /// Any: The operand of a failed unary operation, or the value wrapped by ``undef.from``.
    #[getter]
    fn val(&self, py: Python<'_>) -> Py<PyAny> {
        self.member(py, "val")
    }

    /// Any: The left operand of a failed binary operation.
    #[getter]
    fn lhv(&self, py: Python<'_>) -> Py<PyAny> {
        self.member(py, "lhv")
    }

    /// Any: The right operand of a failed binary operation.
    #[getter]
    fn rhv(&self, py: Python<'_>) -> Py<PyAny> {
        self.member(py, "rhv")
    }

    /// int | None: The position of the error in the source, in code points.
    #[getter]
    fn pos(&self) -> Option<usize> {
        char_pos(&self.inner)
    }

    /// str | None: The text ``pos`` is an index of: the whole document, or the code run by ``parse``.
    #[getter]
    fn source(&self) -> Option<String> {
        locate(&self.inner).map(|loc| loc.text.to_string())
    }

    /// str | None: The URL of the document the error is in.
    #[getter]
    fn uri(&self) -> Option<String> {
        locate(&self.inner).and_then(|loc| loc.uri).map(|u| u.to_string())
    }

    /// str | None: The missing closing bracket of a ``"Termination"`` error.
    #[getter]
    fn expected(&self, py: Python<'_>) -> Py<PyAny> {
        self.member(py, "expected")
    }

    /// str | None: The misplaced token of a ``"Token"`` error.
    #[getter]
    fn token(&self, py: Python<'_>) -> Py<PyAny> {
        self.member(py, "token")
    }

    /// str | None: The exception of a host function or awaitable, such as ``"ValueError: boom"``.
    #[getter]
    fn error(&self, py: Python<'_>) -> Py<PyAny> {
        self.member(py, "error")
    }

    /// Undefined | None: The first operand that is itself undefined, the cause of this error.
    #[getter]
    fn cause(&self) -> Option<Undefined> {
        self.inner.cause().map(|u| Undefined { inner: u.clone() })
    }

    /// Returns the details of the error as a dict, like ``undef.inspect`` in Swan.
    ///
    /// Returns:
    ///     dict[str, Any]: The ``type`` and ``description`` of the error and the
    ///     members that apply to it; ``pos`` is in code points.
    ///
    /// Example:
    ///     >>> err.inspect()
    ///     {'type': 'BinaryOperation', 'description': 'cannot subtract the number `1` from the text `"hello"`', 'op': 'subtraction', 'lhv': 'hello', 'rhv': 1, 'pos': 17}
    fn inspect<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);
        for (name, value) in self.members(py) {
            dict.set_item(name, value)?;
        }
        Ok(dict)
    }

    fn __bool__(&self) -> bool {
        false
    }

    /// Undefined values are equal when they have the same kind and members, whatever the position.
    fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: CompareOp) -> PyResult<Py<PyAny>> {
        let py = other.py();
        let Ok(other) = other.cast::<Undefined>() else { return Ok(py.NotImplemented()) };
        let equal = self.inner == other.borrow().inner;
        match op {
            CompareOp::Eq => Ok(PyBool::new(py, equal).to_owned().into_any().unbind()),
            CompareOp::Ne => Ok(PyBool::new(py, !equal).to_owned().into_any().unbind()),
            _ => Ok(py.NotImplemented()),
        }
    }

    fn __repr__(&self) -> String {
        let pos = char_pos(&self.inner).map(|p| format!(" pos={p}")).unwrap_or_default();
        format!("<pytexl.Undefined {}{pos}: {}>", self.inner.kind(), self.inner.describe())
    }
}

/// Converts the positional arguments of a Python call to one Swan argument.
fn py_args_to_swan(args: &Bound<'_, PyTuple>) -> Value {
    Value::tuple(args.iter().map(|arg| py_to_swan(&arg)).collect())
}

/// `collections.abc.Mapping`.
fn mapping_type(py: Python<'_>) -> PyResult<&Bound<'_, PyAny>> {
    static MAPPING: PyOnceLock<Py<PyAny>> = PyOnceLock::new();
    MAPPING
        .get_or_try_init(py, || Ok::<_, PyErr>(py.import("collections.abc")?.getattr("Mapping")?.unbind()))
        .map(|m| m.bind(py))
}

/// True for coroutines, futures and any object with `__await__`.
fn is_awaitable(obj: &Bound<'_, PyAny>) -> bool {
    obj.hasattr("__await__").unwrap_or(false)
}

/// Calls `f` with each `(key, value)` pair of a mapping whose key is a string.
pub fn for_each_entry(mapping: &Bound<'_, PyAny>, mut f: impl FnMut(String, Value)) -> PyResult<()> {
    let items = mapping.call_method0("items")?;
    for item in items.try_iter()? {
        let (key, value): (Bound<'_, PyAny>, Bound<'_, PyAny>) = item?.extract()?;
        if let Ok(key) = key.extract::<String>() {
            f(key, py_to_swan(&value));
        }
    }
    Ok(())
}

/// Converts a Python value to a Swan value. See the [module documentation](self).
pub fn py_to_swan(obj: &Bound<'_, PyAny>) -> Value {
    let py = obj.py();
    if obj.is_none() {
        Value::Nothing
    } else if let Ok(p) = obj.cast::<Promise>() {
        Value::Promise(p.borrow().inner.clone())
    } else if let Ok(f) = obj.cast::<Function>() {
        f.borrow().inner.clone()
    } else if let Ok(u) = obj.cast::<Undefined>() {
        Value::Undefined(u.borrow().inner.clone())
    } else if let Ok(b) = obj.cast::<PyBool>() {
        Value::Bool(b.is_true())
    } else if obj.is_instance_of::<PyInt>() || obj.is_instance_of::<PyFloat>() {
        obj.extract::<f64>().map(Value::Numb).unwrap_or(Value::Nothing)
    } else if let Ok(s) = obj.cast::<PyString>() {
        s.to_str().map(|s| Value::Text(s.into())).unwrap_or(Value::Nothing)
    } else if let Ok(list) = obj.cast::<PyList>() {
        Value::list(list.iter().map(|item| py_to_swan(&item)).collect())
    } else if let Ok(tuple) = obj.cast::<PyTuple>() {
        Value::tuple(tuple.iter().map(|item| py_to_swan(&item)).collect())
    } else if obj.is_instance_of::<PyDict>() || mapping_type(py).is_ok_and(|m| obj.is_instance(m).unwrap_or(false)) {
        let mut ns = Namespace::new();
        match for_each_entry(obj, |key, value| ns.insert(key, value)) {
            // A document comes back with its tag positions in bytes
            Ok(()) => Value::Namespace(
                texl_core::engine::convert_tag_positions(&ns, char_to_byte_offset).unwrap_or(ns),
            ),
            Err(_) => Value::Nothing,
        }
    } else if is_awaitable(obj) {
        let awaitable = obj.clone().unbind();
        Value::Promise(runtime::promise_of(async move {
            match AwaitPy::new(awaitable).await {
                Ok(result) => Python::attach(|py| py_to_swan(result.bind(py))),
                Err(error) => Value::undef_host_error(error),
            }
        }))
    } else if obj.is_callable() {
        Value::Builtin(py_function(obj.clone().unbind()))
    } else {
        Value::Nothing
    }
}

/// Wraps a Python callable as a Swan function. See [`Function`] for the argument convention,
/// which is the same in this direction. An exception gives an `Undefined`.
fn py_function(func: Py<PyAny>) -> BuiltinFn {
    BuiltinFn::new("python", move |arg| {
        Python::attach(|py| {
            let args: Vec<Py<PyAny>> = match &arg {
                Value::Nothing => vec![],
                Value::Tuple(items) => items.iter().map(|item| swan_to_py(py, item)).collect(),
                other => vec![swan_to_py(py, other)],
            };
            match PyTuple::new(py, args).and_then(|args| func.bind(py).call1(args)) {
                Ok(result) => py_to_swan(&result),
                Err(err) => Value::undef_host_error(runtime::error_text(&err)),
            }
        })
    })
}

/// Converts a Swan value to a Python value. See the [module documentation](self).
pub fn swan_to_py(py: Python<'_>, value: &Value) -> Py<PyAny> {
    let converted: PyResult<Bound<'_, PyAny>> = match value {
        Value::Nothing | Value::Name(_) => Ok(py.None().into_bound(py)),
        Value::Undefined(undef) => Bound::new(py, Undefined { inner: undef.clone() }).map(Bound::into_any),
        Value::Bool(b) => Ok(PyBool::new(py, *b).to_owned().into_any()),
        Value::Numb(n) if n.fract() == 0.0 && n.abs() <= MAX_SAFE_INTEGER => {
            Ok((*n as i64).into_pyobject(py).map(Bound::into_any).unwrap())
        }
        Value::Numb(n) => Ok(PyFloat::new(py, *n).into_any()),
        Value::Text(s) => Ok(PyString::new(py, s).into_any()),
        Value::List(items) => {
            let items: Vec<_> = items.iter().map(|item| swan_to_py(py, item)).collect();
            PyList::new(py, items).map(Bound::into_any)
        }
        Value::Tuple(items) => {
            let items: Vec<_> = items.iter().map(|item| swan_to_py(py, item)).collect();
            PyTuple::new(py, items).map(Bound::into_any)
        }
        Value::Namespace(ns) => {
            // A document gets its tag positions in code points
            let converted = texl_core::engine::convert_tag_positions(ns, char_offset);
            let ns = converted.as_ref().unwrap_or(ns);
            // Namespaces are immutable values built from their members, so they cannot contain themselves
            let dict = PyDict::new(py);
            let mut result = Ok(());
            for key in ns.names() {
                let item = ns.get(&key);
                result = result.and(dict.set_item(key, swan_to_py(py, &item)));
            }
            result.map(|()| dict.into_any())
        }
        Value::Promise(p) => Bound::new(py, Promise { inner: p.clone() }).map(Bound::into_any),
        Value::Func { .. } | Value::Builtin(_) => {
            Bound::new(py, Function { inner: value.clone() }).map(Bound::into_any)
        }
    };
    converted.map(Bound::unbind).unwrap_or_else(|_| py.None())
}
