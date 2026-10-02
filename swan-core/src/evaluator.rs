//! This module evaluates AST nodes into `Value`s.
//!
//! The `eval` function is infallible: all errors are returned as `Value::Undefined(...)`.
//! Operations that need non-trivial logic are delegated to sub-modules. This file handles
//! literals, grouping, function definition and application, mapping, labelling, subcontexting,
//! logic, conditional and composition.

use std::rc::Rc;
use crate::ast::Node;
use crate::value::{Scope, Value, Undef, BuiltinFn};


// Unary operations
mod affirm_operation;
mod negation_operation;
mod logical_negation_operation;
mod lifting_operation;

// Binary / binding operations
mod pairing_operation;
mod assignment_operation;

// Binary operations with tuple broadcasting
mod arithmetic_operations;
mod comparison_operations;



// ----------------------------------------------------------------------------
//  MACROS
// ----------------------------------------------------------------------------

/// Evaluates a binary operation with element-wise tuple broadcasting (§3.5).
///
/// If either operand is a [`Value::Tuple`], pairs the items using [`Value::pair`]
/// and applies `$eval_fn` element-wise. Otherwise, applies `$eval_fn` directly.
///
/// Requires 3 arguments: `($lhv, $rhv, $eval_fn)`.
/// The `$eval_fn` must be a callable taking `(Value, Value)`.
/// If both operands are `Nothing` the result is `Nothing`.
/// A `Nothing` against a non-tuple value is passed through to `$eval_fn` as is.
/// The result of a tuple broadcast is normalized.
///
/// Requires `Rc` and `Value` to be in scope.
macro_rules! map_pairs {
    ($lhv:expr, $rhv:expr, $eval_fn:expr) => {{
        let (lhv, rhv) = ($lhv, $rhv);
        if matches!(lhv, Value::Tuple(_)) || matches!(rhv, Value::Tuple(_)) {
            Value::Tuple(Rc::new(
                lhv.pair(&rhv)
                    .map(|(l,r)| $eval_fn(l,r))
                    .collect(),
            )).normalize()
        } else if lhv == Value::Nothing && rhv == Value::Nothing {
            Value::Nothing
        } else {
            $eval_fn(lhv, rhv)
        }
    }};
}

/// Applies `$eval_fn` (taking one `Value`) to a single operand.
///
/// `Nothing` gives `Nothing`.
/// A `Tuple` is mapped item by item (flattened) and the result is normalized.
/// Any other value is passed directly to `$eval_fn`.
macro_rules! map_values {
    ($val:expr, $eval_fn:expr) => {{
        let val = $val;
        match val {
            Value::Nothing => Value::Nothing,
            Value::Tuple(_) => {
                Value::Tuple(Rc::new(
                    val.items()
                    .map($eval_fn)
                    .collect()
                )).normalize()
            }
            _ => $eval_fn(val)
        }
    }};
}



// ----------------------------------------------------------------------------
//  PUBLIC ENTRY POINT
// ----------------------------------------------------------------------------

