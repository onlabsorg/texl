//! The built-in library and the root scope.
//!
//! [`root_scope`] creates the scope in which Swan code normally runs. It
//! contains these bindings:
//!
//! | Name                  | Kind      | Description                                   |
//! |-----------------------|-----------|-----------------------------------------------|
//! | `TRUE`, `FALSE`       | constant  | The boolean values                            |
//! | `type`                | function  | The type name of a value                      |
//! | `parse`               | function  | Compiles Swan source text into a function     |
//! | `bind`, `label`       | function  | Bind names in the global scope                |
//! | `bool`                | namespace | Boolean conversion and reduction              |
//! | `numb`                | namespace | Number conversion                             |
//! | `text`                | namespace | Text inspection and manipulation              |
//! | `list`                | namespace | List inspection and manipulation              |
//! | `nspace`              | namespace | Namespace inspection                          |
//! | `math`                | namespace | Mathematical constants and functions          |
//! | `time`                | namespace | Timestamps and calendar dates                 |
//! | `promise`             | namespace | Creating and resolving promises               |
//! | `undef`               | namespace | Creating and inspecting undefined values      |
//!
//! Every function is documented, with examples, in the
//! [built-in library reference](https://onlabsorg.github.io/texl/book/swan/builtins/index.html)
//! of the Swan language manual.

// Implementation notes
//
// Purpose: this module builds the root scope and defines the built-ins that
// live directly in it (`type`, `parse`, `TRUE`, `FALSE`).
// `bind` and `label` are also defined here: they take a name (or tuple of names) and return
// a function that binds the names globally. `bind` returns Nothing, `label` returns the values.
// Every other built-in lives in a submodule that exposes `create_namespace(&Scope) -> Value`.
//
// Adding a root-level function:
//   1. Write `fn my_fn(arg: Value) -> Value`.
//   2. Register it in `root_scope()` with `scope.set("my_fn", map_1("my_fn", my_fn))`.
//      `map_1` makes it map over tuples and pass `Nothing` through.
//
// Adding a function to a namespace: put it in the matching submodule (`numb`,
// `text`, `time`, …) and register it in that module's `create_namespace`.
//
// Multi-argument functions: these are curried. Use `map_curried` from `helpers`,
// so `math.pow 10` returns a function that takes the second argument.
//

use crate::value::{BuiltinFn, Scope, Undef, Value};
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::evaluator::eval;
use crate::evaluator::assignment_operation;

// Shared helpers and macros (map_1, map_curried, ...).
mod helpers;
use helpers::*;

// One submodule per built-in namespace; each exposes create_namespace().
mod bool_;
mod numb;
mod text;
mod list;
mod nspace;
mod promise;
mod undef;
mod math;
mod time;



// ----------------------------------------------------------------------------
//  Root scope factory
// ----------------------------------------------------------------------------

/// Builds a fresh scope in which every built-in name is available.
///
/// The built-ins live in a parent scope, and the returned scope is an empty
/// child of it, so user code can shadow a built-in name without changing it.
/// The global names created by the `bind` and `label` built-ins go into the
/// returned scope.
///
/// Every evaluation session should start from a scope created here (or from a
/// child of one).
///
/// # Examples
///
/// ```
/// use swan_core::builtins::root_scope;
/// use swan_core::evaluator::eval;
/// use swan_core::lexer::Lexer;
/// use swan_core::parser::Parser;
///
/// let scope = root_scope();
/// let mut parser = Parser::new(Lexer::new("1+2"));
/// let ast = parser.parse();
/// assert_eq!(eval(&ast, &scope), swan_core::value::Value::Numb(3.0));
/// ```
pub fn root_scope() -> Scope {
    // Built-ins are installed in this base scope.
    let scope = Scope::new();

    // Each namespace is registered under its own name, e.g. `math.sqrt`.
    scope.set("bool"   , bool_::create_namespace(&scope));
    scope.set("numb"   , numb::create_namespace(&scope));
    scope.set("text"   , text::create_namespace(&scope));
    scope.set("list"   , list::create_namespace(&scope));
    scope.set("nspace" , nspace::create_namespace(&scope));
    scope.set("promise", promise::create_namespace(&scope));
    scope.set("undef"  , undef::create_namespace(&scope));
    scope.set("math"   , math::create_namespace(&scope));
    scope.set("time"   , time::create_namespace(&scope));

    scope.set("TRUE" , Value::Bool(true));
    scope.set("FALSE", Value::Bool(false));

    scope.set("type" , map_1("type" , type_function));
    scope.set("parse", map_1("parse", parse_function));

    // Fresh empty child scope where user code runs. User code defines and shadows
    // names here and cannot overwrite the built-ins stored in the parent.
    let user_scope = scope.new_child();

    // `bind` and `label` write into `user_scope`, so they are global from the point
    // of view of user code, even when called from inside a function body.
    // Each `move` closure needs its own clone.
    //
    // NOTE: this creates an Rc cycle (scope -> bind closure -> user_scope -> scope),
    // so a root scope is never freed. `Scope` has no weak reference, so this is accepted.
    let bind_target = user_scope.clone();
    scope.set("bind", Value::Builtin(BuiltinFn::new(
        "bind",
        move |arg| set_global("bind", arg, bind_target.clone(), false)
    )));

    let label_target = user_scope.clone();
    scope.set("label", Value::Builtin(BuiltinFn::new(
        "label",
        move |arg| set_global("label", arg, label_target.clone(), true)
    )));

    // Return the scope in which user code is evaluated
    user_scope
}



