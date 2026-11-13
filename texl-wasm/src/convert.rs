//! Conversions between JS values and Swan values.
//!
//! | JavaScript                     | Swan                                     |
//! |--------------------------------|------------------------------------------|
//! | `null`, `undefined`            | `Nothing`                                |
//! | boolean                        | `Bool`                                   |
//! | number                         | `Numb`                                   |
//! | string                         | `Text`                                   |
//! | [`Tuple`]                      | `Tuple` (normalized)                     |
//! | other array                    | `List` (normalized)                      |
//! | Promise or other thenable      | `Promise` (a rejection gives `Undefined`) |
//! | function                       | `Func` (a throw gives `Undefined`)       |
//! | `TexlUndefined` (from Swan)    | the original `Undefined`                 |
//! | function from Swan             | the original `Func`                      |
//! | other object                   | `Namespace` of its own enumerable keys   |
//!
//! In the other direction, `Nothing` becomes `null`, lists become arrays,
//! tuples become [`Tuple`] objects, namespaces become plain objects (own
//! bindings only), promises become JS Promises, functions become JS functions
//! (see [`crate::function`]; a JS function passed to Swan comes back as
//! itself), and undefined values become `TexlUndefined` objects (see
//! [`crate::undefined`]).
//!
//! A Swan promise that holds itself (`promise.resolve p [p]`) gives one JS
//! Promise, which holds itself the same way; resolved with itself alone, it
//! gives a JS Promise of `null`, since JS has no such promise.
//!
//! A JS function called from Swan gets the items of the Swan argument as its
//! arguments: Swan `f(a, b)` calls it with `a` and `b`, `f(a)` with `a`, and
//! `f()` with none. A JS call of a Swan function does the same the other way. Positions are
//! converted to UTF-16 code units (JS string indices): the `pos` of an
//! undefined value, and the tag spans of a document namespace (see
//! [`texl_core::engine::convert_tag_positions`]). An object with the members
//! of a document gets its tag spans converted back to bytes, so a document
//! passed back to Swan keeps its positions.

use std::cell::RefCell;
use std::rc::Rc;

use js_sys::{Array, Function, Object, Reflect};
use texl_core::document::{utf16_offset, utf16_to_byte_offset};
use swan_core::value::{BuiltinFn, Namespace, Promise, Value};

use crate::function::function_to_js;
use crate::handles;
use crate::undefined::{js_to_undefined, undefined_to_js};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(inline_js = "export class Tuple extends Array {}")]
extern "C" {
    /// The JavaScript class of Swan tuples, exported by the package. It is an
    /// `Array` subclass with nothing added, so it reads like any array, and
    /// `instanceof` tells it from a list.
    #[wasm_bindgen(extends = Array)]
    pub type Tuple;

    #[wasm_bindgen(constructor)]
    fn new() -> Tuple;

    // Exports the class from the package. This is on a function because
    // `reexport` on a type always declares `Tuple: unknown` in TypeScript,
    // while on a function it honors `skip_typescript`, so that `TS_TYPES` in
    // lib.rs can declare the class. It is never called.
    #[wasm_bindgen(js_name = Tuple, reexport, skip_typescript)]
    fn tuple_class_export();
}

#[wasm_bindgen]
extern "C" {
    /// JavaScript's `String(value)`, which throws for some objects.
    #[wasm_bindgen(catch, js_name = String)]
    pub(crate) fn js_string(value: &JsValue) -> Result<String, JsValue>;
}

/// The depth a call of a JS function counts for (see
/// `swan_core::evaluator::descend`). A round trip from Swan through a JS
/// function and back into Swan uses the browser's call stack, about as much
/// as this many levels of evaluation (measured in Chrome).
const HOST_CALL_LEVELS: usize = 16;

thread_local! {
    // The Swan promises whose value is being converted to JS, each with the
    // JS Promise made for it (see `swan_value_to_js`).
    static CONVERTING: RefCell<Vec<(*const (), JsValue)>> = const { RefCell::new(Vec::new()) };
}

thread_local! {
    // The JS arrays and objects being converted to Swan or normalized, from
    // the outermost in: one met again inside itself is a cycle.
    static PATH: RefCell<Vec<JsValue>> = const { RefCell::new(Vec::new()) };
}

