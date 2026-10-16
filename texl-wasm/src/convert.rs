//! Conversions between JS values and Swan values.
//!
//! | JavaScript                     | Swan                                     |
//! |--------------------------------|------------------------------------------|
//! | `null`, `undefined`            | `Nothing`                                |
//! | boolean                        | `Bool`                                   |
//! | number                         | `Numb`                                   |
//! | string                         | `Text`                                   |
//! | array                          | `List`                                   |
//! | Promise or other thenable      | `Promise` (a rejection gives `Nothing`)  |
//! | function                       | `Func` taking one argument               |
//! | other object                   | `Namespace` of its own enumerable keys   |
//!
//! In the other direction, `Nothing` becomes `null`, lists and tuples become
//! arrays, namespaces become plain objects (own bindings only), promises
//! become JS Promises, and functions and undefined values become `undefined`.

use js_sys::{Array, Function, Object, Reflect};
use swan_core::value::{BuiltinFn, Promise, Scope, Value};
use wasm_bindgen::prelude::*;

/// True for native Promises and any other "thenable" (object with a callable `then`).
fn is_thenable(val: &JsValue) -> bool {
    val.is_instance_of::<js_sys::Promise>()
        || (val.is_object()
            && Reflect::get(val, &JsValue::from_str("then"))
                .map(|t| t.is_function())
                .unwrap_or(false))
}

/// Calls `f` with each own enumerable `(key, value)` pair of `obj`.
pub fn for_each_entry(
    obj: &Object,
    mut f: impl FnMut(String, Value),
) -> Result<(), JsValue> {
    let keys = Object::keys(obj);
    for i in 0..keys.length() {
        let key = keys.get(i);
        if let Some(key_str) = key.as_string() {
            let js_val = Reflect::get(obj, &key)?;
            f(key_str, js_to_swan_value(js_val)?);
        }
    }
    Ok(())
}

// ----------------------------------------------------------------------------
// JS -> Swan Conversion
// ----------------------------------------------------------------------------

/// Converts a JS value to a Swan value. See the [module documentation](self).
pub fn js_to_swan_value(val: JsValue) -> Result<Value, JsValue> {
    if val.is_null() || val.is_undefined() {
        Ok(Value::Nothing)
    } else if let Some(b) = val.as_bool() {
        Ok(Value::Bool(b))
    } else if let Some(n) = val.as_f64() {
        Ok(Value::Numb(n))
    } else if let Some(s) = val.as_string() {
        Ok(Value::Text(s.into()))
    } else if Array::is_array(&val) {
        let arr = Array::from(&val);
        let mut items = Vec::with_capacity(arr.length() as usize);
        for i in 0..arr.length() {
            items.push(js_to_swan_value(arr.get(i))?);
        }
        Ok(Value::list(items))
    } else if is_thenable(&val) {
        let swan_promise = Promise::new();
        let sp = swan_promise.clone();
        // Normalises thenables / foreign-realm promises into a native Promise
        let js_promise = js_sys::Promise::resolve(&val);
        wasm_bindgen_futures::spawn_local(async move {
            let result = wasm_bindgen_futures::JsFuture::from(js_promise).await;
            let v = match result {
                Ok(v)  => js_to_swan_value(v).unwrap_or(Value::Nothing),
                Err(_) => Value::Nothing, // Swan promises never reject
            };
            sp.resolve(v);
        });
        Ok(Value::Promise(swan_promise))
    } else if val.is_function() {
        // Wrap JS Function as a Swan BuiltinFn
        // If the JS function is async (returns a Promise), the result is converted by
        // js_to_swan_value into a Swan Promise, so Swan sees it as a promise-returning function.
        let js_fn: Function = val.into();

        let builtin = BuiltinFn::new("js_callback", move |arg: Value| {
            let js_arg = swan_value_to_js(&arg);
            match js_fn.call1(&JsValue::NULL, &js_arg) {
                Ok(res) => js_to_swan_value(res).unwrap_or(Value::Nothing),
                Err(_) => Value::Nothing,
            }
        });

        Ok(Value::Builtin(builtin))
    } else if val.is_object() {
        // Convert JS Object to nested Swan Scope/Namespace
        let sub_scope = Scope::new();
        for_each_entry(&val.into(), |key, value| sub_scope.set(key, value))?;
        Ok(Value::Namespace(sub_scope))
    } else {
        Ok(Value::Nothing)
    }
}

// ----------------------------------------------------------------------------
// Swan -> JS Conversion
// ----------------------------------------------------------------------------

/// Converts a Swan value to a JS value. See the [module documentation](self).
pub fn swan_value_to_js(val: &Value) -> JsValue {
    match val {
        Value::Nothing => JsValue::NULL,
        Value::Bool(b) => JsValue::from_bool(*b),
        Value::Numb(n) => JsValue::from_f64(*n),
        Value::Text(s) => JsValue::from_str(s),
        Value::List(items) => {
            let arr = Array::new();
            for item in items.iter() {
                arr.push(&swan_value_to_js(item));
            }
            arr.into()
        }
        Value::Tuple(items) => {
            let arr = Array::new();
            for item in items.iter() {
                arr.push(&swan_value_to_js(item));
            }
            arr.into()
        }
        Value::Namespace(scope) => {
            let obj = Object::new();
            let bindings = scope.0.borrow().bindings.clone();
            for (k, v) in bindings {
                let _ = Reflect::set(&obj, &JsValue::from_str(&k), &swan_value_to_js(&v));
            }
            obj.into()
        }
        Value::Promise(swan_promise) => {
            let swan_promise = swan_promise.clone();
            let js_promise = js_sys::Promise::new(&mut |resolve: Function, _reject: Function| {
                // `then` fires immediately if already resolved, otherwise on resolution.
                swan_promise.then(move |v| {
                    let _ = resolve.call1(&JsValue::UNDEFINED, &swan_value_to_js(&v));
                    Value::Nothing
                });
            });
            js_promise.into()
        }
        _ => JsValue::UNDEFINED,
    }
}
