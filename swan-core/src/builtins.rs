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
//! Every function is documented, with examples, in the built-in reference of
//! the user manual, linked from the
//! [Texl README](https://github.com/onlabsorg/texl#documentation).

// Implementation notes
//
// Purpose: this module builds the root scope and defines the built-ins that
// live directly in it (`type`, `parse`, `TRUE`, `FALSE`).
// `bind` and `label` are also defined here: they take a name (or tuple of names) and return
// a function that binds the names globally. `bind` returns Nothing, `label` returns the values.
// Every other built-in lives in a submodule that exposes `create_namespace() -> Value`.
//
// Adding a root-level function:
//   1. Write `fn my_fn(arg: Value) -> Value`.
//   2. Register it in `builtins_scope()` with `scope.set("my_fn", map_1("my_fn", my_fn))`.
//      `map_1` makes it map over tuples and pass `Nothing` through.
//
// Adding a function to a namespace: put it in the matching submodule (`numb`,
// `text`, `time`, …) and register it in that module's `create_namespace` with
// `ns.insert("my_fn", ...)`.
//
// Multi-argument functions: these are curried. Use `map_curried` from `helpers`,
// so `math.pow 10` returns a function that takes the second argument.
//

use crate::value::{BuiltinFn, Scope, Undef, Value, WeakScope};
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
/// The built-ins live in ancestors of the returned scope, which is empty, so
/// user code can shadow a built-in name without changing it. The global names
/// created by the `bind` and `label` built-ins go into the returned scope.
///
/// The built-in namespaces and constants are created once per thread and
/// shared by every root scope of that thread; only `bind` and `label`, which
/// target the returned scope, are created per call. The shared scope is
/// frozen (see [`Scope::freeze`]), and namespaces are immutable, so Swan code
/// cannot change the built-ins: `text.(size = 0)` gives `Undefined`. The host
/// can replace a built-in with [`set_builtin`]. Nothing refers back to the
/// returned scope from its ancestors, so it is freed once the last reference to
/// it (or to one of its children) is dropped.
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
    // Layer between the shared built-ins and the user scope, holding `bind` and `label`
    let binders = BUILTINS.with(Scope::new_child);

    // Fresh empty child scope where user code runs. User code defines and shadows
    // names here and cannot overwrite the built-ins stored in its ancestors.
    let user_scope = binders.new_child();

    // `bind` and `label` write into `user_scope`, so they are global from the point
    // of view of user code, even when called from inside a function body. They hold
    // it weakly: `user_scope` already holds them through its parent.
    set_global_binders(&binders, &user_scope);

    // Return the scope in which user code is evaluated
    user_scope
}

thread_local! {
    /// The built-in namespaces and constants, shared by every root scope of the thread.
    ///
    /// The namespaces are children of this scope, which also holds them, so it is never
    /// freed; there is one per thread.
    static BUILTINS: Scope = builtins_scope();
}

/// Builds the scope of the built-in namespaces and constants.
fn builtins_scope() -> Scope {
    let scope = Scope::new();

    // Each namespace is registered under its own name, e.g. `math.sqrt`.
    scope.set("bool"   , bool_::create_namespace());
    scope.set("numb"   , numb::create_namespace());
    scope.set("text"   , text::create_namespace());
    scope.set("list"   , list::create_namespace());
    scope.set("nspace" , nspace::create_namespace());
    scope.set("promise", promise::create_namespace());
    scope.set("undef"  , undef::create_namespace());
    scope.set("math"   , math::create_namespace());
    scope.set("time"   , time::create_namespace());

    scope.set("TRUE" , Value::Bool(true));
    scope.set("FALSE", Value::Bool(false));

    scope.set("type" , map_1("type" , type_function));
    scope.set("parse", map_1("parse", parse_function));

    // Shared by every root scope of the thread: Swan code must not change it
    scope.freeze();

    scope
}

/// Binds `name` to `value` among the built-ins of the current thread.
///
/// The built-ins are shared by every root scope of the thread, so the change
/// is seen by every scope, including the scopes created before. A host uses it
/// to add a built-in, or to replace one: namespaces are immutable, so to
/// replace a member of a built-in namespace, it binds a changed copy of the
/// namespace.
///
/// # Examples
///
/// ```
/// use swan_core::builtins::{root_scope, set_builtin};
/// use swan_core::value::{BuiltinFn, Value};
///
/// let scope = root_scope();
/// let Value::Namespace(mut time) = scope.get("time") else { panic!() };
/// time.insert("now", Value::Builtin(BuiltinFn::new("time.now", |_| Value::Numb(0.0))));
/// set_builtin("time", Value::Namespace(time));
///
/// let Value::Namespace(time) = scope.get("time") else { panic!() };
/// assert_eq!(swan_core::evaluator::eval_apply(time.get("now"), Value::Nothing, 0), Value::Numb(0.0));
/// ```
pub fn set_builtin(name: &str, value: Value) {
    BUILTINS.with(|scope| scope.set(name, value));
}

