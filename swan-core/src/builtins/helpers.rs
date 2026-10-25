

use crate::value::{BuiltinFn, Value};



/// Wraps a single-argument built-in function `fn(Value) -> Value` so that
/// if applied to a `Tuple`, it maps over each element and returns a `Tuple`.
pub fn map_1(name: &'static str, f: impl Fn(Value) -> Value + 'static + Clone) -> Value {
    Value::Builtin(BuiltinFn::new(name, move |arg| arg.map(f.clone())))
}

/// Wraps a truly curried 2-argument built-in: `f arg1 arg2`.
/// Each application is mapped over tuples via `map_1`.
/// Note: calling it with one tuple `(a, b)` does NOT work; use `binary_tuple` for that.
pub fn map_curried(
    name: &'static str,
    f: impl Fn(Value, Value) -> Value + 'static + Copy,
) -> Value {
    map_1(name, move |arg1| {
        // arg1 is captured where the cycle collector can see it
        Value::Builtin(BuiltinFn::capturing(name, vec![arg1], move |captured, arg| {
            let arg1 = captured[0].clone();
            arg.map(move |arg2| f(arg1.clone(), arg2))
        }))
    })
}
