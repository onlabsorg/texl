//! The built-in `nspace` namespace: functions that inspect namespace values.
//!
//! | Function        | Result                                                          |
//! |-----------------|-----------------------------------------------------------------|
//! | `nspace.enum`   | The sorted names bound *locally* in a namespace, as text        |
//! | `nspace.size`   | The number of bindings defined *locally* in a namespace        |
//! | `nspace.parent` | The enclosing namespace, or `Nothing` if there is none         |
//!
//! All functions are wrapped with `map_1`, so they map over tuples, return
//! `Nothing` for `Nothing`, and return `Undefined` for non-namespace arguments.

use crate::value::{Scope, Undef, Value};
use super::helpers::*;



// ----------------------------------------------------------------------------
//  `nspace` built-in namespace
// ----------------------------------------------------------------------------

/// Builds the `nspace` namespace value.
///
/// The namespace is a child of `parent`, so builtins defined in `parent`
/// remain visible from inside it.
pub fn create_namespace(parent: &Scope) -> Value {
    let nspace_scope = parent.new_child();
    nspace_scope.set("enum"  , map_1("nspace.enum"  , nspace_enum));
    nspace_scope.set("size"  , map_1("nspace.size"  , nspace_size));
    nspace_scope.set("parent", map_1("nspace.parent", nspace_parent));
    Value::Namespace(nspace_scope)
}

/// Returns the names bound locally in a namespace (inherited names are excluded),
/// as text values sorted alphabetically (byte-wise, so uppercase sorts before lowercase).
///
/// - empty namespace -> `Nothing`
/// - one binding     -> a single `Text`
/// - several         -> a `Tuple` of `Text`
/// - non-namespace   -> `Undefined`
fn nspace_enum(val: Value) -> Value {
    match val {
        Value::Namespace(scope) => {
            let mut keys: Vec<String> = scope
                .0
                .borrow()
                .bindings
                .keys()
                .cloned()
                .collect();
            keys.sort();
            normalize_tuple(
                keys.into_iter().map(|k| Value::Text(k.into())).collect()
            )
        }
        other => undef_unary!("nspace.enum", other),
    }
}

/// Returns the number of bindings defined locally in a namespace
/// (bindings inherited from parent scopes are not counted).
/// Non-namespace values yield `Undefined`.
fn nspace_size(val: Value) -> Value {
    match val {
        Value::Namespace(scope) => {
            let count = scope.0.borrow().bindings.len();
            Value::Numb(count as f64)
        }
        other => undef_unary!("nspace.size", other),
    }
}

/// Returns the enclosing namespace of `val`, or `Nothing` if it has no parent
/// (i.e. it is the outermost scope). Non-namespace values yield `Undefined`.
fn nspace_parent(val: Value) -> Value {
    match val {
        Value::Namespace(scope) => {
            // Clone the parent handle so the RefCell borrow is released immediately.
            let parent_scope = scope.0.borrow().parent.clone();
            match parent_scope {
                Some(p) => Value::Namespace(p),
                None => Value::Nothing,
            }
        }
        other => undef_unary!("nspace.parent", other),
    }
}



