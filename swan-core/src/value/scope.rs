use super::*;
use std::cell::Cell;


// ----------------------------------------------------------------------------
//  Scope  (evaluation environment — NOT a first-class Swan value)
//
//  A `Scope` is the lexical environment used during evaluation: a parent link
//  and a *live namespace*, the table of the names bound in this scope. Scopes
//  form a parent-chained singly-linked list, one per lexical block. They are
//  reference-counted so that closures can cheaply capture the current
//  environment.
//
//  This type is intentionally separate from `Value::Namespace`. A Scope is an
//  internal evaluator concept; a Namespace is a first-class, immutable Swan
//  value. `Scope::namespace` takes a snapshot of the live namespace (cheap:
//  the map is shared until the scope binds a name again).
// ----------------------------------------------------------------------------

/// A lexical environment: a live namespace of bindings with an optional parent.
///
/// Name lookup ([`Scope::get`]) searches the scope itself and then its
/// ancestors. New bindings ([`Scope::set`]) always go into the scope itself.
/// [`Scope::namespace`] returns the bindings of the scope as a [`Namespace`]
/// value, which later bindings do not change.
///
/// `Scope` is a cheap handle: clones share the same bindings.
///
/// # Examples
///
/// ```
/// use swan_core::value::{Scope, Value};
///
/// let parent = Scope::new();
/// parent.set("x", Value::Numb(1.0));
///
/// let child = parent.new_child();
/// child.set("y", Value::Numb(2.0));
///
/// assert_eq!(child.get("x"), Value::Numb(1.0));   // inherited
/// assert_eq!(parent.get("y"), Value::Nothing);    // not visible upwards
///
/// let snapshot = child.namespace();
/// child.set("z", Value::Numb(3.0));
/// assert_eq!(snapshot.get("z"), Value::Nothing);  // namespaces are immutable
/// ```
#[derive(Clone, Debug)]
pub struct Scope(pub Rc<RefCell<ScopeData>>);

/// A weak handle to a [`Scope`], made with [`Scope::downgrade`].
///
/// It does not keep the scope alive, so it can point back to a scope from a
/// value stored in that scope (or in one of its ancestors) without creating an
/// `Rc` cycle.
///
/// # Examples
///
/// ```
/// use swan_core::value::Scope;
///
/// let scope = Scope::new();
/// let weak = scope.downgrade();
/// assert!(weak.upgrade().is_some());
/// drop(scope);
/// assert!(weak.upgrade().is_none());
/// ```
#[derive(Clone, Debug)]
pub struct WeakScope(std::rc::Weak<RefCell<ScopeData>>);

impl WeakScope {
    /// Returns the scope, or `None` if it has been dropped.
    pub fn upgrade(&self) -> Option<Scope> {
        self.0.upgrade().map(Scope)
    }
}

/// The shared state behind a [`Scope`].
pub struct ScopeData {
    /// The enclosing scope, searched when a name is not bound here.
    pub parent: Option<Scope>,
    /// The live namespace: the bindings defined in this scope (not in its ancestors).
    pub bindings: Namespace,
    /// Whether Swan code is forbidden to bind names here (see [`Scope::freeze`]).
    pub frozen: bool,
}

thread_local! {
    /// The number of `ScopeData` alive on this thread (see [`Scope::live_count`]).
    static LIVE_SCOPES: Cell<usize> = const { Cell::new(0) };
}

impl ScopeData {
    fn new(parent: Option<Scope>, bindings: Namespace, frozen: bool) -> Self {
        LIVE_SCOPES.with(|n| n.set(n.get() + 1));
        Self { parent, bindings, frozen }
    }
}

impl Clone for ScopeData {
    fn clone(&self) -> Self {
        Self::new(self.parent.clone(), self.bindings.clone(), self.frozen)
    }
}

impl Drop for ScopeData {
    fn drop(&mut self) {
        // `try_with`: the counter may already be gone when thread-locals are destroyed
        let _ = LIVE_SCOPES.try_with(|n| n.set(n.get() - 1));
    }
}

impl Scope {
    /// Create a top-level scope with no parent.
    pub fn new() -> Self {
        Self(Rc::new(RefCell::new(ScopeData::new(None, Namespace::new(), false))))
    }

