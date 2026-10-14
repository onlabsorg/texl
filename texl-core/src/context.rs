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

use swan_core::builtins::root_scope;
use swan_core::value::{BuiltinFn, Scope, Value};
use crate::modules;


/// Constructs the `texl` built-in namespace as a child scope of `parent`.
///
/// The returned [`Value::Namespace`] currently contains:
///
/// - `import`: a builtin `texl.import(uri)` that takes a [`Value::Text`] URI
///   and loads the module through [`modules::import`] using `registry`.
///   Imports are resolved against `parent` (captured by the closure). Any
///   non-text argument yields an undefined value
///   (`Value::undef_unary("texl.import", arg, 0)`).
///
/// # Arguments
///
/// * `parent`   - Scope the namespace is parented to and that `import` uses.
/// * `registry` - Module registry used to resolve imported URIs.
fn create_texl_namespace(parent: &Scope, registry: modules::Registry) -> Value {
    let texl_scope = parent.new_child();

    // Bind modules::import to texl.import
    let parent_scope = parent.clone();
    texl_scope.set(
        "import",
        Value::Builtin(BuiltinFn::new("texl.import", move |arg| {
            match arg {
                Value::Text(uri) => modules::import(registry.clone(), &uri, &parent_scope),
                other => Value::undef_unary("texl.import", other, 0),
            }
        })),
    );

    Value::Namespace(texl_scope)
}

/// Creates a standalone `texl` namespace bound to a specific document scope.
///
/// Unlike [`create_texl_namespace`], the namespace scope has no parent (it is
/// created with `Scope::new()`), so it exposes only the members set explicitly
/// here. Its `import` builtin passes `parent_scope` to [`modules::import`],
/// so that `texl.import` called from within a document is resolved relative to
/// that document's own scope.
///
/// This is used both for the caller-facing scope created by [`create`] and for
/// each imported document.
///
/// # Arguments
///
/// * `parent_scope` - The document scope that `import` resolves against.
/// * `registry`     - Module registry used to resolve imported URIs.
///
/// # Note
///
/// Keep the members of this namespace in sync with
/// [`create_texl_namespace`].
pub fn create_import_texl_namespace(parent_scope: &Scope, registry: modules::Registry) -> Value {
    let parent_scope = parent_scope.clone();
    let texl_ns = Scope::new();
    // Any other members of the `texl` namespace (see `context::create_texl_namespace`) must also be set here.
    texl_ns.set(
        "import",
        Value::Builtin(BuiltinFn::new("texl.import", move |arg| {
            match arg {
                Value::Text(uri) => modules::import(registry.clone(), &uri, &parent_scope),
                other => Value::undef_unary("texl.import", other, 0),
            }
        })),
    );
    Value::Namespace(texl_ns)
}


/// Creates a new Texl evaluation scope.
///
/// The returned scope is a child of a root scope that holds all standard Swan
/// built-ins and a `texl` namespace. The child has its own `texl` namespace
/// (see [`create_import_texl_namespace`]) that shadows the root one, so
/// `texl.import` resolves relative to the returned scope. Evaluating or
/// binding names in the returned scope does not modify the built-in level.
///
/// # Arguments
///
/// * `registry` - Module registry used by `texl.import` to resolve URIs.
///
/// # Returns
///
/// A fresh [`Scope`] ready for evaluating Texl code or for host-side
/// `get`/`set` of variables.
pub fn create(registry: modules::Registry) -> Scope {
    // 1. Obtain the base swan root scope pre-populated with standard built-ins (math, text, time, etc.)
    let scope = root_scope();

    // 2. Attach the `texl` namespace to the scope
    scope.set("texl", create_texl_namespace(&scope, registry.clone()));

    // 3. Return a child scope so host/user evaluations won't pollute the built-in level bindings
    let child_scope = scope.new_child();

    // 4. Ensure `texl.import` within a document imports relative to the document's own scope
    // This creates a new `texl` namespace for each imported document.
    child_scope.set("texl", create_import_texl_namespace(&child_scope, registry));

    child_scope
}