/// Binds `bind` and `label` in `scope`, so that they bind names in `target`.
/// `target` is held weakly; once it is dropped, they bind nothing.
fn set_global_binders(scope: &Scope, target: &Scope) {
    // Each `move` closure needs its own clone
    let bind_target = target.downgrade();
    scope.set("bind", Value::Builtin(BuiltinFn::new(
        "bind",
        move |arg| set_global("bind", arg, bind_target.clone(), false)
    )));

    let label_target = target.downgrade();
    scope.set("label", Value::Builtin(BuiltinFn::new(
        "label",
        move |arg| set_global("label", arg, label_target.clone(), true)
    )));
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
///   - that function takes a `Namespace` and evaluates the captured AST in a
///     frozen scope holding its members, below the built-ins (so it sees the
///     members and the built-ins, and cannot bind names).
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
                    Value::Namespace(ns) => BUILTINS.with(|builtins| eval(&ast, &builtins.subcontext(ns))),
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
/// - `target_scope`: the scope where the names are bound (nothing is bound if it was dropped).
/// - `retval`: `false` for `bind` (like `=`, returns `Nothing`),
///   `true` for `label` (like `:`, returns the bound values).
///
/// Returns a builtin function that takes the values and binds the names to them
/// using `assignment_operation::eval`. That is the same function behind `=` and `:`,
/// so mismatched numbers of names and values follow the same rules as those operators.
/// If `name_arg` is not valid names, returns `Undefined` (unary operation `op`).
fn set_global(op: &'static str, name_arg: Value, target_scope: WeakScope, retval: bool) -> Value {

    // Convert Text / tuple of Text into Name / tuple of Name
    let names = wrap_names(&name_arg);
    if matches!(names, Value::Undefined(_)) {
        return Value::undef_unary(op, name_arg, 0);
    }

    Value::Builtin(BuiltinFn::new(op, move |val| {
        if let Some(target) = target_scope.upgrade() {
            assignment_operation::eval(&target, &names, &val);
        }
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

    #[test]
    fn test_root_scopes_share_the_built_ins_but_not_their_globals() {
        let (a, b) = (root_scope(), root_scope());
        let (Value::Namespace(text_a), Value::Namespace(text_b)) = (a.get("text"), b.get("text")) else { panic!() };
        assert!(text_a.ptr_eq(&text_b));

        let ast = Parser::new(Lexer::new(r#"(() -> bind "x" 1)()"#)).parse();
        eval(&ast, &a);
        assert_eq!(a.get("x"), Value::Numb(1.0));
        assert_eq!(b.get("x"), Value::Nothing);
    }

    #[test]
    fn test_built_ins_cannot_be_changed_from_swan() {
        let scope = root_scope();
        let run_in = |src: &str| eval(&Parser::new(Lexer::new(src)).parse(), &scope);

        assert!(matches!(run_in("text.(size = 0)"), Value::Undefined(Undef::BinaryOperation { op: "assignment", .. })));
        assert!(matches!(run_in("math.(pi : 3)"), Value::Undefined(Undef::BinaryOperation { op: "labelling", .. })));
        assert_eq!(run_in(r#"text.size "abc""#), Value::Numb(3.0));
        assert_eq!(run_in("TRUE"), Value::Bool(true));

        // Function calls in a subcontext bind their parameters in a child scope
        assert_eq!(run_in("text.((x -> x + 1) 1)"), Value::Numb(2.0));
        // The user scope is not frozen
        assert_eq!(run_in("size = 0, size"), Value::Numb(0.0));
    }

    #[test]
    fn test_root_scopes_are_freed() {
        let scope = root_scope();
        let ast = Parser::new(Lexer::new(r#"bind "x" 1, label "y" 2"#)).parse();
        eval(&ast, &scope);
        let binders = scope.0.borrow().parent.clone().unwrap();
        let (weak_scope, weak_binders) = (scope.downgrade(), binders.downgrade());

        drop((scope, binders));
        assert!(weak_scope.upgrade().is_none());
        assert!(weak_binders.upgrade().is_none());
    }

    #[test]
    fn test_namespaces_do_not_keep_their_scope_alive() {
        // A namespace has no parent, so storing it in the scope it was created in is no cycle
        for src in ["ns = {y: 2}", "x = 1, ns = {y: x, inner = {z: 3}}", "ns = {a: 1}, b = ns.{c: a}"] {
            let scope = root_scope();
            eval(&Parser::new(Lexer::new(src)).parse(), &scope);
            let weak = scope.downgrade();
            drop(scope);
            assert!(weak.upgrade().is_none(), "{src}");
        }
    }

    #[test]
    fn test_bind_writes_only_in_the_live_scope() {
        let scope = root_scope();
        let run_in = |src: &str| eval(&Parser::new(Lexer::new(src)).parse(), &scope);

        // Called while a namespace is being built, `bind` binds in the global scope, not in it
        run_in(r#"ns = {f = () -> bind "x" 1, y = f()}"#);
        assert_eq!(run_in("x"), Value::Numb(1.0));
        assert_eq!(run_in(r#"ns "x""#), Value::Nothing);

        // Called in a subcontext, it binds in the global scope, not in the namespace
        run_in(r#"ns.(label "z" 2)"#);
        assert_eq!(run_in("z"), Value::Numb(2.0));
        assert_eq!(run_in(r#"ns "z""#), Value::Nothing);

        // A namespace taken from the global scope does not see later bindings
        let snapshot = scope.namespace();
        run_in(r#"bind "w" 3"#);
        assert_eq!(snapshot.get("w"), Value::Nothing);
        assert_eq!(scope.namespace().get("w"), Value::Numb(3.0));
    }
}
