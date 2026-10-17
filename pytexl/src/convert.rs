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
//! | awaitable (coroutine, future, task)       | `Promise` (an exception gives `Nothing`)    |
//! | callable                                  | `Func`                                      |
//! | [`Promise`], [`Function`], [`Undefined`]  | the Swan value they wrap                    |
//! | anything else                             | `Nothing`                                   |
//!
//! In the other direction, `Nothing` becomes `None`, integral numbers become `int` and the
//! others `float`, lists become `list`, tuples become `tuple`, namespaces become `dict` (own
//! bindings only), promises become [`Promise`], functions become [`Function`] and undefined
//! values become [`Undefined`], which keeps the details of the error.

use std::rc::Rc;

use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyBool, PyDict, PyFloat, PyInt, PyList, PyString, PyTuple};
use swan_core::evaluator::eval_apply;
use pyo3::basic::CompareOp;
use swan_core::value::{BuiltinFn, Scope, Undef, Value};

use crate::runtime::{self, AwaitPy};

/// Integers up to this magnitude are exact as `f64`, and come back to Python as `int`.
const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

/// A Swan promise. Await it to get its value, converted to a Python value.
///
/// Promises never fail: an operation that cannot produce a value resolves to
/// `None`. Await a promise inside a running asyncio event loop.
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
        swan_to_py(py, &eval_apply(self.inner.clone(), arg, 0))
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
/// members of `undef.inspect` in Swan. `type` names the kind of error, and the
/// other attributes depend on it; those that do not apply are `None`:
///
/// ===================  ================================  ==========================================
/// `type`               Attributes                        Meaning
/// ===================  ================================  ==========================================
/// `"UnaryOperation"`   `op`, `val`, `pos`                operation `op` failed on the operand `val`
/// `"BinaryOperation"`  `op`, `lhv`, `rhv`, `pos`         operation `op` failed on `lhv` and `rhv`
/// `"Termination"`      `expected`, `pos`                 the closing bracket `expected` is missing
/// `"Token"`            `token`, `pos`                    a token is in the wrong place
/// `"Name"`             `pos`                             a name was expected
/// `"Value"`            `val`                             created by `undef.from`
/// ===================  ================================  ==========================================
///
/// Operands are converted to Python values; an operand that is itself
/// undefined is an `Undefined`. `pos` is the byte offset of the error in the
/// source expression (0 for built-in functions). An `Undefined` is falsy, and
/// passing it back to Swan gives the original undefined value.
///
/// Attributes:
///     type (str): The kind of error.
///     op (str | None): The name of the failed operation.
///     val (Any): The operand of a unary operation, or the value of `undef.from`.
///     lhv (Any): The left operand of a binary operation.
///     rhv (Any): The right operand of a binary operation.
///     pos (int | None): The position of the error in the source.
///     expected (str | None): The missing closing bracket.
///     token (str | None): The misplaced token.
///
/// Example:
///     >>> context = pytexl.Engine().create_context()
///     >>> await context.render('<% err = "hello" - 1 %>')
///     ''
///     >>> err = context.get("err")
///     >>> err
///     <pytexl.Undefined BinaryOperation op='subtraction' lhv='hello' rhv=1 pos=15>
///     >>> err.op, err.lhv, err.rhv
///     ('subtraction', 'hello', 1)
#[pyclass(unsendable, module = "pytexl", name = "Undefined")]
pub struct Undefined {
    pub inner: Undef,
}

