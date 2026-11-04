//! Undefined values in JavaScript.
//!
//! A Swan undefined value becomes a plain, frozen JS object (`TexlUndefined`
//! in the TypeScript types) with every detail already converted: `type`,
//! `description`, the operands (`op`, `val`, `lhv`, `rhv`), `expected`,
//! `token`, `error`, `pos` (in UTF-16 code units, a JS string index), `uri` and
//! `cause`. Members that do not apply are `null`. Only `source`, the text
//! `pos` indexes, is not converted up front: it can be a whole document, so
//! it is a getter that makes the string each time it is read.
//!
//! The object holds no wasm memory, so it needs no `free()`. To give the
//! original value back to Swan when the object is passed back (to
//! `context.set`, or as the result of a JS function), a module-private
//! `WeakMap` maps each object to a key in a table of Swan values; a
//! `FinalizationRegistry` removes the entry when JS collects the object.
//! The getters are one shared function bound to that key, so they do not
//! keep the object alive. Nothing outside this module can make an object that converts to an
//! undefined value, and the lookup runs no JS code.

use std::cell::RefCell;
use std::collections::HashMap;

use js_sys::{FinalizationRegistry, Function, Object, Reflect, WeakMap};
use swan_core::value::{Undef, Value};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use texl_core::document::{locate, members, utf16_offset};

use crate::convert::swan_value_to_js;

/// The undefined values given to JS, by key.
struct Registry {
    /// JS object -> key in `undefs`.
    keys: WeakMap,
    /// Removes the entry of a collected object from `undefs`.
    finalizer: FinalizationRegistry,
    /// Gives the source of the entry whose key is its argument: see [`source_of`].
    source_getter: Function,
    undefs: HashMap<u32, Undef>,
    next: u32,
}

thread_local! {
    static REGISTRY: RefCell<Option<Registry>> = const { RefCell::new(None) };
}

/// Runs `f` with the registry, creating it on first use.
fn with_registry<R>(f: impl FnOnce(&mut Registry) -> R) -> R {
    REGISTRY.with(|reg| {
        let mut reg = reg.borrow_mut();
        let reg = reg.get_or_insert_with(|| {
            let cleanup = Closure::<dyn FnMut(JsValue)>::new(|held: JsValue| {
                if let Some(key) = held.as_f64() {
                    REGISTRY.with(|reg| {
                        if let Some(reg) = reg.borrow_mut().as_mut() {
                            reg.undefs.remove(&(key as u32));
                        }
                    });
                }
            });
            let finalizer = FinalizationRegistry::new(cleanup.as_ref().unchecked_ref());
            cleanup.forget(); // lives as long as the module
            let getter = Closure::<dyn Fn(f64) -> JsValue>::new(source_of);
            let source_getter = getter.as_ref().unchecked_ref::<Function>().clone();
            getter.forget(); // lives as long as the module
            Registry { keys: WeakMap::new(), finalizer, source_getter, undefs: HashMap::new(), next: 0 }
        });
        f(reg)
    })
}

/// The source of the undefined value with this key in the registry, `null`
/// if it has none or its object has been collected.
fn source_of(key: f64) -> JsValue {
    with_registry(|reg| reg.undefs.get(&(key as u32)).cloned())
        .and_then(|undef| locate(&undef))
        .map_or(JsValue::NULL, |loc| JsValue::from_str(&loc.text))
}

/// The members of every `TexlUndefined`, `null` when they do not apply.
const MEMBERS: [&str; 11] = ["op", "val", "lhv", "rhv", "expected", "token", "error", "pos", "source", "uri", "cause"];

/// Converts an undefined value to a `TexlUndefined` object.
pub fn undefined_to_js(undef: &Undef) -> JsValue {
    let obj = Object::new();
    let set = |name: &str, value: &JsValue| { let _ = Reflect::set(&obj, &JsValue::from_str(name), value); };
    set("type", &JsValue::from_str(undef.kind()));
    set("description", &JsValue::from_str(&undef.describe()));
    for name in MEMBERS {
        set(name, &JsValue::NULL);
    }
    // The members in document terms (`uri`)
    for (name, value) in members(undef) {
        match name {
            "type" | "description" | "pos" => {}
            _ => set(name, &swan_value_to_js(&value)),
        }
    }
    // A position that cannot be located in a document or expression stays `null`
    let location = locate(undef);
    if let Some(loc) = &location {
        set("pos", &JsValue::from_f64(utf16_offset(&loc.text, loc.pos) as f64));
    }

    // `cause` is the object of the first undefined operand (or tuple item), already converted
    if undef.cause().is_some() {
        let cause = undef.operands().into_iter().find_map(|(name, value)| {
            let operand = Reflect::get(&obj, &JsValue::from_str(name)).ok()?;
            match value {
                Value::Undefined(_) => Some(operand),
                Value::Tuple(items) => items
                    .iter()
                    .position(|item| matches!(item, Value::Undefined(_)))
                    .and_then(|i| Reflect::get_u32(&operand, i as u32).ok()),
                _ => None,
            }
        });
        if let Some(cause) = cause {
            set("cause", &cause);
        }
    }

    let (key, source_getter) = with_registry(|reg| {
        let key = reg.next;
        reg.next = reg.next.wrapping_add(1);
        reg.undefs.insert(key, undef.clone());
        let key = JsValue::from_f64(key as f64);
        reg.keys.set(&obj, &key);
        reg.finalizer.register(&obj, &key);
        (key, reg.source_getter.clone())
    });
    if location.is_some() {
        let descriptor = Object::new();
        let _ = Reflect::set(&descriptor, &JsValue::from_str("get"), &source_getter.bind1(&JsValue::NULL, &key));
        let _ = Reflect::set(&descriptor, &JsValue::from_str("enumerable"), &JsValue::TRUE);
        Object::define_property(&obj, &JsValue::from_str("source"), &descriptor);
    }
    Object::freeze(&obj);
    obj.into()
}

/// The undefined value of a `TexlUndefined` object made by [`undefined_to_js`], if `val` is one.
pub fn js_to_undefined(val: &JsValue) -> Option<Undef> {
    if !val.is_object() {
        return None;
    }
    with_registry(|reg| {
        let key = reg.keys.get(val.unchecked_ref::<Object>()).as_f64()?;
        reg.undefs.get(&(key as u32)).cloned()
    })
}