// ----------------------------------------------------------------------------
//  UNIT TESTS
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;
    use crate::evaluator::eval;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn root_scope() -> Scope {
        let scope = Scope::new();
        scope.set("nspace", create_namespace(&scope));
        scope.set("TRUE"  , Value::Bool(true));
        scope.set("FALSE" , Value::Bool(false));
        scope.new_child()
    }

    fn run(src: &str) -> Value {
        let scope = root_scope();
        let mut parser = Parser::new(Lexer::new(src));
        let ast = parser.parse();
        eval(&ast, &scope)
    }

    fn is_undefined(v: &Value) -> bool {
        matches!(v, Value::Undefined(_))
    }

    fn text(s: &str) -> Value {
        Value::Text(s.into())
    }

    #[test]
    fn test_size() {
        assert_eq!(run("nspace.size {a: 1, b: 2}"), Value::Numb(2.0));
        assert_eq!(run("nspace.size()"), Value::Nothing);
        assert_eq!(
            run("nspace.size({a:1, b:2}, {c:3, d:4, e:5})"), 
            Value::Tuple(Rc::new(vec![Value::Numb(2.0), Value::Numb(3.0)]))
        );
        assert_eq!(run("nspace.size {}"), Value::Numb(0.0));
        assert!(is_undefined(&run("nspace.size 42")));
        // The builtin namespace holds exactly: enum, parent, size
        assert_eq!(run("nspace.size nspace"), Value::Numb(3.0));
        // Mixed tuple: the non-namespace element becomes Undefined, the rest still map
        match run("nspace.size(1, {a:1})") {
            Value::Tuple(items) => {
                assert!(is_undefined(&items[0]));
                assert_eq!(items[1], Value::Numb(1.0));
            }
            other => panic!("expected tuple, got {:?}", other),
        }
    }

    #[test]
    fn test_enum() {
        assert_eq!(
            run("nspace.enum {a: 1, b: 2}"),
            Value::Tuple(Rc::new(vec![Value::Text("a".into()), Value::Text("b".into())]))
        );
        assert_eq!(run("nspace.enum()"), Value::Nothing);
        assert_eq!(
            run("nspace.enum({a:1, b:2}, {c:3})"),
            Value::Tuple(Rc::new(vec![
                    Value::Text("a".into()), 
                    Value::Text("b".into()),
                    Value::Text("c".into()),
            ]))
        );
        // empty -> Nothing; single key -> plain Text, not a 1-tuple
        assert_eq!(run("nspace.enum {}"), Value::Nothing);
        assert_eq!(run("nspace.enum {a: 1}"), text("a"));
        // keys are sorted regardless of definition order
        assert_eq!(
            run("nspace.enum {b: 1, a: 2}"),
            Value::Tuple(Rc::new(vec![text("a"), text("b")]))
        );
        // non-namespace -> Undefined
        assert!(is_undefined(&run("nspace.enum 42")));
        // only local names are listed, not inherited ones
        assert_eq!(run("nspace.enum {}"), Value::Nothing);
        // the builtin namespace lists its own functions, sorted
        assert_eq!(
            run("nspace.enum nspace"),
            Value::Tuple(Rc::new(vec![text("enum"), text("parent"), text("size")]))
        );
    }

    #[test]
    fn test_parent() {

        // 1. Parent of a child namespace literal { ... } created inside another scope
        assert_eq!(
            run("ns = {a = 1}, child = ns.{b = 2}, nspace.parent child == ns"),
            Value::Bool(true)
        );

        // 2. Parent on non-namespace yields Undefined
        assert!(matches!(run("nspace.parent 42"), Value::Undefined(_)));

        // 3. The parent of the root scope should be null
        assert_eq!(
            run("nspace.parent(nspace.parent(nspace.parent{}))"),
            Value::Nothing
        );
        // nspace.parent nspace is the scope holding nspace/TRUE/FALSE
        // (uppercase names sort before lowercase)
        assert_eq!(
            run("nspace.enum(nspace.parent nspace)"),
            Value::Tuple(Rc::new(vec![text("FALSE"), text("TRUE"), text("nspace")]))
        );
        // Nothing in -> Nothing out
        assert_eq!(run("nspace.parent()"), Value::Nothing);

        // Tuples
        assert_eq!(
            run("(a,b) = nspace.parent(14, nspace.{}), a, b == nspace"), 
            Value::Tuple(Rc::new(vec![
                    undef_unary!("nspace.parent", Value::Numb(14.0)),
                    Value::Bool(true)
            ]))
        );
    }

    #[test]
    fn test_direct_size_and_enum() {
        let scope = Scope::new();
        assert_eq!(nspace_size(Value::Namespace(scope.clone())), Value::Numb(0.0));
        assert_eq!(nspace_enum(Value::Namespace(scope.clone())), Value::Nothing);

        scope.set("y", Value::Numb(1.0));
        scope.set("x", Value::Numb(2.0));
        assert_eq!(nspace_size(Value::Namespace(scope.clone())), Value::Numb(2.0));
        assert_eq!(
            nspace_enum(Value::Namespace(scope)),
            Value::Tuple(Rc::new(vec![text("x"), text("y")]))
        );
    }

    #[test]
    fn test_direct_locality() {
        // Inherited bindings must not be counted or listed
        let parent = Scope::new();
        parent.set("p", Value::Numb(1.0));
        let child = parent.new_child();
        child.set("c", Value::Numb(2.0));
        assert_eq!(nspace_size(Value::Namespace(child.clone())), Value::Numb(1.0));
        assert_eq!(nspace_enum(Value::Namespace(child)), text("c"));
    }

    #[test]
    fn test_direct_parent() {
        let parent = Scope::new();
        let child = parent.new_child();
        assert_eq!(
            nspace_parent(Value::Namespace(child)),
            Value::Namespace(parent.clone())
        );
        // A top-level scope has no parent
        assert_eq!(nspace_parent(Value::Namespace(parent)), Value::Nothing);
    }

    #[test]
    fn test_non_namespace_arguments_are_undefined() {
        for v in [Value::Numb(1.0), Value::Bool(true), text("s")] {
            assert!(is_undefined(&nspace_size(v.clone())));
            assert!(is_undefined(&nspace_enum(v.clone())));
            assert!(is_undefined(&nspace_parent(v)));
        }
    }
}