/// Runs `f`, which converts the items or members of the array or object
/// `val`. A value that contains itself (`a.self = a`, a DOM node, `window`)
/// would be followed forever: it is a `TypeError` instead.
fn inside<R>(val: &JsValue, f: impl FnOnce() -> Result<R, JsValue>) -> Result<R, JsValue> {
    if PATH.with(|path| path.borrow().iter().any(|outer| Object::is(outer, val))) {
        return Err(js_sys::TypeError::new("cannot convert a value that contains itself").into());
    }
    PATH.with(|path| path.borrow_mut().push(val.clone()));
    let result = f();
    PATH.with(|path| path.borrow_mut().pop());
    result
}

/// The JS Promise of the Swan promise at `key`, if its value is being converted.
fn converting(key: *const ()) -> Option<JsValue> {
    CONVERTING.with(|c| c.borrow().iter().find(|(k, _)| *k == key).map(|(_, js)| js.clone()))
}

/// The `Undefined` of what JS threw or rejected with, an `Error` or any other value.
fn host_error(thrown: &JsValue) -> Value {
    Value::undef_host_error(js_string(thrown).unwrap_or_else(|_| "unknown error".to_string()))
}

/// The Swan value of `result`, the outcome of JS code: the `Undefined` of the error
/// if it threw, or if reading the value threw (a getter, say).
fn outcome_to_swan(result: Result<JsValue, JsValue>) -> Value {
    result.and_then(js_to_swan_value).unwrap_or_else(|thrown| host_error(&thrown))
}

/// Appends each item of `items`, converted to JS, to `arr`.
fn push_items(arr: &Array, items: &[Value]) {
    for item in items {
        arr.push(&swan_value_to_js(item));
    }
}

/// Converts each item of the JS array `arr` to a Swan value.
pub(crate) fn array_items(arr: &Array) -> Result<Vec<Value>, JsValue> {
    let mut items = Vec::with_capacity(arr.length() as usize);
    for i in 0..arr.length() {
        items.push(js_to_swan_value(arr.get(i))?);
    }
    Ok(items)
}

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
    } else if let Some(value) = handles::value_of(&val) {
        // A `TexlUndefined` or a function from Swan: the original value
        Ok(value)
    } else if val.is_instance_of::<Tuple>() {
        // Before the array test, since a Tuple is also an array
        inside(&val, || Ok(Value::tuple(array_items(val.unchecked_ref())?)))
    } else if Array::is_array(&val) {
        inside(&val, || Ok(Value::list(array_items(val.unchecked_ref())?)))
    } else if is_thenable(&val) {
        let swan_promise = Promise::new();
        let sp = swan_promise.clone();
        // Normalises thenables / foreign-realm promises into a native Promise
        let js_promise = js_sys::Promise::resolve(&val);
        wasm_bindgen_futures::spawn_local(async move {
            let result = wasm_bindgen_futures::JsFuture::from(js_promise).await;
            sp.resolve(outcome_to_swan(result)); // Swan promises never reject
        });
        Ok(Value::Promise(swan_promise))
    } else if val.is_function() {
        // A JS function that returns a promise (an async function) gives a Swan promise.
        // The built-in holds the JS function as host data, so it leaves Swan as itself.
        let js_fn: Function = val.into();
        let host_fn = js_fn.clone();
        let builtin = BuiltinFn::new("js_callback", move |arg: Value| {
            // A JS function that calls back into Swan uses the browser's stack
            let _levels = match swan_core::evaluator::descend(HOST_CALL_LEVELS) {
                Ok(levels) => levels,
                Err(too_deep) => return too_deep,
            };
            // The items of the argument are the arguments
            let args = Array::new();
            match &arg {
                Value::Nothing => {}
                Value::Tuple(items) => push_items(&args, items),
                other => { args.push(&swan_value_to_js(other)); }
            }
            outcome_to_swan(js_fn.apply(&JsValue::NULL, &args))
        })
        .with_host_data(host_fn);

        Ok(Value::Builtin(builtin))
    } else if val.is_object() {
        // Convert JS Object to nested Swan Namespace
        let mut ns = Namespace::new();
        inside(&val, || for_each_entry(val.unchecked_ref(), |key, value| ns.insert(key, value)))?;
        // A document comes back with its tag positions in bytes
        let ns = texl_core::engine::convert_tag_positions(&ns, utf16_to_byte_offset).unwrap_or(ns);
        Ok(Value::Namespace(ns))
    } else {
        Ok(Value::Nothing)
    }
}