    /// Create a child scope that inherits from a parent.
    pub fn with_parent(parent: Scope) -> Self {
        Self(Rc::new(RefCell::new(ScopeData::new(Some(parent), Namespace::new(), false))))
    }

    /// Returns the number of scopes alive on the current thread.
    ///
    /// It is a diagnostic for memory leaks: a host can compare it before and
    /// after a piece of work. It includes the built-ins scope, which lives as
    /// long as the thread.
    ///
    /// # Examples
    ///
    /// ```
    /// use swan_core::value::Scope;
    ///
    /// let before = Scope::live_count();
    /// let scope = Scope::new();
    /// assert_eq!(Scope::live_count(), before + 1);
    /// drop(scope);
    /// assert_eq!(Scope::live_count(), before);
    /// ```
    pub fn live_count() -> usize {
        LIVE_SCOPES.with(|n| n.get())
    }

    /// Creates a new child scope using `self` as the parent environment.
    pub fn new_child(&self) -> Self {
        Self::with_parent(self.clone())
    }

    /// Creates a frozen child scope whose bindings are the members of `ns`.
    ///
    /// This is the scope of the subcontexting operation `ns.expr`: names are
    /// looked up in `ns` first, then in `self` and its ancestors. Swan code
    /// cannot bind names in it (see [`Scope::freeze`]).
    ///
    /// # Examples
    ///
    /// ```
    /// use swan_core::value::{Namespace, Scope, Value};
    ///
    /// let scope = Scope::new();
    /// scope.set("y", Value::Numb(2.0));
    /// let ns: Namespace = [("x", Value::Numb(1.0))].into_iter().collect();
    ///
    /// let sub = scope.subcontext(ns);
    /// assert_eq!(sub.get("x"), Value::Numb(1.0));
    /// assert_eq!(sub.get("y"), Value::Numb(2.0));
    /// assert!(sub.is_frozen());
    /// ```
    pub fn subcontext(&self, ns: Namespace) -> Self {
        Self(Rc::new(RefCell::new(ScopeData::new(Some(self.clone()), ns, true))))
    }

    /// Returns the parent scope, or `None` for a top-level scope.
    pub fn parent(&self) -> Option<Scope> {
        self.0.borrow().parent.clone()
    }

    /// Returns the bindings of this scope (not of its ancestors) as a namespace.
    ///
    /// The namespace is a snapshot: names bound in the scope afterwards are not
    /// in it. It is cheap, since the scope and the namespace share their map
    /// until the scope binds a name again.
    pub fn namespace(&self) -> Namespace {
        self.0.borrow().bindings.clone()
    }

    /// Freezes the scope: Swan code can no longer bind names in it.
    ///
    /// An assignment (`=`) or a labelling (`:`) evaluated in a frozen scope binds
    /// nothing and gives `Undefined`, for example `text.(size = 0)`. The host can
    /// still change the scope with [`Scope::set`], and the children of a frozen
    /// scope are not frozen.
    ///
    /// # Examples
    ///
    /// ```
    /// use swan_core::evaluator::eval;
    /// use swan_core::lexer::Lexer;
    /// use swan_core::parser::Parser;
    /// use swan_core::value::{Scope, Value};
    ///
    /// let scope = Scope::new();
    /// scope.freeze();
    /// let ast = Parser::new(Lexer::new("x = 1")).parse();
    /// assert!(matches!(eval(&ast, &scope), Value::Undefined(_)));
    /// assert_eq!(scope.get("x"), Value::Nothing);
    /// ```
    pub fn freeze(&self) {
        self.0.borrow_mut().frozen = true;
    }

    /// Whether the scope is frozen (see [`Scope::freeze`]).
    pub fn is_frozen(&self) -> bool {
        self.0.borrow().frozen
    }

    /// Returns a [`WeakScope`] that points to this scope without keeping it alive.
    pub fn downgrade(&self) -> WeakScope {
        WeakScope(Rc::downgrade(&self.0))
    }

