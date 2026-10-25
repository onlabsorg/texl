use super::*;


// ----------------------------------------------------------------------------
//  Namespace  (first-class Swan value: an immutable table of members)
//
//  A `Namespace` is a plain name → value map. It has no parent and no link to
//  the scope it was built in. Swan code cannot change it: assignments only
//  happen in a `Scope`, and a `{...}` literal takes a snapshot of its scope's
//  bindings when it is done.
//
//  The map is shared through an `Rc`. `insert` is copy-on-write, so a host can
//  build or extend a namespace without changing the other values that share
//  its map.
// ----------------------------------------------------------------------------

/// An immutable table of members: the value of a Swan namespace.
///
/// Cloning is cheap: clones share the same map. [`Namespace::insert`] is
/// copy-on-write, so it never changes the other clones; it is meant for hosts
/// that build a namespace before handing it to Swan.
///
/// # Examples
///
/// ```
/// use swan_core::value::{Namespace, Value};
///
/// let mut point = Namespace::new();
/// point.insert("x", Value::Numb(3.0));
///
/// let copy = point.clone();
/// point.insert("y", Value::Numb(4.0));
///
/// assert_eq!(point.get("y"), Value::Numb(4.0));
/// assert_eq!(copy.get("y"), Value::Nothing);      // the clone is unchanged
/// ```
#[derive(Clone, Default)]
pub struct Namespace(Rc<HashMap<String, Value>>);

impl Namespace {
    /// Creates an empty namespace.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the member `name`, or [`Value::Nothing`] if there is none.
    pub fn get(&self, name: &str) -> Value {
        self.0.get(name).cloned().unwrap_or(Value::Nothing)
    }

    /// Whether the namespace has a member called `name`.
    pub fn contains(&self, name: &str) -> bool {
        self.0.contains_key(name)
    }

    /// Binds `name` to `value`, replacing any previous member with that name.
    ///
    /// The map is copied first if other clones share it, so they do not see the change.
    pub fn insert(&mut self, name: impl Into<String>, value: Value) {
        Rc::make_mut(&mut self.0).insert(name.into(), value);
    }

    /// The number of members.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether the namespace has no members.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The names of the members, sorted (byte-wise, so uppercase sorts before lowercase).
    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.0.keys().cloned().collect();
        names.sort();
        names
    }

    /// An iterator over the members, in no particular order.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Value)> {
        self.0.iter()
    }

    /// The shared map, for the cycle collector.
    pub(crate) fn map(&self) -> &Rc<HashMap<String, Value>> {
        &self.0
    }

    /// Whether `self` and `other` share the same map.
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl<K: Into<String>> FromIterator<(K, Value)> for Namespace {
    fn from_iter<I: IntoIterator<Item = (K, Value)>>(iter: I) -> Self {
        Self(Rc::new(iter.into_iter().map(|(k, v)| (k.into(), v)).collect()))
    }
}

/// Two namespaces are equal when they have the same members with equal values.
impl PartialEq for Namespace {
    fn eq(&self, other: &Self) -> bool {
        self.ptr_eq(other) || self.0 == other.0
    }
}

impl fmt::Debug for Namespace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Sorted, so that the output is stable
        let mut members: Vec<_> = self.0.iter().collect();
        members.sort_by(|a, b| a.0.cmp(b.0));
        f.debug_map().entries(members).finish()
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_and_insert() {
        let mut ns = Namespace::new();
        assert_eq!(ns.get("a"), Value::Nothing);
        assert!(ns.is_empty());

        ns.insert("a", Value::Numb(1.0));
        assert_eq!(ns.get("a"), Value::Numb(1.0));
        assert!(ns.contains("a"));
        assert_eq!(ns.len(), 1);
    }

    #[test]
    fn test_insert_is_copy_on_write() {
        let mut a = Namespace::new();
        a.insert("x", Value::Numb(1.0));
        let b = a.clone();
        assert!(a.ptr_eq(&b));

        a.insert("x", Value::Numb(2.0));
        assert!(!a.ptr_eq(&b));
        assert_eq!(a.get("x"), Value::Numb(2.0));
        assert_eq!(b.get("x"), Value::Numb(1.0));
    }

    #[test]
    fn test_equality_by_value() {
        let a: Namespace = [("x", Value::Numb(1.0)), ("y", Value::Numb(2.0))].into_iter().collect();
        let b: Namespace = [("y", Value::Numb(2.0)), ("x", Value::Numb(1.0))].into_iter().collect();
        assert_eq!(a, b);

        let c: Namespace = [("x", Value::Numb(1.0))].into_iter().collect();
        assert_ne!(a, c);
    }

    #[test]
    fn test_names_are_sorted() {
        let ns: Namespace = [("b", Value::Nothing), ("a", Value::Nothing), ("C", Value::Nothing)]
            .into_iter().collect();
        assert_eq!(ns.names(), vec!["C", "a", "b"]);
    }
}