/// Evaluates an AST node within a given `Scope` context and returns a `Value`.
///
/// This function is **infallible**: it never panics. All error conditions are
/// encoded as `Value::Undefined(...)` so that Swan's "errors are values" semantics
/// are preserved throughout, and sub-expression errors propagate as `Undefined` values. (#14)
pub fn eval(node: &Node, scope: &Scope) -> Value {
    match node {


        // ===== Literals / Leafs =====

        // Numeric literal
        Node::Numb(leaf)       => Value::Numb(leaf.val),

        // String literal
        Node::Text(leaf)       => Value::Text(leaf.val.as_str().into()),

        // Nothing literal: `()` or empty expression
        Node::Nothing(_)       => Value::Nothing,

        // Identifier lookup
        Node::Identifier(leaf) => scope.get(&leaf.val),



        // ===== Grouping =====

        // List creation: `[X]` - the evaluated operand is flattened into the list items (Nothing is skipped)
        Node::List(unary) => {
            let val = eval(&unary.opd, scope);
            let vec: Vec<_> = val.items().collect();
            Value::List(Rc::new(vec))
        }

        // Namespace creation: `{X}` - body is evaluated for side effects (bindings), result is discarded
        Node::Namespace(unary) => {
            let child_scope = scope.new_child();
            eval(&unary.opd, &child_scope);
            Value::Namespace(child_scope)
        }



        // ===== Unary Operations =====

        // Unary affirm operation: `+X`
        Node::AffirmOp(unary) => {
            let val = eval(&unary.opd, scope);
            affirm_operation::eval(val)
        }

        // Unary negation operation: `-X`
        Node::NegateOp(unary) => {
            let val = eval(&unary.opd, scope);
            map_values!(val, |v| negation_operation::eval(v, unary.pos))
        }

        // Unary logical negation operation: `!X`
        Node::LogicalNegationOp(unary) => {
            let val = eval(&unary.opd, scope);
            map_values!(val, logical_negation_operation::eval)
        }

        // Unary lifting operation: `$X`
        Node::LiftingOp(unary) => {
            let names = eval_names(&unary.opd);
            map_values!(names, |n| lifting_operation::eval(n, scope, unary.pos))
        }



        // ===== Core Operations =====

        // Function definition: `A -> B` - LHO is resolved as names, not evaluated. Captures current scope.
        Node::FunctionDefinitionOp(bin) => {
            let names = eval_names(&bin.lho);
            if matches!(names, Value::Undefined(_)) {
                invalid_names("function_definition", names, bin.pos)
            } else {
                Value::Func{
                    params: Box::new(names),
                    body: bin.rho.clone(),
                    closure: scope.clone(),
                }
            }
        }

        // `f x` — function application.
        // A tuple of callables is broadcast through `map_values!`, with the same argument given to each function.
        Node::ApplicationOp(bin) => {
            // The left-hand operand must evaluate to a callable (Builtin or Func).
            // The right-hand operand is the argument (possibly a Tuple for multiple
            // arguments). Builtin closures receive the whole argument Value and are
            // responsible for destructuring it if needed.
            let func = eval(&bin.lho, scope);
            let args = eval(&bin.rho, scope);
            map_values!(func, |f| eval_apply(f, args.clone(), bin.pos))
        }

        // Pairing operation: `X , Y`
        Node::PairingOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            pairing_operation::eval(lhv, rhv)
        }

        // Mapping: `T => F` - outer `map_values!` iterates over args, inner over funcs. Every arg to every func.
        Node::MappingOp(bin) => {
            let args = eval(&bin.lho, scope);
            let func = eval(&bin.rho, scope);
            map_values!(args, |a: Value| map_values!(func.clone(), |f| eval_apply(f, a.clone(), bin.pos)))
        }

        // Assignment: `name = V` - evaluates to Nothing, binds names in scope
        Node::AssignmentOp(bin) => {
            let names = eval_names(&bin.lho);
            if matches!(names, Value::Undefined(_)) {
                return invalid_names("assignment", names, bin.pos);
            }

            let values = eval(&bin.rho, scope);
            assignment_operation::eval(scope, &names, &values);
            Value::Nothing
        }

        // Labelling: `name : V` - evaluates to V, binds names in scope
        Node::LabellingOp(bin) => {
            let names = eval_names(&bin.lho);
            if matches!(names, Value::Undefined(_)) {
                return invalid_names("labelling", names, bin.pos);
            }

            let values = eval(&bin.rho, scope);
            assignment_operation::eval(scope, &names, &values);
            values
        }

        // `ns.expression` — namespace sub-contexting
        Node::SubcontextingOp(bin) => {
            // The left-hand side must evaluate to a `Value::Namespace`.
            // The right-hand side may be any expression and is evaluated within the namespace's scope.
            // A non-namespace left side gives an `Undefined`.
            let nst = eval(&bin.lho, scope);
            map_values!(nst, |ns| match ns {
                Value::Namespace(ns_scope) => {
                    eval(&bin.rho, &ns_scope)
                }
                lhv => Value::Undefined(Undef::BinaryOperation {
                    op:  "subcontexting",
                    lhv: Box::new(lhv),
                    rhv: Box::new(Value::Nothing),  // rhv can't be evaluated without a scope
                    pos: bin.pos,
                }),
            })
        }



        // ===== Arithmetic Operations =====

        // Sum: `A + B`
        Node::SumOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            map_pairs!(lhv, rhv, |l,r| arithmetic_operations::eval_add(l, r, bin.pos))
        }

        // Subtraction: `A - B`
        Node::SubtractionOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            map_pairs!(lhv, rhv, |l,r| arithmetic_operations::eval_subtraction(l, r, bin.pos))
        }

        // Product: `A * B`
        Node::ProductOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            map_pairs!(lhv, rhv, |l,r| arithmetic_operations::eval_product(l, r, bin.pos))
        }

        // Division: `A / B`
        Node::DivisionOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            map_pairs!(lhv, rhv, |l,r| arithmetic_operations::eval_division(l, r, bin.pos))
        }

        // Modulo: `A % B`
        Node::ModuloOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            map_pairs!(lhv, rhv, |l,r| arithmetic_operations::eval_modulo(l, r, bin.pos))
        }

        // Exponentiation: `A ^ B`
        Node::PowerOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            map_pairs!(lhv, rhv, |l,r| arithmetic_operations::eval_power(l, r, bin.pos))
        }



        // ===== Comparison Operations =====

        // Equal: `A == B`
        Node::EqualToOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            map_pairs!(lhv, rhv, |l,r| comparison_operations::eval_equal_to(l, r))
        }

        // Non-equal: `A != B`
        Node::NotEqualToOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            map_pairs!(lhv, rhv, |l,r| comparison_operations::eval_not_equal_to(l, r))
        }

        // Greater-than: `A > B`
        Node::GreaterThanOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            map_pairs!(lhv, rhv, |l,r| comparison_operations::eval_greater_than(l, r, bin.pos))
        }

        // Non-greater-than: `A <= B`
        Node::GreaterThanOrEqualToOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            map_pairs!(lhv, rhv, |l,r| comparison_operations::eval_greater_than_or_equal_to(l, r, bin.pos))
        }

        // Less than: `A < B`
        Node::LessThanOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            map_pairs!(lhv, rhv, |l,r| comparison_operations::eval_less_than(l, r, bin.pos))
        }

        // Non-less-than: `A >= B`
        Node::LessThanOrEqualToOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            map_pairs!(lhv, rhv, |l,r| comparison_operations::eval_less_than_or_equal_to(l, r, bin.pos))
        }



        // ===== Logic Operations =====

        // And: `A & B` - both operands are evaluated eagerly. Returns `r` if `l` is truthy, else `l`.
        Node::AndOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            map_pairs!(lhv, rhv, |l: Value, r: Value| {
                if bool::from(l.clone()) { r } else { l }
            })
        }

        // Or: `A | B` - both operands are evaluated eagerly. Returns `l` if `l` is truthy, else `r`.
        Node::OrOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            map_pairs!(lhv, rhv, |l: Value, r: Value| {
                if bool::from(l.clone()) { l } else { r }
            })
        }



        // ===== Conditional operations =====

        // Conditional: `A ? B` - with tuples, falsy items are filtered out through `Nothing` and normalization.
        Node::ConditionalOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            map_pairs!(lhv, rhv, |l,r| {
                if bool::from(l) { r } else { Value::Nothing }
            })
        }

        // Alternative: `A ; B` - RHO is evaluated lazily, and only if LHO is Nothing.
        Node::AlternativeOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            match lhv {
                Value::Nothing => eval(&bin.rho, scope),
                _ => lhv
            }
        }



        // ===== Composition operations =====

        // Composition: `F << G` - apply `G` first, then `F`.
        Node::CompositionOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            map_pairs!(lhv, rhv, |l,r| eval_composition(l, r, bin.pos))
        }

        // Reverse composition: `F >> G` - apply `F` first, then `G`. (Swapped operands to eval_composition).
        Node::ReverseCompositionOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            map_pairs!(lhv, rhv, |l,r| eval_composition(r, l, bin.pos))
        }



        // ===== Syntax Errors =====

        // Missing closing bracket - turn parse errors into Undefined values
        Node::Unclosed(leaf) => {
            Value::Undefined(Undef::Termination{
                expected: leaf.val,
                pos: leaf.pos,
            })
        }

        // Undefined token - turn parse errors into Undefined values
        Node::UnexpectedToken(leaf) => {
            Value::Undefined(Undef::Token{
                token: leaf.val.clone(),
                pos: leaf.pos
            })
        }
    }
}