    /// Returns the value bound to `key` in this scope or its nearest ancestor
    /// that binds it, or [`Value::Nothing`] if no scope binds it.
    ///
    /// The borrow on `self` is released *before* recursing into the parent so
    /// that there is no live `RefCell` borrow held across the recursive call.
    /// This prevents a runtime panic in the (unlikely but possible) event that
    /// the same `Scope` node appears more than once in the chain.
    pub fn get(&self, key: &str) -> Value {
        // Release the borrow before any recursive call.
        let (found, parent) = {
            let inner = self.0.borrow();
            let found = inner.bindings.contains(key).then(|| inner.bindings.get(key));
            let parent = inner.parent.clone();
            (found, parent)
        };

        found.unwrap_or_else(|| {
            parent
                .map(|p| p.get(key))
                .unwrap_or(Value::Nothing)
        })
    }

    /// Returns the value bound to `key` in the *outermost* scope of the chain
    /// that binds it, or [`Value::Nothing`] if no scope binds it.
    ///
    /// This is the opposite search order of [`Scope::get`]. It implements the
    /// Swan lifting operator `$name`.
    pub fn lift(&self, key: &str) -> Value {
        // Build a vector of scopes starting from the root down to `self`
        let mut chain = Vec::new();
        let mut current = Some(self.clone());

        while let Some(scope) = current {
            chain.push(scope.clone());
            let inner = scope.0.borrow();
            current = inner.parent.clone();
        }

        // Search top-down (root is at the back of `chain`)
        for scope in chain.iter().rev() {
            let inner = scope.0.borrow();
            if inner.bindings.contains(key) {
                return inner.bindings.get(key);
            }
        }

        Value::Nothing
    }

    /// Inserts or updates a binding in the current local scope only.
    ///
    /// Namespaces taken from the scope before ([`Scope::namespace`]) do not change.
    pub fn set(&self, key: impl Into<String>, val: Value) {
        self.0.borrow_mut().bindings.insert(key, val);
    }
}

impl PartialEq for ScopeData {

    fn eq(&self, other: &Self) -> bool {

        // Compare bindings by value
        if self.bindings != other.bindings {
            return false;
        }

        // Check if both parents exist or both are None
        match (&self.parent, &other.parent) {
            (None, None) => true,
            (Some(p1), Some(p2)) => Rc::ptr_eq(&p1.0, &p2.0), // Same parent pointer instance
            _ => false,
        }
    }
}

impl fmt::Debug for ScopeData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Print only the binding names: values may be Funcs whose closures
        // point back to this very scope, which would recurse forever.
        f.debug_struct("ScopeData")
            .field("bindings", &self.bindings.names())
            .field("has_parent", &self.parent.is_some())
            .field("frozen", &self.frozen)
            .finish()
    }
}

impl PartialEq for Scope {
    fn eq(&self, other: &Self) -> bool {
        // Optional performance shortcut: check if both Scopes share the exact same ScopeData instance
        if Rc::ptr_eq(&self.0, &other.0) {
            return true;
        }

        // Compare the inner ScopeData fields
        self.0.borrow().eq(&other.0.borrow())
    }
}

impl Eq for Scope {}