impl Undefined {
    /// The name of the kind of error and its members, in the order of `undef.inspect`.
    fn members(&self, py: Python<'_>) -> (&'static str, Vec<(&'static str, Py<PyAny>)>) {
        let text = |s: &str| PyString::new(py, s).into_any().unbind();
        let pos = |p: usize| p.into_pyobject(py).unwrap().into_any().unbind();
        match &self.inner {
            Undef::UnaryOperation { op, val, pos: p } => {
                ("UnaryOperation", vec![("op", text(op)), ("val", swan_to_py(py, val)), ("pos", pos(*p))])
            }
            Undef::BinaryOperation { op, lhv, rhv, pos: p } => (
                "BinaryOperation",
                vec![("op", text(op)), ("lhv", swan_to_py(py, lhv)), ("rhv", swan_to_py(py, rhv)), ("pos", pos(*p))],
            ),
            Undef::Termination { expected, pos: p } => {
                ("Termination", vec![("expected", text(&expected.to_string())), ("pos", pos(*p))])
            }
            Undef::Token { token, pos: p } => ("Token", vec![("token", text(token)), ("pos", pos(*p))]),
            Undef::Name { pos: p } => ("Name", vec![("pos", pos(*p))]),
            Undef::Value(val) => ("Value", vec![("val", swan_to_py(py, val))]),
        }
    }

    /// The member `name`, or `None` if this kind of error has no such member.
    fn member(&self, py: Python<'_>, name: &str) -> Py<PyAny> {
        let (_, members) = self.members(py);
        members.into_iter().find(|(n, _)| *n == name).map(|(_, v)| v).unwrap_or_else(|| py.None())
    }
}

#[pymethods]
impl Undefined {
    /// str: The kind of error, such as ``"BinaryOperation"``.
    #[getter(r#type)]
    fn kind(&self, py: Python<'_>) -> &'static str {
        self.members(py).0
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

    /// int | None: The byte offset of the error in the source expression.
    #[getter]
    fn pos(&self, py: Python<'_>) -> Py<PyAny> {
        self.member(py, "pos")
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

    /// Returns the details of the error as a dict, like ``undef.inspect`` in Swan.
    ///
    /// Returns:
    ///     dict[str, Any]: The ``type`` of the error and the members that apply to it.
    ///
    /// Example:
    ///     >>> err.inspect()
    ///     {'type': 'BinaryOperation', 'op': 'subtraction', 'lhv': 'hello', 'rhv': 1, 'pos': 15}
    fn inspect<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let (kind, members) = self.members(py);
        let dict = PyDict::new(py);
        dict.set_item("type", kind)?;
        for (name, value) in members {
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

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let (kind, members) = self.members(py);
        let mut repr = format!("<pytexl.Undefined {kind}");
        for (name, value) in members {
            repr.push_str(&format!(" {name}={}", value.bind(py).repr()?));
        }
        repr.push('>');
        Ok(repr)
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
        let scope = Scope::new();
        match for_each_entry(obj, |key, value| scope.set(key, value)) {
            Ok(()) => Value::Namespace(scope),
            Err(_) => Value::Nothing,
        }
    } else if is_awaitable(obj) {
        let awaitable = obj.clone().unbind();
        Value::Promise(runtime::promise_of(async move {
            match AwaitPy::new(awaitable).await {
                Some(result) => Python::attach(|py| py_to_swan(result.bind(py))),
                None => Value::Nothing,
            }
        }))
    } else if obj.is_callable() {
        Value::Builtin(py_function(obj.clone().unbind()))
    } else {
        Value::Nothing
    }
}

/// Wraps a Python callable as a Swan function. See [`Function`] for the argument convention,
/// which is the same in this direction. An exception gives `Nothing`.
fn py_function(func: Py<PyAny>) -> BuiltinFn {
    BuiltinFn::new("python", move |arg| {
        Python::attach(|py| {
            let args: Vec<Py<PyAny>> = match &arg {
                Value::Nothing => vec![],
                Value::Tuple(items) => items.iter().map(|item| swan_to_py(py, item)).collect(),
                other => vec![swan_to_py(py, other)],
            };
            let args = match PyTuple::new(py, args) {
                Ok(args) => args,
                Err(_) => return Value::Nothing,
            };
            match func.bind(py).call1(args) {
                Ok(result) => py_to_swan(&result),
                Err(_) => Value::Nothing,
            }
        })
    })
}

/// Converts a Swan value to a Python value. See the [module documentation](self).
pub fn swan_to_py(py: Python<'_>, value: &Value) -> Py<PyAny> {
    to_py(py, value, &mut Vec::new())
}

/// `parents` holds the namespaces being converted, so that a namespace containing
/// itself gives `None` instead of recursing forever.
fn to_py(py: Python<'_>, value: &Value, parents: &mut Vec<Scope>) -> Py<PyAny> {
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
            let items: Vec<_> = items.iter().map(|item| to_py(py, item, parents)).collect();
            PyList::new(py, items).map(Bound::into_any)
        }
        Value::Tuple(items) => {
            let items: Vec<_> = items.iter().map(|item| to_py(py, item, parents)).collect();
            PyTuple::new(py, items).map(Bound::into_any)
        }
        Value::Namespace(scope) => {
            if parents.iter().any(|p| Rc::ptr_eq(&p.0, &scope.0)) {
                return py.None();
            }
            parents.push(scope.clone());
            let dict = PyDict::new(py);
            let mut bindings: Vec<(String, Value)> =
                scope.0.borrow().bindings.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            bindings.sort_by(|a, b| a.0.cmp(&b.0));
            let mut result = Ok(());
            for (key, item) in bindings {
                result = result.and(dict.set_item(key, to_py(py, &item, parents)));
            }
            parents.pop();
            result.map(|()| dict.into_any())
        }
        Value::Promise(p) => Bound::new(py, Promise { inner: p.clone() }).map(Bound::into_any),
        Value::Func { .. } | Value::Builtin(_) => {
            Bound::new(py, Function { inner: value.clone() }).map(Bound::into_any)
        }
    };
    converted.map(Bound::unbind).unwrap_or_else(|_| py.None())
}