// ----------------------------------------------------------------------------
// Normalization
// ----------------------------------------------------------------------------

/// True for an object made by `{...}` or `Object.create(null)`.
fn is_plain_object(val: &JsValue) -> bool {
    if !val.is_object() {
        return false;
    }
    let proto = Object::get_prototype_of(val);
    proto.is_null() || Object::get_prototype_of(&proto).is_null()
}

/// The normalized items of `arr`: tuples spread, `null` and `undefined` dropped.
fn normalized_items(arr: &Array) -> Result<Vec<JsValue>, JsValue> {
    let mut items = Vec::with_capacity(arr.length() as usize);
    for i in 0..arr.length() {
        let item = normalize(arr.get(i))?;
        if item.is_instance_of::<Tuple>() {
            items.extend(Array::from(&item).iter());
        } else if !item.is_null() {
            items.push(item);
        }
    }
    Ok(items)
}

/// Gives a JS value the form Swan would give it: see `normalize` in lib.rs.
pub fn normalize(val: JsValue) -> Result<JsValue, JsValue> {
    if val.is_null() || val.is_undefined() {
        Ok(JsValue::NULL)
    } else if js_to_undefined(&val).is_some() {
        Ok(val)
    } else if val.is_instance_of::<Tuple>() {
        let mut items = inside(&val, || normalized_items(val.unchecked_ref()))?;
        Ok(match items.len() {
            0 => JsValue::NULL,
            1 => items.remove(0),
            _ => {
                let tuple = Tuple::new();
                items.iter().for_each(|item| { tuple.push(item); });
                tuple.into()
            }
        })
    } else if Array::is_array(&val) {
        let arr = Array::new();
        inside(&val, || normalized_items(val.unchecked_ref()))?.iter().for_each(|item| { arr.push(item); });
        Ok(arr.into())
    } else if is_thenable(&val) || !is_plain_object(&val) {
        Ok(val)
    } else {
        // Members are kept, also when they are `null`
        let obj = Object::new();
        let keys = Object::keys(val.unchecked_ref::<Object>());
        inside(&val, || {
            for i in 0..keys.length() {
                let key = keys.get(i);
                Reflect::set(&obj, &key, &normalize(Reflect::get(&val, &key)?)?)?;
            }
            Ok(())
        })?;
        Ok(obj.into())
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
            push_items(&arr, items);
            arr.into()
        }
        Value::Tuple(items) => {
            let tuple = Tuple::new();
            push_items(&tuple, items);
            tuple.into()
        }
        Value::Namespace(ns) => {
            // A document gets its tag positions in UTF-16 code units
            let converted = texl_core::engine::convert_tag_positions(ns, utf16_offset);
            let ns = converted.as_ref().unwrap_or(ns);
            let obj = Object::new();
            for (k, v) in ns.iter() {
                let _ = Reflect::set(&obj, &JsValue::from_str(k), &swan_value_to_js(v));
            }
            obj.into()
        }
        Value::Promise(swan_promise) => {
            // A promise whose value is being converted, met again inside that
            // value (`promise.resolve p [p]`): the JS Promise being made for it
            let key = Rc::as_ptr(&swan_promise.0) as *const ();
            if let Some(js_promise) = converting(key) {
                return js_promise;
            }
            let mut resolver = None;
            let js_promise: JsValue = js_sys::Promise::new(&mut |resolve: Function, _reject: Function| {
                resolver = Some(resolve);
            }).into();
            let Some(resolve) = resolver else { return js_promise };
            let own = js_promise.clone();
            // `then` fires immediately if already resolved, otherwise on resolution.
            swan_promise.then(move |v| {
                CONVERTING.with(|c| c.borrow_mut().push((key, own.clone())));
                let value = swan_value_to_js(&v);
                CONVERTING.with(|c| c.borrow_mut().pop());
                // JS rejects a promise resolved with itself
                let value = if Object::is(&value, &own) { JsValue::NULL } else { value };
                let _ = resolve.call1(&JsValue::UNDEFINED, &value);
                Value::Nothing
            });
            js_promise
        }
        Value::Undefined(undef) => undefined_to_js(undef),
        Value::Func { .. } | Value::Builtin(_) => function_to_js(val),
        Value::Name(_) => JsValue::UNDEFINED,
    }
}
