//! Evaluator: turns AST nodes into Swan `Value`s.
//!
//! # Entry point
//! [`eval`] takes a `&Node` and a `&Scope` and returns a `Value`. It is
//! **infallible**: every error (syntax errors, invalid names, type mismatches)
//! is returned as a `Value::Undefined(...)`, and errors in sub-expressions
//! propagate as ordinary values. It never panics.
//!
//! # What lives where
//! This file handles literals, identifier lookup, grouping (`[]`, `{}`),
//! function definition and application, pairing dispatch, mapping, assignment,
//! labelling, subcontexting, promise chaining (`~>`), logic (`&`, `|`),
//! conditional (`?`, `;`), composition (`<<`, `>>`) and syntax-error nodes.
//! Operations with non-trivial logic are delegated to the sub-modules declared
//! below (unary operators, pairing, assignment, `~>`, arithmetic and comparison).
//!
//! # Tuple semantics
//! Tuples are flat and normalized (`Nothing` items are dropped). Operators are
//! lifted over tuples by two internal macros:
//! - `map_values!` for unary operations: `Nothing` gives `Nothing`, a tuple is
//!   mapped item by item, any other value goes straight to the operation.
//! - `map_pairs!` for binary operations: tuples are paired element-wise
//!   (the shorter side is padded with `Nothing`) and the result is normalized.
//!
//! # Evaluation order
//! Operands are evaluated eagerly, left then right. The only exception is the
//! alternative operator `A ; B`, whose right operand is evaluated only when `A`
//! is `Nothing`. For `->`, `=` and `:` the left operand is not evaluated; it is
//! resolved into names by `eval_names`.
//!
//! # Scopes
//! - `{X}` evaluates `X` in a new child scope and returns it as a `Namespace`.
//! - `A -> B` captures the current scope as a closure; applying it evaluates
//!   the body in a child of that closure scope.
//! - `ns.X` evaluates `X` inside the scope of the namespace `ns`.

use std::rc::Rc;
use crate::ast::Node;
use crate::value::{Scope, Value, Undef, BuiltinFn};


// Unary operations
/// Unary `+X`.
mod affirm_operation;
/// Unary `-X` (numeric negation).
mod negation_operation;
/// Unary `!X` (logical negation).
mod logical_negation_operation;
/// Unary `$X` (lifting: looks up names in the scope).
mod lifting_operation;

// Binary operations
/// `X , Y` (tuple construction with flattening and normalization).
mod pairing_operation;
/// Binding of names to values; shared by `=`, `:` and function application.
pub(crate) mod assignment_operation;
/// `P ~> F` (promise chaining).
mod then_operation;
/// `+ - * / % ^` on numbers (and other supported types).
mod arithmetic_operations;
/// `== != > >= < <=`.
mod comparison_operations;



// ----------------------------------------------------------------------------
//  MACROS
// ----------------------------------------------------------------------------