#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scope_new_and_set_get() {
        let scope = Scope::new();

        // Getting an unbound key returns Nothing
        assert_eq!(scope.get("a"), Value::Nothing);

        // Setting a binding and retrieving it
        scope.set("a", Value::Numb(42.0));
        assert_eq!(scope.get("a"), Value::Numb(42.0));
    }

    #[test]
    fn test_scope_with_parent() {
        let parent = Scope::new();
        parent.set("x", Value::Numb(10.0));

        let child = Scope::with_parent(parent);
        
        // Child can look up variables from parent
        assert_eq!(child.get("x"), Value::Numb(10.0));

        // Unbound key in both child and parent resolves to Nothing
        assert_eq!(child.get("y"), Value::Nothing);
    }

    #[test]
    fn test_scope_new_child() {
        let parent = Scope::new();
        parent.set("greeting", Value::Text("hello".into()));

        let child = parent.new_child();

        // Child inherits parent binding
        assert_eq!(child.get("greeting"), Value::Text("hello".into()));

        // Local set in child does not pollute parent
        child.set("local_var", Value::Bool(true));
        assert_eq!(child.get("local_var"), Value::Bool(true));
        assert_eq!(parent.get("local_var"), Value::Nothing);
    }

    #[test]
    fn test_scope_shadowing() {
        let parent = Scope::new();
        parent.set("x", Value::Numb(1.0));

        let child = parent.new_child();
        // Shadow 'x' in the child scope
        child.set("x", Value::Numb(2.0));

        // Child sees shadowed value, parent retains original value
        assert_eq!(child.get("x"), Value::Numb(2.0));
        assert_eq!(parent.get("x"), Value::Numb(1.0));
    }

    #[test]
    fn test_parent_mutation_visible_to_child() {
        let scope1 = Scope::new();
        let scope2 = scope1.new_child();

        // Modify parent AFTER child creation
        scope1.set("x", Value::Numb(10.0));

        // Child reflects post-creation modifications in parent
        assert_eq!(scope2.get("x"), Value::Numb(10.0));
    }

    #[test]
    fn test_deep_scope_chain() {
        let grand_parent = Scope::new();
        grand_parent.set("a", Value::Numb(1.0));

        let parent = grand_parent.new_child();
        parent.set("b", Value::Numb(2.0));

        let child = parent.new_child();
        child.set("c", Value::Numb(3.0));

        // Child can resolve up the entire chain
        assert_eq!(child.get("a"), Value::Numb(1.0));
        assert_eq!(child.get("b"), Value::Numb(2.0));
        assert_eq!(child.get("c"), Value::Numb(3.0));

        // Parent cannot see child bindings
        assert_eq!(parent.get("c"), Value::Nothing);
        assert_eq!(grand_parent.get("b"), Value::Nothing);
    }

    #[test]
    fn test_scope_equality_same_instance() {
        let scope = Scope::new();
        scope.set("a", Value::Numb(1.0));

        // A scope is equal to its clone (pointer identity shortcut)
        let clone = scope.clone();
        assert_eq!(scope, clone);
    }

    #[test]
    fn test_scope_equality_top_level() {
        let scope1 = Scope::new();
        scope1.set("a", Value::Numb(1.0));

        let scope2 = Scope::new();
        scope2.set("a", Value::Numb(1.0));

        // Both are top-level (parent: None) with equal bindings
        assert_eq!(scope1, scope2);

        // Modifying one breaks equality
        scope2.set("b", Value::Numb(2.0));
        assert_ne!(scope1, scope2);
    }

    #[test]
    fn test_scope_equality_shared_parent_instance() {
        let parent = Scope::new();
        parent.set("shared", Value::Bool(true));

        let child1 = parent.new_child();
        child1.set("x", Value::Numb(10.0));

        let child2 = parent.new_child();
        child2.set("x", Value::Numb(10.0));

        // Both children share the EXACT same parent reference and have equal bindings
        assert_eq!(child1, child2);
    }

    #[test]
    fn test_scope_equality_different_parent_instances() {
        // Parent 1 and Parent 2 have identical bindings, but are DIFFERENT memory instances
        let parent1 = Scope::new();
        parent1.set("shared", Value::Bool(true));

        let parent2 = Scope::new();
        parent2.set("shared", Value::Bool(true));

        let child1 = parent1.new_child();
        child1.set("x", Value::Numb(10.0));

        let child2 = parent2.new_child();
        child2.set("x", Value::Numb(10.0));

        // Must NOT be equal because parents are different instances (Rc::ptr_eq fails)
        assert_ne!(child1, child2);
    }

    #[test]
    fn test_scope_lift() {
        let root = Scope::new();
        root.set("x", Value::Numb(1.0));
        root.set("y", Value::Text("root".into()));

        let parent = root.new_child();
        parent.set("x", Value::Numb(2.0));
        parent.set("z", Value::Bool(true));

        let child = parent.new_child();
        child.set("x", Value::Numb(3.0));

        // 1. `x` is defined at root, parent, and child. 
        //    `lift` should return the root's value (1.0), unlike `get` which returns 3.0.
        assert_eq!(child.lift("x"), Value::Numb(1.0));
        assert_eq!(child.get("x"), Value::Numb(3.0));

        // 2. `y` is only defined at root.
        assert_eq!(child.lift("y"), Value::Text("root".into()));

        // 3. `z` is defined in parent and child chain, but not root.
        //    `lift` finds the top-most occurrence down the chain (the parent's value).
        assert_eq!(child.lift("z"), Value::Bool(true));

        // 4. Looking up a non-existent binding returns `Value::Nothing`.
        assert_eq!(child.lift("unbound"), Value::Nothing);
    }
}