// EVAL HELPERS

/// Builds the `Undefined` value returned when the left-hand side of a binding
/// operation (`->`, `=`, `:`) is not a valid name or tuple of names.
fn invalid_names(op: &'static str, names: Value, pos: usize) -> Value {
    Value::Undefined(Undef::BinaryOperation {
        op,
        lhv: Box::new(names),
        rhv: Box::new(Value::Nothing), // the right-hand side is never evaluated
        pos,
    })
}

/// Packs the per-item results of applying a list/namespace to a tuple of
/// arguments. Unlike `Value::normalize`, `Nothing` results are kept so that
/// result positions line up with argument positions.
fn collect_results(items: Vec<Value>) -> Value {
    match items.len() {
        0 => Value::Nothing,
        1 => items.into_iter().next().unwrap(),
        _ => Value::Tuple(Rc::new(items)),
    }
}

/// Evaluates an AST nodes that are supposed to resolve into names
///
/// This functions resolves values or tuple at the left side of an
/// assignment operation or at the left side of a function definition.
/// Those values can be either an identifier or a tuple of identifiers.
/// It returns `Value::Name`, `Value::Nothing`, or a tuple of names,
/// and `Undefined(Undef::Name{pos})` for anything else.
fn eval_names(node: &Node) -> Value {
    match node {

        // Nothing literal: `()` or empty expression
        Node::Nothing(_) => Value::Nothing,

        // Identifier lookup
        Node::Identifier(leaf) => Value::Name(leaf.val.as_str().into()),

        // Pairing operation: `X , Y`
        Node::PairingOp(bin) => {
            let name1 = eval_names(&bin.lho);
            let name2 = eval_names(&bin.rho);

            // Short-circuit / propagate if either side failed to produce a valid name
            if matches!(name1, Value::Undefined(_)) {
                return name1;
            }
            if matches!(name2, Value::Undefined(_)) {
                return name2;
            }

            pairing_operation::eval(name1, name2)
        }



        // Any other value will return and undefined value
        node => Value::Undefined(Undef::Name{ pos: node.position() }),
    }
}



