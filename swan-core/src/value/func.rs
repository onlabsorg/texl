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
#[derive(Clone)]
pub struct BuiltinFn {
    /// Human-readable name, used in Debug output and error messages.
    pub name: &'static str,
    func: Rc<dyn Fn(Value) -> Value>,
}

impl BuiltinFn {
    /// Create a new built-in function from any `Fn(Value) -> Value`.
    ///
    /// Both plain function pointers (`fn(Value) -> Value`) and closures are
    /// accepted because both coerce to `dyn Fn(Value) -> Value`.
    pub fn new(name: &'static str, f: impl Fn(Value) -> Value + 'static) -> Self {
        Self { name, func: Rc::new(f) }
    }

    /// Invoke the built-in with the given argument.
    #[inline]
    pub fn call(&self, arg: Value) -> Value {
        (self.func)(arg)
    }
}

impl std::fmt::Debug for BuiltinFn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<builtin {}>", self.name)
    }
}