/// Evaluates a binary operation with element-wise tuple broadcasting.
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
/// Requires `Value` to be in scope. Both operands are evaluated by the caller before the macro is used.
macro_rules! map_pairs {
    ($lhv:expr, $rhv:expr, $eval_fn:expr) => {{
        let (lhv, rhv) = ($lhv, $rhv);
        if matches!(lhv, Value::Tuple(_)) || matches!(rhv, Value::Tuple(_)) {
            Value::tuple(
                lhv.pair(&rhv)
                    .map(|(l,r)| $eval_fn(l,r))
                    .collect(),
            )
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
///
/// The `$eval_fn` must be a callable taking one `Value`.
/// Requires `Value` to be in scope.
/// Note that `$val` is evaluated once and moved into the macro.
macro_rules! map_values {
    ($val:expr, $eval_fn:expr) => {{
        let val = $val;
        match val {
            Value::Nothing => Value::Nothing,
            Value::Tuple(_) => {
                Value::tuple(
                    val.items()
                    .map($eval_fn)
                    .collect()
                )
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
/// are preserved throughout, and sub-expression errors propagate as `Undefined` values.
///
/// # Arguments
/// - `node`: the AST node to evaluate.
/// - `scope`: the environment used for identifier lookup. Assignment and labelling
///   write into this scope (not into a parent).
///
/// # Error values produced directly here
/// - `Undef::Termination` for `Node::Unclosed` (missing closing bracket).
/// - `Undef::Token` for `Node::UnexpectedToken`.
/// - `Undef::BinaryOperation` named `"function_definition"`, `"assignment"` or
///   `"labelling"` when the left side is not a valid name or tuple of names
///   (the `lhv` holds the `Undef::Name` describing the offending node).
/// - `Undef::BinaryOperation` named `"subcontexting"` when the left side is not
///   a namespace.
/// - `Undef::BinaryOperation` named `"application"` (see [`eval_apply`]).
///
/// Errors from the delegated sub-modules are documented there.
///
/// # Examples
///
/// ```
/// use swan_core::builtins::root_scope;
/// use swan_core::evaluator::eval;
/// use swan_core::lexer::Lexer;
/// use swan_core::parser::Parser;
/// use swan_core::value::Value;
///
/// let scope = root_scope();
/// let ast = Parser::new(Lexer::new(r#""a" - 1"#)).parse();
/// assert!(matches!(eval(&ast, &scope), Value::Undefined(_)));
/// ```
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
            Value::list(vec)
        }

        // Namespace creation: `{X}` - body is evaluated for side effects (bindings), result is discarded
        Node::Namespace(unary) => {
            let child_scope = scope.new_child();
            eval(&unary.opd, &child_scope);
            Value::Namespace(child_scope)
        }



        // ===== Unary Operations =====

        // Unary affirm operation: `+X`
        // (not broadcast via map_values!: the sub-module handles any value itself)
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
        Node::ApplicationOp(bin) => {
            // The left-hand operand must evaluate to a callable (Builtin or Func).
            // The right-hand operand is the argument (possibly a Tuple for multiple
            // arguments).
            // Functions are broadcast: for a tuple of callables each one receives the same whole argument.
            let func = eval(&bin.lho, scope);
            let args = eval(&bin.rho, scope);
            map_values!(func, |f| eval_apply(f, args.clone(), bin.pos))
        }

        // Pairing: `X , Y` - flattens both sides into a normalized tuple.
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

        // Promise chaining: `P ~> F` - each promise in `P` (tuples are broadcast) is chained to `F`; the result of each is a new promise.
        Node::ThenOp(bin) => {
            let promises = eval(&bin.lho, scope);
            let func     = eval(&bin.rho, scope);
            map_values!(promises, |p| then_operation::eval(p, func.clone(), bin.pos))
        }

        // Assignment: `name = V` - evaluates to Nothing, binds names in scope. Invalid names give an Undefined and nothing is bound.
        Node::AssignmentOp(bin) => {
            let names = eval_names(&bin.lho);
            if matches!(names, Value::Undefined(_)) {
                return invalid_names("assignment", names, bin.pos);
            }

            let values = eval(&bin.rho, scope);
            assignment_operation::eval(scope, &names, &values);
            Value::Nothing
        }

        // Labelling: `name : V` - evaluates to V, binds names in scope. Invalid names give an Undefined and nothing is bound.
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
                lhv => Value::undef_binary("subcontexting", lhv, Value::Nothing, bin.pos),
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

        // Greater-than-or-equal: `A >= B`
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

        // Less-than-or-equal: `A <= B`
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

        // Alternative: `A ; B` - RHO is evaluated lazily, and only if LHO is exactly Nothing (a tuple is never Nothing, so it is returned as is).
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
/// operation is not a valid name or tuple of names.
///
/// - `op`: the operation name (`"function_definition"`, `"assignment"` or `"labelling"`).
/// - `names`: the `Undefined(Undef::Name{..})` returned by [`eval_names`].
/// - `pos`: position of the operator.
///
/// The result is `Undef::BinaryOperation { op, lhv: names, rhv: Nothing, pos }`.
fn invalid_names(op: &'static str, names: Value, pos: usize) -> Value {
    Value::undef_binary(op, names, Value::Nothing, pos)
}

/// Resolves an AST node into names, without evaluating it.
///
/// Used for the left side of `=`, `:` and `->`, which must be an identifier,
/// `()`, or a pairing (`,`) of those (parentheses do not matter).
///
/// Returns:
/// - `Value::Name` for an identifier.
/// - `Value::Nothing` for `()` or an empty expression.
/// - A normalized tuple of names for pairings.
/// - `Undefined(Undef::Name{pos})` for any other node, where `pos` is the
///   position of the offending node. In a pairing, the first `Undefined`
///   (left side first) is propagated unchanged.
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

/// `func arg` - applies a callable or indexable Swan value to an argument.
///
/// `pos` is the source position of the application, used in error values.
///
/// Dispatches on the type of `func`:
///
/// - `Value::Builtin(f)`: calls the native Rust function with the whole `arg`.
/// - `Value::Func { ... }`: creates a child scope of the closure, binds the
///   parameters to `arg` (destructuring tuples, see `assignment_operation`),
///   and evaluates the body there.
/// - `Value::List`: indexing by a zero-based number. A tuple argument gives a
///   tuple of results, normalized (so out-of-range indices are dropped).
/// - `Value::Namespace`: own-only lookup by text key. A tuple argument gives a
///   normalized tuple of results (missing keys are dropped).
/// - `Value::Text`: character indexing by a zero-based number. A tuple argument
///   gives a normalized tuple of results.
/// - Anything else (including `Nothing` and `Undefined`): returns
///   `Undef::BinaryOperation { op: "application", lhv: func, rhv: arg, pos }`.
///
/// Broadcasting over a tuple of *callables* is not done here; the callers
/// (`ApplicationOp`, `MappingOp`) do it with `map_values!`.
pub fn eval_apply(func: Value, arg: Value, pos: usize) -> Value {
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
                Value::tuple(items)
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
                Value::tuple(items)
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
                Value::tuple(items)
            } else {
                eval_apply_text(&text, arg, pos)
            }
        }

        other => Value::undef_binary("application", other, arg, pos),
    }
}

/// Applies a List to a single index `arg`.
///
/// Negative, fractional or out-of-range indices give `Nothing`.
/// Non-numbers give `Undefined`. `pos` is used for the `Undefined` returned for a non-number index.
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
        other => Value::undef_binary("application", Value::list(list.to_vec()), other, pos),
    }
}

/// Applies a Text to a single index `arg`.
///
/// Returns the character at position `arg` as a one-character `Text`.
/// Indexing is by Unicode character (`char`), not by byte, and is zero-based.
/// Negative, fractional or out-of-range indices give `Nothing`.
/// Non-numbers give `Undefined`. `pos` is used for the `Undefined` returned for a non-number index.
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
        other => Value::undef_binary("application", Value::Text(text.clone()), other, pos),
    }
}

/// Applies a Namespace to a single key `arg`.
///
/// Own-only lookup: parent scopes are not searched.
/// A missing key gives `Nothing`; a non-text key gives
/// `Undef::BinaryOperation { op: "application", .. }`.
fn eval_apply_namespace(scope: &Scope, arg: Value, pos: usize) -> Value {
    match arg {
        Value::Text(t) => {
            let found = scope.0.borrow().bindings.get(t.as_ref()).cloned();
            found.unwrap_or(Value::Nothing)
        }
        // Argument is not a string
        other => Value::undef_binary("application", Value::Namespace(scope.clone()), other, pos),
    }
}



// ----------------------------------------------------------------------------
//  FUNCTION COMPOSITION OPERATION
// ----------------------------------------------------------------------------

/// `f << g` builds a `Builtin` named `"composed"` that computes `f(g(x))`.
///
/// Both functions are applied with [`eval_apply`] and the whole argument `x`
/// is passed to `g`; its whole result is passed to `f`. Applying errors become
/// `Undefined` values, as usual. `pos` is the position of the composition
/// operator and is used for any application error. `>>` calls this with its
/// operands swapped. Tuple operands are broadcast by the caller using `map_pairs!`.
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
/// Tests for the constructs handled directly in this file. Operations delegated to sub-modules are tested in their own modules.
mod tests {
    use super::*;
    use std::assert_matches;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    /// Parses and evaluates `src` in a fresh scope that has `TRUE` and `FALSE` defined.
    fn evaluate(src: &str) -> Value {
        let mut parser = Parser::new(Lexer::new(src));
        let ast = parser.parse();
        let scope = Scope::new();
        scope.set("TRUE", Value::Bool(true));
        scope.set("FALSE", Value::Bool(false));
        eval(&ast, &scope)
    }

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
        assert_eq!(
            evaluate("[7 , 2 , 9]"), 
            Value::list(vec![
                Value::Numb(7.0),
                Value::Numb(2.0),
                Value::Numb(9.0)
            ])
        );

        assert_eq!(
            evaluate("[7 , [2,9]]"), 
            Value::list(vec![
                Value::Numb(7.0),
                Value::list(vec![
                    Value::Numb(2.0),
                    Value::Numb(9.0)
                ])
            ])
        );

        assert_eq!(evaluate("[]"), Value::list(vec![]));
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
        assert_eq!(
            evaluate("f = (a,b,c) -> (a,b)*c , f(2,3,4,5)"), 
            Value::tuple(vec![Value::Numb(8.0), Value::Numb(15.0)])
        );
        assert_eq!(evaluate("(()->3)()"), Value::Numb(3.0));
        assert_eq!(evaluate("(()->3) 10"), Value::Numb(3.0));

        // Nothing
        assert_eq!(evaluate("() 5"), Value::Nothing);

        // Tuple
        assert_eq!(
            evaluate("(x->2*x , x->3*x) 4"),
            Value::tuple(vec![Value::Numb(8.0), Value::Numb(12.0)])
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
            Value::tuple(vec![Value::Numb(10.0), Value::Numb(30.0)])
        );
        assert_eq!(
            evaluate("([1,2,3] , [10,20,30]) 1"),
            Value::tuple(vec![Value::Numb(2.0), Value::Numb(20.0)])
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
            Value::Numb(10.0),
        );

        assert_eq!(
            evaluate("({x:1, y:2} , {x:10, y:20}) \"y\""),
            Value::tuple(vec![Value::Numb(2.0), Value::Numb(20.0)])
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
            Value::tuple(vec![
                Value::Text("h".into()),
                Value::Text("o".into()),
            ])
        );

        // Applying a tuple of texts to a number
        assert_eq!(
            evaluate(r#"("abc", "xyz") 1"#),
            Value::tuple(vec![
                Value::Text("b".into()),
                Value::Text("y".into()),
            ])
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
            Value::tuple(vec![Value::Numb(5.0), Value::Numb(5.0)])
        );

        // Destructuring labelling behaves like destructuring assignment,
        // but also yields the assigned values as the LabellingOp's result.
        assert_eq!(
            evaluate("(a, b) : (1, 2)"),
            Value::tuple(vec![Value::Numb(1.0), Value::Numb(2.0)])
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
        assert_eq!(evaluate("f = a -> 2*a , (2,3,4) => f"), Value::tuple(vec![
            Value::Numb(4.0),
            Value::Numb(6.0),
            Value::Numb(8.0),
        ]));

        // Mapping value to function
        assert_eq!(evaluate("f = a -> 2*a , 2 => f"), Value::Numb(4.0));

        // Mapping nothing to function
        assert_eq!(evaluate("f = a -> 2*a , () => f"), Value::Nothing);

        // Normalizing output of mapping
        assert_eq!(evaluate("f = a -> () , (1,2,3) => f"), Value::Nothing);

        // Mapping tuple to tuple of functions
        assert_eq!(
            evaluate("f = a -> 2*a , g = b -> 3*b, (2,5) => (f,g)"),
            Value::tuple(vec![
                Value::Numb(4.0),
                Value::Numb(6.0),
                Value::Numb(10.0),
                Value::Numb(15.0),
            ])
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
            Value::tuple(vec![
                Value::Numb(1.0),
                Value::Numb(3.0),
            ])
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
            Value::tuple(vec![
                Value::Bool(false),
                Value::Text("abc".into()),
            ])
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
            Value::tuple(vec![
                Value::Numb(10.0),
                Value::Numb(1.0),
            ])
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

    #[test]
    fn promise_as_condition() {
        // A promise is FALSE while pending and TRUE once resolved, whatever its value
        let scope = crate::builtins::root_scope().new_child();
        let run = |src: &str| eval(&Parser::new(Lexer::new(src)).parse(), &scope);

        run("p = promise.create()");
        assert_eq!(run("p ? 1"), Value::Nothing);
        assert_eq!(run("p ? 1 ; 2"), Value::Numb(2.0));
        assert_matches!(run("p & 1"), Value::Promise(_));
        assert_eq!(run("p | 1"), Value::Numb(1.0));
        assert_eq!(run("!p"), Value::Bool(true));
        assert_eq!(run("(TRUE, p) ? (1, 2)"), Value::Numb(1.0));

        run("promise.resolve p FALSE");
        assert_eq!(run("p ? 1"), Value::Numb(1.0));
        assert_eq!(run("p & 1"), Value::Numb(1.0));
        assert_matches!(run("p | 1"), Value::Promise(_));
        assert_eq!(run("!p"), Value::Bool(false));
        assert_eq!(run("(TRUE, p) ? (1, 2)"), Value::tuple(vec![Value::Numb(1.0), Value::Numb(2.0)]));
    }
}
