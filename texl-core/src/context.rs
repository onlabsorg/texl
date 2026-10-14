//! Evaluation context construction for Texl documents.
//!
//! A Texl context is a Swan [`Scope`] containing:
//!
//! 1. all standard Swan built-ins (`bool`, `numb`, `text`, `list`, `nspace`, ...),
//!    obtained from [`swan_core::builtins::root_scope`], and
//! 2. a Texl-specific `texl` namespace that exposes document-level
//!    functionality, currently only `texl.import(uri)`.
//!
//! The public entry point is [`create`]. The two helper functions build the
//! `texl` namespace in two flavours:
//!
//! - [`create_texl_namespace`] (private) builds the namespace that lives in the
//!   built-in (root) scope. It is a child of that scope.
//! - [`create_import_texl_namespace`] builds a standalone namespace (no parent
//!   scope) whose `import` resolves relative to a given document scope.
//!
//! # Scope layout produced by [`create`]
//!
//! ```text
//! root scope            Swan built-ins + `texl` (import bound to root scope)
//!   └─ child scope      returned to the caller; holds a `texl` that shadows
//!                       the root one (import bound to this child scope)
//! ```
//!
//! Host code and user documents evaluate in the child scope, so their bindings
//! never pollute the built-in level.
//!
//! # Adding members to `texl`
//!
//! Any new member of the `texl` namespace must be registered in **both**
//! [`create_texl_namespace`] and [`create_import_texl_namespace`]; otherwise it
//! will be missing from either the root-level or the per-document namespace.

use crate::helpers::promise_async;
use crate::modules;
use swan_core::builtins::root_scope;
use swan_core::value::{BuiltinFn, Scope, Value};

/// Helper to create the `texl.import` built-in.
///
/// Wraps the asynchronous [`modules::import`] in a Swan Promise, checks
/// the cache to prevent duplicate fetches, and returns the pending promise.
fn create_import_builtin(parent_scope: Scope, registry: modules::Registry) -> Value {
    Value::Builtin(BuiltinFn::new("texl.import", move |arg| {
        match arg {
            Value::Text(uri) => {
                let norm_url = match registry.normalize_url(&uri) {
                    Ok(url) => url,
                    Err(err) => return Value::undef_unary("import.invalid_uri", Value::Text(err.into()), 0),
                };

                // Deduplicate in-flight and already loaded requests
                if let Some(cached) = registry.get_cached(&norm_url) {
                    return cached;
                }

                let registry_clone = registry.clone();
                let uri_str = uri.to_string();
                let parent_scope_clone = parent_scope.clone();
                let norm_url_key = norm_url.clone();

                let promise = promise_async(
                    registry.spawner().as_ref(),
                    async move {
                        modules::import(registry_clone, uri_str, parent_scope_clone).await
                    },
                );

                registry.set_cached(norm_url_key, promise.clone());
                promise
            }
            other => Value::undef_unary("texl.import", other, 0),
        }
    }))
}

/// Constructs the `texl` built-in namespace as a child scope of `parent`.
fn create_texl_namespace(parent: &Scope, registry: modules::Registry) -> Value {
    let texl_scope = parent.new_child();
    texl_scope.set("import", create_import_builtin(parent.clone(), registry));
    Value::Namespace(texl_scope)
}

/// Creates a standalone `texl` namespace bound to a specific document scope.
///
/// Unlike [`create_texl_namespace`], the namespace scope has no parent (it is
/// created with `Scope::new()`), so it exposes only the members set explicitly
/// here. Its `import` builtin passes `parent_scope` to [`modules::import`],
/// so that `texl.import` called from within a document is resolved relative to
/// that document's own scope.
pub fn create_import_texl_namespace(parent_scope: &Scope, registry: modules::Registry) -> Value {
    let texl_ns = Scope::new();
    texl_ns.set("import", create_import_builtin(parent_scope.clone(), registry));
    Value::Namespace(texl_ns)
}

/// Creates a new Texl evaluation scope.
pub fn create(registry: modules::Registry) -> Scope {
    // 1. Obtain the base swan root scope pre-populated with standard built-ins (math, text, time, etc.)
    let scope = root_scope();

    // 2. Attach the `texl` namespace to the scope
    scope.set("texl", create_texl_namespace(&scope, registry.clone()));

    // 3. Return a child scope so host/user evaluations won't pollute the built-in level bindings
    let child_scope = scope.new_child();

    // 4. Ensure `texl.import` within a document imports relative to the document's own scope
    child_scope.set("texl", create_import_texl_namespace(&child_scope, registry));

    child_scope
}
