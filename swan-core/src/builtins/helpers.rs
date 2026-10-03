

use crate::value::{BuiltinFn, Value};



/// Wraps a single-argument built-in function `fn(Value) -> Value` so that
/// if applied to a `Tuple`, it maps over each element and returns a `Tuple`.
pub fn map_1(name: &'static str, f: impl Fn(Value) -> Value + 'static + Clone) -> Value {
    Value::Builtin(BuiltinFn::new(name, move |arg| arg.map(f.clone())))
}

/// Wraps a curried 2-argument built-in function so that if given tuples,
/// it pairs the tuple elements using `arg1.pair(&arg2)` and returns a mapped `Tuple`.
pub fn map_curried(
    name: &'static str,
    f: impl Fn(Value, Value) -> Value + 'static + Copy,
) -> Value {
    map_1(name, move |arg1| {
        map_1(name, move |arg2| f(arg1.clone(), arg2))
    })
}
