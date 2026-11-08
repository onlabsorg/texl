//! Swan values held by JS objects.
//!
//! Some Swan values become JS objects that must turn back into the same Swan
//! value when JS passes them back to the engine: undefined values
//! (`TexlUndefined`, see [`crate::undefined`]) and functions (see
//! [`crate::function`]). Such an object is a *handle*: it holds no wasm
//! memory, only a key into a table of Swan values. A module-private `WeakMap`
//! maps the object to its key, and a `FinalizationRegistry` removes the entry
//! when JS collects the object, so the table never keeps a value the host no
//! longer uses. Nothing outside this crate can make an object that is found
//! in the table, and the lookup runs no JS code.
//!
//! The table holds its values strongly: for the cycle collector, a value with
//! a handle is referenced from outside Swan. When the entry is removed, the
//! value is freed with its last reference, or by the next collection if it is
//! in a reference cycle.

use std::cell::RefCell;
use std::collections::HashMap;

use js_sys::{FinalizationRegistry, Object, WeakMap};
use swan_core::value::Value;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

/// The Swan values held by JS objects, by key.
struct Handles {
    /// JS object -> key in `values`.
    keys: WeakMap,
    /// Removes the entry of a collected object from `values`.
    finalizer: FinalizationRegistry,
    values: HashMap<u32, Value>,
    next: u32,
}

thread_local! {
    static HANDLES: RefCell<Option<Handles>> = const { RefCell::new(None) };
}

/// Runs `f` with the table, creating it on first use.
fn with_handles<R>(f: impl FnOnce(&mut Handles) -> R) -> R {
    HANDLES.with(|handles| {
        let mut handles = handles.borrow_mut();
        let handles = handles.get_or_insert_with(|| {
            let cleanup = Closure::<dyn FnMut(JsValue)>::new(|held: JsValue| {
                if let Some(key) = held.as_f64() {
                    // Taken out of the table first: dropping the value can run Rust drop code
                    let value = HANDLES.with(|handles| {
                        handles.borrow_mut().as_mut().and_then(|h| h.values.remove(&(key as u32)))
                    });
                    drop(value);
                }
            });
            let finalizer = FinalizationRegistry::new(cleanup.as_ref().unchecked_ref());
            cleanup.forget(); // lives as long as the module
            Handles { keys: WeakMap::new(), finalizer, values: HashMap::new(), next: 0 }
        });
        f(handles)
    })
}

/// Stores `value` in the table and returns its key. The entry is removed
/// when the object made a handle of it with [`register`] is collected.
pub fn insert(value: Value) -> u32 {
    with_handles(|handles| {
        let key = handles.next;
        handles.next = handles.next.wrapping_add(1);
        handles.values.insert(key, value);
        key
    })
}

/// Makes `obj` the handle of the value with this key (from [`insert`]).
pub fn register(obj: &Object, key: u32) {
    with_handles(|handles| {
        let held = JsValue::from_f64(key as f64);
        handles.keys.set(obj, &held);
        handles.finalizer.register(obj, &held);
    })
}

/// The value with this key, if its handle has not been collected.
pub fn get(key: u32) -> Option<Value> {
    with_handles(|handles| handles.values.get(&key).cloned())
}

/// The value `val` is a handle of, if it is one.
pub fn value_of(val: &JsValue) -> Option<Value> {
    if !val.is_object() && !val.is_function() {
        return None;
    }
    with_handles(|handles| {
        let key = handles.keys.get(val.unchecked_ref::<Object>()).as_f64()?;
        handles.values.get(&(key as u32)).cloned()
    })
}