// ----------------------------------------------------------------------------
//  built-in functions
// ----------------------------------------------------------------------------

/// Builtin `type`: returns the type name of its argument as a `Text`.
///
/// Wrapped with `map_1`, so `type()` is `Nothing` and `type(a, b)` gives a
/// tuple of type names. Both `Func` and `Builtin` values report `"Func"`.
fn type_function(val: Value) -> Value {
    let n = match val {
        Value::Nothing => "Nothing",
        Value::Bool(_) => "Bool",
        Value::Numb(_) => "Numb",
        Value::Text(_) => "Text",
        Value::List(_) => "List",
        Value::Namespace(_) => "Namespace",
        Value::Func { .. } | Value::Builtin(_) => "Func",
        Value::Promise(_)   => "Promise",
        Value::Undefined(_) => "Undefined",
        Value::Tuple(_)     => "Tuple",
        Value::Name(_)      => "Name",
    };
    Value::Text(n.into())
}


/// Builtin `parse` function:
///   - takes a `Text` containing source code and parses it immediately.
///   - returns a builtin function named `"eval"`.
///   - that function takes a `Namespace` and evaluates the captured AST in
///     that namespace as its scope.
///   - a non-`Text` argument to `parse` gives `Undefined`, labelled `"parse"`.
///   - a non-`Namespace` argument to the returned function gives `Undefined`,
///     labelled `"eval"`.
fn parse_function(arg: Value) -> Value {
    match arg {
        Value::Text(source) => {
            let mut parser = Parser::new(Lexer::new(&source));
            let ast = parser.parse();

            // The closure takes ownership of the AST.
            Value::Builtin(BuiltinFn::new("eval", move |ns_arg| {
                match ns_arg {
                    Value::Namespace(scope) => eval(&ast, &scope),
                    other => Value::undef_unary("eval", other, 0),
                }
            }))
        }
        other => Value::undef_unary("parse", other, 0),
    }
}


/// Shared implementation of the builtins `bind` and `label`.
///
/// - `op`: `"bind"` or `"label"`, used as the label of the `Undefined` result.
/// - `name_arg`: a `Text`, a tuple of `Text`s, or `Nothing`. These are the names.
/// - `target_scope`: the scope where the names are bound.
/// - `retval`: `false` for `bind` (like `=`, returns `Nothing`),
///   `true` for `label` (like `:`, returns the bound values).
///
/// Returns a builtin function that takes the values and binds the names to them
/// using `assignment_operation::eval`. That is the same function behind `=` and `:`,
/// so mismatched numbers of names and values follow the same rules as those operators.
/// If `name_arg` is not valid names, returns `Undefined` (unary operation `op`).
fn set_global(op: &'static str, name_arg: Value, target_scope: Scope, retval: bool) -> Value {

    // Convert Text / tuple of Text into Name / tuple of Name
    let names = wrap_names(&name_arg);
    if matches!(names, Value::Undefined(_)) {
        return Value::undef_unary(op, name_arg, 0);
    }

    Value::Builtin(BuiltinFn::new(op, move |val| {
        assignment_operation::eval(&target_scope, &names, &val);
        if retval { val } else { Value::Nothing }
    }))
}

/// Converts a runtime value into a validated name or tuple of names.
/// `Text` gives a `Name`, `Nothing` stays `Nothing`, and a tuple of `Text`s gives a
/// tuple of `Name`s. Anything else gives `Undefined(Undef::Name)`.
fn wrap_names(val: &Value) -> Value {
    match val {
        Value::Nothing => Value::Nothing,
        Value::Text(s) => Value::Name(s.clone()),
        Value::Tuple(items) => {
            let mapped: Vec<Value> = items.iter().map(wrap_names).collect();
            if mapped.iter().any(|v| matches!(v, Value::Undefined(_))) {
                Value::Undefined(Undef::Name { pos: 0 })
            } else {
                Value::tuple(mapped)
            }
        }
        _ => Value::Undefined(Undef::Name { pos: 0 }),
    }
}



