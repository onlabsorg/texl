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
//! The object holds no wasm memory, so it needs no `free()`. It is a handle
//! of the undefined value (see [`crate::handles`]), so it gives the original
//! value back to Swan when it is passed back (to `context.set`, or as the
//! result of a JS function). The getters are one shared function bound to its
//! key, so they do not keep the object alive.

use std::cell::OnceCell;

use js_sys::{Function, Object, Reflect};
use swan_core::value::{Undef, Value};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use texl_core::document::{locate, members, utf16_offset};

use crate::convert::swan_value_to_js;
use crate::handles;

thread_local! {
    /// Gives the source of the undefined value whose key is its argument: see [`source_of`].
    static SOURCE_GETTER: OnceCell<Function> = const { OnceCell::new() };
}

/// The shared `source` getter, unbound.
fn source_getter() -> Function {
    SOURCE_GETTER.with(|cell| {
        cell.get_or_init(|| {
            let getter = Closure::<dyn Fn(f64) -> JsValue>::new(source_of);
            let getter_fn = getter.as_ref().unchecked_ref::<Function>().clone();
            getter.forget(); // lives as long as the module
            getter_fn
        })
        .clone()
    })
}

/// The source of the undefined value with this key, `null` if it has none or
/// its object has been collected.
fn source_of(key: f64) -> JsValue {
    match handles::get(key as u32) {
        Some(Value::Undefined(undef)) => locate(&undef).map_or(JsValue::NULL, |loc| JsValue::from_str(&loc.text)),
        _ => JsValue::NULL,
    }
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

    let key = handles::insert(Value::Undefined(undef.clone()));
    handles::register(&obj, key);
    if location.is_some() {
        let descriptor = Object::new();
        let _ = Reflect::set(&descriptor, &JsValue::from_str("get"), &source_getter().bind1(&JsValue::NULL, &JsValue::from_f64(key as f64)));
        let _ = Reflect::set(&descriptor, &JsValue::from_str("enumerable"), &JsValue::TRUE);
        Object::define_property(&obj, &JsValue::from_str("source"), &descriptor);
    }
    Object::freeze(&obj);
    obj.into()
}

/// The undefined value of a `TexlUndefined` object made by [`undefined_to_js`], if `val` is one.
pub fn js_to_undefined(val: &JsValue) -> Option<Undef> {
    match handles::value_of(val)? {
        Value::Undefined(undef) => Some(undef),
        _ => None,
    }
}
