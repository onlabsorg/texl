use wasm_bindgen::prelude::*;
use js_sys::{Object, Reflect, Array, Function};
use swan_core::value::{Scope, Value, BuiltinFn, Promise};
use texl_core::{context, template};

/// Opaque wrapper around a Swan Scope for JS
#[wasm_bindgen]
pub struct TexlContext {
    pub(crate) scope: Scope,
}

#[wasm_bindgen]
impl TexlContext {

    /// Retrieves a variable bound in the context by key
    pub fn get(&self, key: &str) -> JsValue {
        let swan_val = self.scope.get(key);
        swan_value_to_js(&swan_val)
    }

    /// Sets or updates a variable in the local context scope.
    pub fn set(&self, key: &str, value: JsValue) -> Result<(), JsValue> {
        let swan_val = js_to_swan_value(value)?;
        self.scope.set(key, swan_val);
        Ok(())
    }

    /// Returns an array of keys bound locally in the current scope.
    pub fn names(&self) -> Array {
        let arr = Array::new();
        let bindings = self.scope.0.borrow().bindings.clone();
        
        let mut keys: Vec<String> = bindings.keys().cloned().collect();
        keys.sort(); // Sort keys alphabetically for predictable output

        for key in keys {
            arr.push(&JsValue::from_str(&key));
        }
        arr
    }
}

#[wasm_bindgen]
pub fn create_context(js_obj: Option<Object>) -> Result<TexlContext, JsValue> {
    // 1. Create standard base scope containing Swan built-ins + texl namespace
    let scope = context::create();

    // 2. Hydrate scope with properties passed from JavaScript
    if let Some(obj) = js_obj {
        let keys = Object::keys(&obj);
        for i in 0..keys.length() {
            let key = keys.get(i);
            if let Some(key_str) = key.as_string() {
                let js_val = Reflect::get(&obj, &key)?;
                let swan_val = js_to_swan_value(js_val)?;
                scope.set(key_str, swan_val);
            }
        }
    }

    Ok(TexlContext { scope })
}

#[wasm_bindgen]
/// Renders a template asynchronously. Returns a JS Promise<string>; any Swan
/// promises (including wrapped JS promises / async functions) are awaited.
pub fn render(template_str: String, ctx: &TexlContext) -> js_sys::Promise {
    let scope = ctx.scope.clone();
    wasm_bindgen_futures::future_to_promise(async move {
        let out = template::render(&template_str, &scope).await;
        Ok(JsValue::from_str(&out))
    })
}

/// True for native Promises and any other "thenable" (object with a callable `then`).
fn is_thenable(val: &JsValue) -> bool {
    val.is_instance_of::<js_sys::Promise>()
        || (val.is_object()
            && Reflect::get(val, &JsValue::from_str("then"))
                .map(|t| t.is_function())
                .unwrap_or(false))
}

// ----------------------------------------------------------------------------
// JS -> Swan Conversion
// ----------------------------------------------------------------------------

fn js_to_swan_value(val: JsValue) -> Result<Value, JsValue> {
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
        let obj: Object = val.into();
        let sub_scope = Scope::new();
        let keys = Object::keys(&obj);
        for i in 0..keys.length() {
            let key = keys.get(i);
            if let Some(k_str) = key.as_string() {
                let v = Reflect::get(&obj, &key)?;
                sub_scope.set(k_str, js_to_swan_value(v)?);
            }
        }
        Ok(Value::Namespace(sub_scope))
    } else {
        Ok(Value::Nothing)
    }
}

// ----------------------------------------------------------------------------
// Swan -> JS Conversion (used when invoking JS functions from Swan)
// ----------------------------------------------------------------------------

fn swan_value_to_js(val: &Value) -> JsValue {
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