// ----------------------------------------------------------------------------
//  Tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Evaluates `src` in a fresh root scope.
    fn run(src: &str) -> Value {
        let scope = root_scope();
        let mut parser = Parser::new(Lexer::new(src));
        let ast = parser.parse();
        eval(&ast, &scope)
    }

    #[test]
    fn test_type() {
        // Item
        assert_eq!(run("type(1 == 1)"), Value::Text("Bool".into()));
        assert_eq!(run("type 10"), Value::Text("Numb".into()));
        assert_eq!(run(r#"type "abc""#), Value::Text("Text".into()));
        assert_eq!(run("type [1, 2]"), Value::Text("List".into()));
        assert_eq!(run("type {a: 1}"), Value::Text("Namespace".into()));
        assert_eq!(run("type(x -> x)"), Value::Text("Func".into()));
        assert_eq!(run("type type"), Value::Text("Func".into()));

        // Nothing
        assert_eq!(run("type()"), Value::Nothing);

        // Tuple
        assert_eq!(
            run(r#"type(10, "abc", [1])"#),
            Value::tuple(vec![
                Value::Text("Numb".into()),
                Value::Text("Text".into()),
                Value::Text("List".into()),
            ])
        );
    }

    #[test]
    fn test_parse() {
        // Evaluation with a scope argument
        assert_eq!(
            run(r#"eval_fn = parse "a + b", eval_fn { a = 10, b = 20 }"#),
            Value::Numb(30.0)
        );

        // Returns a function/closure
        assert_eq!(
            run(r#"double = (parse "x -> x * 2") {}, double 5"#),
            Value::Numb(10.0)
        );

        // Nothing
        assert_eq!(run("parse()"), Value::Nothing);

        // Tuple
        assert_eq!(
            run(r#"(f,g) = parse("2*x", "3*x"), f{x:10}, g{x:20}"#),
            Value::tuple(vec![
                Value::Numb(20.0),
                Value::Numb(60.0),
            ]),
        );

        // Invalid argument type returns Undefined
        assert!(matches!(run("parse 123"), Value::Undefined(_)));
    }

    #[test]
    fn test_bind() {
        // bind returns Nothing, but the name is bound
        assert_eq!(run(r#"(bind "x") 5"#), Value::Nothing);
        assert_eq!(run(r#"(bind "x") 5, x"#), Value::Numb(5.0));

        // Tuple of names
        assert_eq!(run(r#"(bind ("a","b")) (1,2), a + b"#), Value::Numb(3.0));

        // Nothing as names: values are discarded
        assert_eq!(run(r#"(bind ()) 5"#), Value::Nothing);

        // Binding is global, even when called inside a function body
        assert_eq!(run(r#"f = v -> (bind "y") v, f 7, y"#), Value::Numb(7.0));

        // Mismatched counts behave exactly like `=`
        assert_eq!(
            run(r#"(bind ("a","b")) (1,2,3), (a,b)"#),
            run("(a,b) = (1,2,3), (a,b)")
        );
        assert_eq!(
            run(r#"(bind ("a","b")) 1, (a,b)"#),
            run("(a,b) = 1, (a,b)")
        );
        assert_eq!(
            run(r#"(bind "a") (1,2), a"#),
            run("a = (1,2), a")
        );

        // Invalid names give Undefined labelled "bind"
        assert!(matches!(run("bind 5"), Value::Undefined(Undef::UnaryOperation { op: "bind", .. })));
        assert!(matches!(run(r#"bind ("a", 5)"#), Value::Undefined(Undef::UnaryOperation { op: "bind", .. })));
    }

    #[test]
    fn test_label() {
        // label returns the value and binds the name
        assert_eq!(run(r#"(label "x") 5"#), Value::Numb(5.0));
        assert_eq!(
            run(r#"(label "x") 5, x"#),
            Value::tuple(vec![Value::Numb(5.0), Value::Numb(5.0)])
        );

        // Tuple of names returns the tuple of values
        assert_eq!(
            run(r#"(label ("a","b")) (1,2)"#),
            Value::tuple(vec![Value::Numb(1.0), Value::Numb(2.0)])
        );

        // Binding is global, even when called inside a function body
        assert_eq!(
            run(r#"f = v -> (label "y") v, f 7, y"#),
            Value::tuple(vec![Value::Numb(7.0), Value::Numb(7.0)])
        );

        // Mismatched counts behave exactly like `:`
        assert_eq!(
            run(r#"(label ("a","b")) (1,2,3)"#),
            run("(a,b) : (1,2,3)")
        );
        assert_eq!(
            run(r#"(label ("a","b")) 1"#),
            run("(a,b) : 1")
        );
        assert_eq!(
            run(r#"(label "a") (1,2)"#),
            run("a = (1,2), a")
        );

        // Invalid names give Undefined labelled "label"
        assert!(matches!(run("label 5"), Value::Undefined(Undef::UnaryOperation { op: "label", .. })));
        assert!(matches!(run(r#"label ("a", 5)"#), Value::Undefined(Undef::UnaryOperation { op: "label", .. })));
    }
}
