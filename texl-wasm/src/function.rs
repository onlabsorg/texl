//! Swan functions in JavaScript.
//!
//! A Swan function (a `Func` or a built-in) becomes a plain JS function, a
//! handle of the Swan value (see [`crate::handles`]): it holds no wasm memory
//! and needs no `free()`, and JS collects it like any other function. Passed
//! back to the engine, it gives the original Swan function. All the functions
//! are made by one JS factory and call one shared Rust function with their
//! key, so a function keeps nothing else alive.
//!
//! The JS arguments are the items of the Swan argument: `f(a, b)` calls the
//! Swan function with the tuple `(a, b)`, `f(a)` with `a` and `f()` with `()`
//! (`Nothing`). `this` is ignored. The result is converted like any value: a
//! promise becomes a JS Promise, and an undefined value is returned as a
//! `TexlUndefined`, not thrown.
//!
//! A built-in that wraps a JS function (a JS function passed to the engine)
//! gives back that JS function instead.

use std::cell::OnceCell;

use js_sys::{Array, Function};
use swan_core::evaluator::apply;
use swan_core::value::Value;
use wasm_bindgen::prelude::*;

use crate::convert::{array_items, swan_value_to_js};
use crate::handles;

#[wasm_bindgen(inline_js = "export function swanFunction(call, key) { return function (...args) { return call(key, args); }; }")]
extern "C" {
    /// A JS function that calls `call(key, args)` with the array of its arguments.
    #[wasm_bindgen(js_name = swanFunction)]
    fn make_function(call: &Function, key: f64) -> Function;
}

thread_local! {
    /// The `call` of every function made by [`function_to_js`]: see [`call`].
    static CALL: OnceCell<Function> = const { OnceCell::new() };
}

/// Calls the Swan function with this key with the JS arguments `args`.
fn call(key: f64, args: Array) -> Result<JsValue, JsValue> {
    // The key is found while its function is alive, which is while it can be called
    let Some(func) = handles::get(key as u32) else { return Ok(JsValue::NULL) };
    let arg = Value::tuple(array_items(&args)?);
    Ok(swan_value_to_js(&apply(func, arg)))
}

/// Converts a Swan function (`Func` or `Builtin`) to a JS function.
pub fn function_to_js(func: &Value) -> JsValue {
    if let Value::Builtin(builtin) = func
        && let Some(js_fn) = builtin.host_data::<Function>()
    {
        return js_fn.clone().into();
    }
    let call = CALL.with(|cell| {
        cell.get_or_init(|| {
            let closure = Closure::<dyn Fn(f64, Array) -> Result<JsValue, JsValue>>::new(call);
            let call: Function = closure.as_ref().unchecked_ref::<Function>().clone();
            closure.forget(); // lives as long as the module
            call
        })
        .clone()
    });
    let key = handles::insert(func.clone());
    let js_fn = make_function(&call, key as f64);
    handles::register(&js_fn, key);
    js_fn.into()
}

/// The Swan function `val` is the JS form of, if it is one.
pub fn js_to_function(val: &JsValue) -> Option<Value> {
    handles::value_of(val).filter(|v| matches!(v, Value::Func { .. } | Value::Builtin(_)))
}