// ----------------------------------------------------------------------------
//  APPLICATION OPERATION
// ----------------------------------------------------------------------------

/// `func arg` — applies a callable Swan value to an argument.
///
/// Dispatches on the type of `func`:
///
/// - `Value::Builtin(f)`: calls the native Rust function directly.
/// - `Value::Func { ... }`: a child scope of the closure is created, the parameters are bound to the argument, and the body is evaluated.
/// - `Value::List`: indexing by number, or by a tuple of numbers.
/// - `Value::Namespace`: lookup by text key, or by a tuple of keys.
/// - `Value::Text`: character indexing by number, or by a tuple of numbers.
/// - Anything else: returns `Undefined(application)`.
fn eval_apply(func: Value, arg: Value, pos: usize) -> Value {
    match func {
        Value::Builtin(ref f) => f.call(arg),

        Value::Func { params, body, closure } => {
            let local_scope = closure.new_child();
            assignment_operation::eval(&local_scope, &params, &arg);
            eval(&body, &local_scope)
        }

        // --- List Application ---
        Value::List(list) => {
            if matches!(arg, Value::Tuple(_)) {
                let items: Vec<Value> = arg
                    .items()
                    .map(|idx| eval_apply_list(&list, idx, pos))
                    .collect();
                collect_results(items)
            } else {
                eval_apply_list(&list, arg, pos)
            }
        }

        // --- Namespace Application ---
        Value::Namespace(scope) => {
            if matches!(arg, Value::Tuple(_)) {
                let items: Vec<Value> = arg
                    .items()
                    .map(|key| eval_apply_namespace(&scope, key, pos))
                    .collect();
                collect_results(items)
            } else {
                eval_apply_namespace(&scope, arg, pos)
            }
        }

        // --- Text Application ---
        Value::Text(text) => {
            if matches!(arg, Value::Tuple(_)) {
                let items: Vec<Value> = arg
                    .items()
                    .map(|idx| eval_apply_text(&text, idx, pos))
                    .collect();
                collect_results(items)
            } else {
                eval_apply_text(&text, arg, pos)
            }
        }

        other => Value::Undefined(Undef::BinaryOperation {
            op: "application",
            lhv: Box::new(other),
            rhv: Box::new(arg),
            pos,
        }),
    }
}

/// Applies a List to a single index `arg`.
///
/// Negative, fractional or out-of-range indices give `Nothing`.
/// Non-numbers give `Undefined`.
fn eval_apply_list(list: &[Value], arg: Value, pos: usize) -> Value {
    match arg {
        Value::Numb(n) => {
            if n >= 0.0 && n.fract() == 0.0 {
                let idx = n as usize;
                if idx < list.len() {
                    return list[idx].clone();
                }
            }
            // Index is out of bounds or not a whole integer
            Value::Nothing
        }
        // Argument is not a number
        other => Value::Undefined(Undef::BinaryOperation {
            op: "application",
            lhv: Box::new(Value::List(Rc::new(list.to_vec()))),
            rhv: Box::new(other),
            pos,
        }),
    }
}

