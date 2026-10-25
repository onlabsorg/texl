//! The built-in `nspace` namespace: functions that inspect namespace values.
//!
//! | Function        | Result                                         |
//! |-----------------|------------------------------------------------|
//! | `nspace.enum`   | The sorted names of the members, as text       |
//! | `nspace.size`   | The number of members                          |
//!
//! All functions are wrapped with `map_1`, so they map over tuples, return
//! `Nothing` for `Nothing`, and return `Undefined` for non-namespace arguments.

use crate::value::{Namespace, Value};
use super::helpers::*;



// ----------------------------------------------------------------------------
//  `nspace` built-in namespace
// ----------------------------------------------------------------------------

/// Builds the `nspace` namespace value.
pub fn create_namespace() -> Value {
    let mut ns = Namespace::new();
    ns.insert("enum", map_1("nspace.enum", nspace_enum));
    ns.insert("size", map_1("nspace.size", nspace_size));
    Value::Namespace(ns)
}

/// Returns the names of the members of a namespace, as text values sorted
/// alphabetically (byte-wise, so uppercase sorts before lowercase).
///
/// - empty namespace -> `Nothing`
/// - one binding     -> a single `Text`
/// - several         -> a `Tuple` of `Text`
/// - non-namespace   -> `Undefined`
fn nspace_enum(val: Value) -> Value {
    match val {
        Value::Namespace(ns) => Value::tuple(
            ns.names().into_iter().map(|k| Value::Text(k.into())).collect()
        ),
        other => Value::undef_unary("nspace.enum", other, 0),
    }
}

/// Returns the number of members of a namespace.
/// Non-namespace values yield `Undefined`.
fn nspace_size(val: Value) -> Value {
    match val {
        Value::Namespace(ns) => Value::Numb(ns.len() as f64),
        other => Value::undef_unary("nspace.size", other, 0),
    }
}



// ----------------------------------------------------------------------------
//  UNIT TESTS
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Scope;
    use crate::evaluator::eval;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn root_scope() -> Scope {
        let scope = Scope::new();
        scope.set("nspace", create_namespace());
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
            Value::tuple(vec![Value::Numb(2.0), Value::Numb(3.0)])
        );
        assert_eq!(run("nspace.size {}"), Value::Numb(0.0));
        assert!(is_undefined(&run("nspace.size 42")));
        // The builtin namespace holds exactly: enum, size
        assert_eq!(run("nspace.size nspace"), Value::Numb(2.0));
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
            Value::tuple(vec![Value::Text("a".into()), Value::Text("b".into())])
        );
        assert_eq!(run("nspace.enum()"), Value::Nothing);
        assert_eq!(
            run("nspace.enum({a:1, b:2}, {c:3})"),
            Value::tuple(vec![
                    Value::Text("a".into()), 
                    Value::Text("b".into()),
                    Value::Text("c".into()),
            ])
        );
        // empty -> Nothing; single key -> plain Text, not a 1-tuple
        assert_eq!(run("nspace.enum {}"), Value::Nothing);
        assert_eq!(run("nspace.enum {a: 1}"), text("a"));
        // keys are sorted regardless of definition order
        assert_eq!(
            run("nspace.enum {b: 1, a: 2}"),
            Value::tuple(vec![text("a"), text("b")])
        );
        // non-namespace -> Undefined
        assert!(is_undefined(&run("nspace.enum 42")));
        // only members are listed, not the names of the enclosing scope
        assert_eq!(run("nspace.enum {}"), Value::Nothing);
        assert_eq!(run("x = 1, nspace.enum {y: x}"), text("y"));
        // the builtin namespace lists its own functions, sorted
        assert_eq!(
            run("nspace.enum nspace"),
            Value::tuple(vec![text("enum"), text("size")])
        );
    }

    #[test]
    fn test_direct_size_and_enum() {
        let mut ns = Namespace::new();
        assert_eq!(nspace_size(Value::Namespace(ns.clone())), Value::Numb(0.0));
        assert_eq!(nspace_enum(Value::Namespace(ns.clone())), Value::Nothing);

        ns.insert("y", Value::Numb(1.0));
        ns.insert("x", Value::Numb(2.0));
        assert_eq!(nspace_size(Value::Namespace(ns.clone())), Value::Numb(2.0));
        assert_eq!(
            nspace_enum(Value::Namespace(ns)),
            Value::tuple(vec![text("x"), text("y")])
        );
    }

    #[test]
    fn test_non_namespace_arguments_are_undefined() {
        for v in [Value::Numb(1.0), Value::Bool(true), text("s")] {
            assert!(is_undefined(&nspace_size(v.clone())));
            assert!(is_undefined(&nspace_enum(v)));
        }
    }
}



