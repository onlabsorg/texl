use super::*;




// ----------------------------------------------------------------------------
//  BuiltinFn  (a native Rust function callable from Swan)
// ----------------------------------------------------------------------------

/// A native Rust function that can be called from Swan code.
///
/// # Why a newtype instead of a bare `Rc<dyn Fn>`?
///
/// 1. **Debug**: `dyn Fn` is not `Debug`; storing a `name` gives meaningful
///    output in error messages and test output.
/// 2. **Metadata**: the `name` can be used when building `Undefined` error
///    values ("function `math.sin` called on a Bool").
/// 3. **Ergonomics**: `BuiltinFn::new("name", |arg| ...)` is self-documenting.
///
/// # Argument convention
///
/// The function receives the evaluated right-hand side as a single `Value`.
/// Multi-argument calls arrive as `Value::Tuple`.  The function is responsible
/// for destructuring the tuple when it expects more than one argument.
///
/// # Return value
///
/// The function must never panic.  Return a [`Value::Undefined`] (for example
/// with [`Value::undef_unary`]) for any input that is invalid.
///
/// # Examples
///
/// ```
/// use swan_core::value::{BuiltinFn, Value};
///
/// let negate = BuiltinFn::new("negate", |arg| match arg {
///     Value::Numb(n) => Value::Numb(-n),
///     other => Value::undef_unary("negate", other, 0),
/// });
/// assert_eq!(negate.call(Value::Numb(2.0)), Value::Numb(-2.0));
/// ```
///
/// # Captured values
///
/// A closure passed to [`BuiltinFn::new`] is opaque: the cycle collector
/// ([`crate::gc`]) cannot see the values it captures. A built-in that holds
/// Swan values (functions, promises, collections) should use
/// [`BuiltinFn::capturing`] instead, which keeps them where the collector can
/// trace them.
#[derive(Clone)]
pub struct BuiltinFn {
    /// Human-readable name, used in Debug output and error messages.
    pub name: &'static str,
    body: BuiltinBody,
}

/// A built-in that receives its captured values on every call.
type CapturingFn = Rc<dyn Fn(&[Value], Value) -> Value>;

#[derive(Clone)]
enum BuiltinBody {
    Plain(Rc<dyn Fn(Value) -> Value>),
    Capturing(CapturingFn, Rc<Vec<Value>>),
}

impl BuiltinFn {
    /// Create a new built-in function from any `Fn(Value) -> Value`.
    ///
    /// Both plain function pointers (`fn(Value) -> Value`) and closures are
    /// accepted because both coerce to `dyn Fn(Value) -> Value`.
    pub fn new(name: &'static str, f: impl Fn(Value) -> Value + 'static) -> Self {
        Self { name, body: BuiltinBody::Plain(Rc::new(f)) }
    }

    /// Create a built-in function that holds the Swan values `captured`.
    ///
    /// `f` receives `captured` and the argument on every call. `f` itself
    /// should capture no Swan value, so that the cycle collector sees every
    /// value the built-in holds (see [`crate::gc`]).
    ///
    /// # Examples
    ///
    /// ```
    /// use swan_core::value::{BuiltinFn, Value};
    ///
    /// let add = BuiltinFn::capturing("add", vec![Value::Numb(1.0)], |captured, arg| {
    ///     match (&captured[0], arg) {
    ///         (Value::Numb(a), Value::Numb(b)) => Value::Numb(a + b),
    ///         (_, other) => Value::undef_unary("add", other, 0),
    ///     }
    /// });
    /// assert_eq!(add.call(Value::Numb(2.0)), Value::Numb(3.0));
    /// ```
    pub fn capturing(
        name: &'static str,
        captured: Vec<Value>,
        f: impl Fn(&[Value], Value) -> Value + 'static,
    ) -> Self {
        Self { name, body: BuiltinBody::Capturing(Rc::new(f), Rc::new(captured)) }
    }

    /// Invoke the built-in with the given argument.
    #[inline]
    pub fn call(&self, arg: Value) -> Value {
        match &self.body {
            BuiltinBody::Plain(f) => f(arg),
            BuiltinBody::Capturing(f, captured) => f(captured, arg),
        }
    }

    /// The values captured with [`BuiltinFn::capturing`], if any.
    pub(crate) fn captured(&self) -> Option<&Rc<Vec<Value>>> {
        match &self.body {
            BuiltinBody::Plain(_) => None,
            BuiltinBody::Capturing(_, captured) => Some(captured),
        }
    }
}

impl std::fmt::Debug for BuiltinFn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<builtin {}>", self.name)
    }
}