/// Applies a Text to a single index `arg`.
///
/// Returns the character at position `arg` as a one-character `Text`.
/// Indexing is by Unicode character (`char`), not by byte, and is zero-based.
/// Negative, fractional or out-of-range indices give `Nothing`.
/// Non-numbers give `Undefined`.
fn eval_apply_text(text: &Rc<str>, arg: Value, pos: usize) -> Value {
    match arg {
        Value::Numb(n) => {
            if n >= 0.0 && n.fract() == 0.0 {
                if let Some(ch) = text.chars().nth(n as usize) {
                    return Value::Text(ch.to_string().into());
                }
            }
            // Index is out of bounds or not a whole integer
            Value::Nothing
        }
        // Argument is not a number
        other => Value::Undefined(Undef::BinaryOperation {
            op: "application",
            lhv: Box::new(Value::Text(text.clone())),
            rhv: Box::new(other),
            pos,
        }),
    }
}

/// Applies a Namespace to a single key `arg`.
///
/// Own-only lookup per Swan spec §3.7.
fn eval_apply_namespace(scope: &Scope, arg: Value, pos: usize) -> Value {
    match arg {
        Value::Text(t) => {
            // Own-only lookup per Swan spec §3.7
            let found = scope.0.borrow().bindings.get(t.as_ref()).cloned();
            found.unwrap_or(Value::Nothing)
        }
        // Argument is not a string
        other => Value::Undefined(Undef::BinaryOperation {
            op: "application",
            lhv: Box::new(Value::Namespace(scope.clone())),
            rhv: Box::new(other),
            pos,
        }),
    }
}



// ----------------------------------------------------------------------------
//  FUNCTION COMPOSITION OPERATION
// ----------------------------------------------------------------------------

/// `f << g` builds a builtin that computes `f(g(x))`.
fn eval_composition(f: Value, g: Value, pos: usize) -> Value {
    Value::Builtin(BuiltinFn::new("composed", move |x:Value| {
        let y = eval_apply(g.clone(), x, pos);    // y = g(x)
        eval_apply(f.clone(), y, pos)             // z = f(y)
    }))
}



