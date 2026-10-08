
use swan_core::builtins::root_scope;
use swan_core::value::{Scope, Value};

/// Constructs the `texl` built-in namespace as a child scope of `parent`.
fn create_texl_namespace(parent: &Scope) -> Value {
    let texl_scope = parent.new_child();

    // Register texl-specific functions and settings (e.g. dummy function for now)
    //texl_scope.set("dymmy", Value::Builtin(BuiltinFn::new("texl.dummy", texl_dummy)));

    Value::Namespace(texl_scope)
}


/// Creates a new Texl scope, containing all standard Swan built-ins plus
/// the `texl` namespace.
pub fn create() -> Scope {
    // 1. Obtain the base swan root scope pre-populated with standard built-ins (math, text, time, etc.)
    let scope = root_scope();

    // 2. Attach the `texl` namespace to the scope
    scope.set("texl", create_texl_namespace(&scope));

    // 3. Return a child scope so host/user evaluations won't pollute the built-in level bindings
    scope.new_child()
}