// ----------------------------------------------------------------------------
//  Tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::assert_matches;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn evaluate(src: &str) -> Value {
        let mut parser = Parser::new(Lexer::new(src));
        let ast = parser.parse();
        let scope = Scope::new();
        scope.set("TRUE", Value::Bool(true));
        scope.set("FALSE", Value::Bool(false));
        eval(&ast, &scope)
    }

    // Operations delegated to sub-modules (affirm, negation, logical negation,
    // lifting, pairing, arithmetic, comparison) are tested in their own modules.

    #[test]
    fn number_literal() {
        assert_eq!(evaluate("7.2"), Value::Numb(7.2));
    }

    #[test]
    fn string_literal() {
        assert_eq!(evaluate("\"abc\""), Value::Text("abc".into()));
    }

    #[test]
    fn nothing_literal() {

        assert_eq!(evaluate(""), Value::Nothing);
        assert_eq!(evaluate("   "), Value::Nothing);
        assert_eq!(evaluate("# comment\n"), Value::Nothing);
        assert_eq!(evaluate("  # comment\n  "), Value::Nothing);

        assert_eq!(evaluate("()"), Value::Nothing);
        assert_eq!(evaluate("(   )"), Value::Nothing);
        assert_eq!(evaluate("(\n# comment\n)"), Value::Nothing);
        assert_eq!(evaluate("(())"), Value::Nothing);
    }

    #[test]
    fn identifier() {
        assert_eq!(evaluate("TRUE"), Value::Bool(true));
        assert_eq!(evaluate("x = 4, x"), Value::Numb(4.0));
    }

    #[test]
    fn list_literal() {
        assert_eq!(evaluate("[7 , 2 , 9]"), Value::List(Rc::new(vec![
                    Value::Numb(7.0),
                    Value::Numb(2.0),
                    Value::Numb(9.0)
        ])));

        assert_eq!(evaluate("[7 , [2,9]]"), Value::List(Rc::new(vec![
                    Value::Numb(7.0),
                    Value::List(Rc::new(vec![
                        Value::Numb(2.0),
                        Value::Numb(9.0)
                    ]))
        ])));

        assert_eq!(evaluate("[]"), Value::List(Rc::new(vec![])));
    }

    #[test]
    fn namespace_literal() {
        match evaluate("{x:10, y:20}") {
            Value::Namespace(scope) => {
                assert_eq!(scope.get("x"), Value::Numb(10.0));
                assert_eq!(scope.get("y"), Value::Numb(20.0));

                // Inherited from parent scope
                assert_eq!(scope.get("TRUE"), Value::Bool(true));
            }
            other => panic!("expected Namespace, got {:?}", other),
        }
    }

    #[test]
    fn function_definition() {

        assert_matches!(evaluate("(x, y) -> x+y"), Value::Func{params:_, body:_, closure:_});

        // Invalid left-hand names.
        assert_eq!(evaluate("(x, 12) -> 2*x"), invalid_names("function_definition", Value::Undefined(Undef::Name{pos:4}), 8));
    }

    #[test]
    fn application() {
        // --- Func ---
        assert_eq!(evaluate("f = (a) -> 2*a , f 5"), Value::Numb(10.0));
        assert_eq!(evaluate("f = (a,b) -> a+b , f(3,7)"), Value::Numb(10.0));
        assert_eq!(evaluate("f = (a,b,c) -> (a,b)*c , f(2,3,4,5)"), Value::Tuple(Rc::new(vec![
            Value::Numb(8.0),
            Value::Numb(15.0),
        ])));
        assert_eq!(evaluate("(()->3)()"), Value::Numb(3.0));
        assert_eq!(evaluate("(()->3) 10"), Value::Numb(3.0));

        // Nothing
        assert_eq!(evaluate("() 5"), Value::Nothing);

        // Tuple
        assert_eq!(
            evaluate("(x->2*x , x->3*x) 4"),
            Value::Tuple(Rc::new(vec![Value::Numb(8.0), Value::Numb(12.0)]))
        );

        // --- List ---
        assert_eq!(evaluate("[10, 20, 30] 1"), Value::Numb(20.0));

        // Out-of-bounds returns Value::Nothing
        assert_eq!(evaluate("[10, 20] 5"), Value::Nothing);

        // Non-number argument returns Undefined(Undef::BinaryOperation)
        assert!(matches!(
            evaluate(r#"[10, 20] "0""#),
            Value::Undefined(Undef::BinaryOperation { op: "application", .. })
        ));

        // Applying list to a Tuple
        assert_eq!(
            evaluate("[10, 20, 30] (0, 5, 2)"),
            Value::Tuple(Rc::new(vec![Value::Numb(10.0), Value::Nothing, Value::Numb(30.0)]))
        );
        assert_eq!(
            evaluate("([1,2,3] , [10,20,30]) 1"),
            Value::Tuple(Rc::new(vec![Value::Numb(2.0), Value::Numb(20.0)]))
        );

        // --- Namespace ---
        assert_eq!(evaluate(r#"{x: 10, y: 20} "x""#), Value::Numb(10.0));

        // Missing key returns Value::Nothing
        assert_eq!(evaluate(r#"{x: 10} "z""#), Value::Nothing);

        // Non-text key returns Undefined(Undef::BinaryOperation)
        assert!(matches!(
            evaluate("{x: 10} 1"),
            Value::Undefined(Undef::BinaryOperation { op: "application", .. })
        ));

        // Applying namespace to a Tuple
        assert_eq!(
            evaluate(r#"{x: 10, y: 20} ("x", "missing")"#),
            Value::Tuple(Rc::new(vec![Value::Numb(10.0), Value::Nothing]))
        );

        assert_eq!(
            evaluate("({x:1, y:2} , {x:10, y:20}) \"y\""),
            Value::Tuple(Rc::new(vec![Value::Numb(2.0), Value::Numb(20.0)]))
        );

        // --- Text ---
        assert_eq!(evaluate(r#""hello" 1"#), Value::Text("e".into()));
        assert_eq!(evaluate(r#""hello" 0"#), Value::Text("h".into()));

        // Indexing is by character, not by byte
        assert_eq!(evaluate(r#""héllo" 1"#), Value::Text("é".into()));

        // Out-of-bounds, negative and fractional indices return Nothing
        assert_eq!(evaluate(r#""hello" 5"#), Value::Nothing);
        assert_eq!(evaluate(r#""hello" (-1)"#), Value::Nothing);
        assert_eq!(evaluate(r#""hello" 1.5"#), Value::Nothing);
        assert_eq!(evaluate(r#""" 0"#), Value::Nothing);

        // Non-number argument returns Undefined(Undef::BinaryOperation)
        match evaluate(r#""abc" "0""#) {
            Value::Undefined(Undef::BinaryOperation { op, lhv, rhv, .. }) => {
                assert_eq!(op, "application");
                assert_eq!(*lhv, Value::Text("abc".into()));
                assert_eq!(*rhv, Value::Text("0".into()));
            }
            other => panic!("expected Undefined::BinaryOperation, got {:?}", other),
        }

        // Applying text to a Tuple (Nothing is kept for positional alignment)
        assert_eq!(
            evaluate(r#""hello" (0, 10, 4)"#),
            Value::Tuple(Rc::new(vec![
                Value::Text("h".into()),
                Value::Nothing,
                Value::Text("o".into()),
            ]))
        );

        // Applying a tuple of texts to a number
        assert_eq!(
            evaluate(r#"("abc", "xyz") 1"#),
            Value::Tuple(Rc::new(vec![
                Value::Text("b".into()),
                Value::Text("y".into()),
            ]))
        );
    }

    #[test]
    fn assignment() {
        // Assignment evaluates to Nothing but binds the name.
        assert_eq!(evaluate("x = 5"), Value::Nothing);
        assert_eq!(evaluate("x = 5, x"), Value::Numb(5.0));

        // Destructuring assignment.
        assert_eq!(evaluate("(a, b) = (1, 2), a + b"), Value::Numb(3.0));

        // Invalid left-hand names.
        assert_eq!(evaluate("(x, 12) = (1, 2)"), invalid_names("assignment", Value::Undefined(Undef::Name{pos:4}), 8));
    }

    #[test]
    fn labelling() {
        // `x : 5` evaluates to the assigned value itself (unlike `=`),
        // *and* binds `x` in scope. Parses as `(x : 5), x`.
        assert_eq!(evaluate("x : 5"), Value::Numb(5.0));

        assert_eq!(
            evaluate("x : 5, x"),
            Value::Tuple(Rc::new(vec![Value::Numb(5.0), Value::Numb(5.0)]))
        );

        // Destructuring labelling behaves like destructuring assignment,
        // but also yields the assigned values as the LabellingOp's result.
        assert_eq!(
            evaluate("(a, b) : (1, 2)"),
            Value::Tuple(Rc::new(vec![Value::Numb(1.0), Value::Numb(2.0)]))
        );

        // Invalid left-hand names.
        assert_eq!(evaluate("(x, 12) : (1, 2)"), invalid_names("labelling", Value::Undefined(Undef::Name{pos:4}), 8));
    }

    #[test]
    fn subcontexting() {
        assert_eq!(evaluate("{x:10}.x"), Value::Numb(10.0));
        assert_eq!(evaluate("{x:10, y:20}.(x+y)"), Value::Numb(30.0));
        assert_eq!(evaluate("{x:10, y:20}.TRUE"), Value::Bool(true));
        match evaluate("x=10, x.TRUE") {
            Value::Undefined(Undef::BinaryOperation { op, lhv, rhv, pos }) => {
                assert_eq!(op, "subcontexting");
                assert_eq!(*lhv, Value::Numb(10.0));
                assert_eq!(*rhv, Value::Nothing);
                assert_eq!(pos, 7);
            }
            other => panic!("expected Undefined::BinaryOperation, got {:?}", other),
        }
    }

    #[test]
    fn mapping() {

        // Mapping tuple to function
        assert_eq!(evaluate("f = a -> 2*a , (2,3,4) => f"), Value::Tuple(Rc::new(vec![
            Value::Numb(4.0),
            Value::Numb(6.0),
            Value::Numb(8.0),
        ])));

        // Mapping value to function
        assert_eq!(evaluate("f = a -> 2*a , 2 => f"), Value::Numb(4.0));

        // Mapping nothing to function
        assert_eq!(evaluate("f = a -> 2*a , () => f"), Value::Nothing);

        // Normalizing output of mapping
        assert_eq!(evaluate("f = a -> () , (1,2,3) => f"), Value::Nothing);

        // Mapping tuple to tuple of functions
        assert_eq!(
            evaluate("f = a -> 2*a , g = b -> 3*b, (2,5) => (f,g)"),
            Value::Tuple(Rc::new(vec![
                Value::Numb(4.0),
                Value::Numb(6.0),
                Value::Numb(10.0),
                Value::Numb(15.0),
            ]))
        );
    }

    #[test]
    fn conditional() {

        // Left side casts to true -> returns right-hand side operand
        assert_eq!(evaluate("TRUE ? 10"), Value::Numb(10.0));
        assert_eq!(evaluate("1 ? \"yes\""), Value::Text("yes".into()));
        assert_eq!(evaluate("[1] ? 42"), Value::Numb(42.0));

        // Left side casts to false -> returns Nothing
        assert_eq!(evaluate("FALSE ? 10"), Value::Nothing);
        assert_eq!(evaluate("0 ? 10"), Value::Nothing);
        assert_eq!(evaluate("\"\" ? 10"), Value::Nothing);
        assert_eq!(evaluate("() ? 10"), Value::Nothing);
        assert_eq!(evaluate("() ? ()"), Value::Nothing);

        // Tuples get filtered by truthyness
        assert_eq!(
            evaluate("(TRUE, FALSE, TRUE, FALSE) ? (1,2,3,4)"),
            Value::Tuple(Rc::new(vec![
                Value::Numb(1.0),
                Value::Numb(3.0),
            ]))
        );
    }

    #[test]
    fn alternative() {
        assert_eq!(evaluate("() ; 2"), Value::Numb(2.0));
        assert_eq!(evaluate("1 ; 2"), Value::Numb(1.0));
        assert_eq!(evaluate("FALSE ; 2"), Value::Bool(false));
    }

    #[test]
    fn and_op() {

        // True & RHS -> returns RHS
        assert_eq!(evaluate("TRUE & 10"), Value::Numb(10.0));
        assert_eq!(evaluate("TRUE & FALSE"), Value::Bool(false));
        assert_eq!(evaluate("1 & \"hello\""), Value::Text("hello".into()));

        // False & RHS -> returns LHS
        assert_eq!(evaluate("FALSE & 10"), Value::Bool(false));
        assert_eq!(evaluate("0 & 10"), Value::Numb(0.0));
        assert_eq!(evaluate("\"\" & 10"), Value::Text("".into()));

        // Nothing
        assert_eq!(evaluate("() & ()"), Value::Nothing);
        assert_eq!(evaluate("() & 10"), Value::Nothing);

        // Tuple
        assert_eq!(
            evaluate("(FALSE, 1) & (10, \"abc\")"),
            Value::Tuple(Rc::new(vec![
                Value::Bool(false),
                Value::Text("abc".into()),
            ]))
        );
        assert_eq!(evaluate("TRUE & (10, 1)"), Value::Numb(10.0));
    }

    #[test]
    fn or_op() {
        // True | RHS -> returns LHS
        assert_eq!(evaluate("TRUE | 10"), Value::Bool(true));
        assert_eq!(evaluate("1 | 10"), Value::Numb(1.0));
        assert_eq!(evaluate("\"yes\" | 10"), Value::Text("yes".into()));

        // False | RHS -> returns RHS
        assert_eq!(evaluate("FALSE | 10"), Value::Numb(10.0));
        assert_eq!(evaluate("0 | \"fallback\""), Value::Text("fallback".into()));
        assert_eq!(evaluate("\"\" | TRUE"), Value::Bool(true));

        // Nothing
        assert_eq!(evaluate("() | 5"), Value::Numb(5.0));

        // Tuple
        assert_eq!(
            evaluate("(FALSE, 1) | (10, \"abc\")"),
            Value::Tuple(Rc::new(vec![
                Value::Numb(10.0),
                Value::Numb(1.0),
            ]))
        );
        assert_eq!(evaluate("(FALSE, 0) | 10"), Value::Numb(10.0));
    }

    #[test]
    fn composition() {
        assert_eq!(evaluate("f = y -> 2*y , g = x -> x+1 , (f<<g)(3)"), Value::Numb(8.0));
    }

    #[test]
    fn reverse_composition() {
        assert_eq!(evaluate("f = y -> 2*y , g = x -> x+1 , (f>>g)(3)"), Value::Numb(7.0));
    }

    #[test]
    fn unclosed_bracket() {
        assert!(matches!(evaluate("(1, 2"), Value::Undefined(Undef::Termination{ .. })));
    }

    #[test]
    fn unexpected_token() {
        assert!(matches!(evaluate(")"), Value::Undefined(Undef::Token{ .. })));
    }

    #[test]
    fn precedence() {
        assert_eq!(evaluate("ns = {f: x -> 2*x}, ns.f 3"), Value::Numb(6.0));
        assert_eq!(evaluate("ns = {a=10}, f = x -> 2*x, f ns.a"), Value::Numb(20.0));
        assert_eq!(evaluate("f = x -> {y:x}, (f 3).y"), Value::Numb(3.0));
    }
}
