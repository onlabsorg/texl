// src/eval.rs

use std::rc::Rc;
use std::cmp::Ordering;
use crate::ast::Node;
use crate::value::{Scope, Value, Undef, BuiltinFn};





// ----------------------------------------------------------------------------
//  MACROS
// ----------------------------------------------------------------------------

/// Evaluates a binary operation with element-wise tuple broadcasting (§3.5).
///
/// If either operand is a [`Value::Tuple`], pairs the items using [`Value::pair`]
/// and applies `$eval_fn` element-wise. Otherwise, applies `$eval_fn` directly.
///
/// Requires 4 arguments: `($lhv, $rhv, $eval_fn, $pos)`
macro_rules! eval_binary {
    ($eval_fn:path, $lhv:expr, $rhv:expr, $pos:expr) => {{
        let (lhv, rhv) = ($lhv, $rhv);
        if matches!(lhv, Value::Tuple(_)) || matches!(rhv, Value::Tuple(_)) {
            Value::Tuple(Rc::new(
                lhv.pair(&rhv)
                    .map(|(l, r)| $eval_fn(l, r, $pos))
                    .collect(),
            ))
        } else if lhv == Value::Nothing && rhv == Value::Nothing {
            Value::Nothing
        } else {
            $eval_fn(lhv, rhv, $pos)
        }
    }};
}





// ----------------------------------------------------------------------------
//  PUBLIC ENTRY POINT
// ----------------------------------------------------------------------------

/// Evaluates an AST node within a given `Scope` context and returns a `Value`.
///
/// This function is **infallible**: it never panics.  All error conditions are
/// encoded as `Value::Undefined(...)` so that Swan's "errors are values" semantics
/// are preserved throughout. (#14)
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

        // List creation: `[X]`
        Node::List(unary) => {
            let val = eval(&unary.opd, scope);
            let vec: Vec<_> = val.items().collect();
            Value::List(Rc::new(vec))
        }

        // Namespace creation: `{X}`
        Node::Namespace(unary) => {
            let child_scope = scope.new_child();
            eval(&unary.opd, &child_scope);
            Value::Namespace(child_scope)
        }



        // ===== Unary Operations =====

        // Unary affimr operation: `+X`
        Node::AffirmOp(unary) => {
            let val = eval(&unary.opd, scope);
            eval_affirm(val)          // pos not needed — pure identity
        }

        // Unary negation operation: `-X`
        Node::NegateOp(unary) => {
            let val = eval(&unary.opd, scope);
            if matches!(val, Value::Tuple(_)) {
                Value::Tuple(Rc::new(
                    val.items()
                    .map(|v| eval_negate(v, unary.pos))
                    .collect()
                ))
            } else {
                eval_negate(val, unary.pos)
            }
        }

        // Unary logical negation operation: `!X`
        Node::LogicalNegationOp(unary) => {
            let val = eval(&unary.opd, scope);
            Value::Bool(!bool::from(val))
        }

        // Unary lifting operation: `$X`
        Node::LiftingOp(unary) => {
            let names = eval_names(&unary.opd);
            if matches!(names, Value::Tuple(_)) {
                Value::Tuple(Rc::new(
                    names.items()
                    .map(|n| eval_lifting(n, &scope, unary.pos))
                    .collect()
                ))
            } else {
                eval_lifting(names, &scope, unary.pos)
            }
        }



        // ===== Binary Operations :: Core =====

        // Function definition: `A -> B`
        Node::FunctionDefinitionOp(bin) => {
            let names = eval_names(&bin.lho);
            if matches!(names, Value::Undefined(_)) {
                Value::Undefined(Undef::BinaryOperation{
                    op: "function_definition",
                    lhv: Box::new(names),
                    rhv: Box::new(Value::Nothing),
                    pos: bin.pos,
                })
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
            // arguments). Builtin closures receive the whole argument Value and are
            // responsible for destructuring it if needed.
            let func = eval(&bin.lho, scope);
            let args= eval(&bin.rho, scope);
            eval_apply(func, args, bin.pos)
        }

        // Pairing operation: `X , Y`
        Node::PairingOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            eval_pairing(lhv, rhv)
        }

        // Mapping: `T => F`
        Node::MappingOp(bin) => {
            let args = eval(&bin.lho, scope);
            let func = eval(&bin.rho, scope);
            let vals: Vec<Value> = args.items().map(|arg| eval_apply(func.clone(), arg, bin.pos)).collect();
            match vals.len() {
                0 => Value::Nothing,
                1 => vals[0].clone(),
                _ => Value::Tuple(vals.into()),
            }
        }

        // Assignemt: `name = V`
        Node::AssignmentOp(bin) => {
            let names = eval_names(&bin.lho);
            if matches!(names, Value::Undefined(_)) {
                return Value::Undefined(Undef::BinaryOperation{
                    op: "assignment",
                    lhv: Box::new(names),
                    rhv: Box::new(Value::Nothing),
                    pos: bin.pos,
                });
            }

            let values = eval(&bin.rho, scope);
            eval_assignment(scope, &names, &values);
            Value::Nothing
        }

        // Labelling: `name : V`
        Node::LabellingOp(bin) => {
            let names = eval_names(&bin.lho);
            if matches!(names, Value::Undefined(_)) {
                return Value::Undefined(Undef::BinaryOperation{
                    op: "labelling",
                    lhv: Box::new(names),
                    rhv: Box::new(Value::Nothing),
                    pos: bin.pos,
                });
            }

            let values = eval(&bin.rho, scope);
            eval_assignment(scope, &names, &values);
            values
        }

        // `ns.expression` — namespace sub-contexting
        Node::SubcontextingOp(bin) => {
            // The left-hand side must evaluate to a `Value::Namespace`.
            // The right-hand side must be an `Identifier` node; its string is used
            // as the lookup key, not its evaluated value.
            match eval(&bin.lho, scope) {
                Value::Namespace(ns_scope) => {
                    eval(&bin.rho, &ns_scope)
                }
                lhv => Value::Undefined(Undef::BinaryOperation {
                    op:  "subcontexting",
                    lhv: Box::new(lhv),
                    rhv: Box::new(Value::Nothing),  // rhv can´t be evaluated without a scope
                    pos: bin.pos,
                }),
            }
        }



        // ===== Arithmetic Operations =====

        // Sum: `A + B`
        Node::SumOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            eval_binary!(eval_add, lhv, rhv, bin.pos)
        }

        // Subtraction: `A - B`
        Node::SubtractionOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            eval_binary!(eval_subtraction, lhv, rhv, bin.pos)
        }

        // Product: `A * B`
        Node::ProductOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            eval_binary!(eval_product, lhv, rhv, bin.pos)
        }

        // Division: `A / B`
        Node::DivisionOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            eval_binary!(eval_division, lhv, rhv, bin.pos)
        }

        // Modulo: `A % B`
        Node::ModuloOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            eval_binary!(eval_modulo, lhv, rhv, bin.pos)
        }

        // Exponentiation: `A ^ B`
        Node::PowerOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            eval_binary!(eval_power, lhv, rhv, bin.pos)
        }



        // ===== Comparison Operations =====

        // Equal: `A == B`
        Node::EqualToOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            eval_equal_to(lhv, rhv)
        }

        // Non-equal: `A != B`
        Node::NotEqualToOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            eval_not_equal_to(lhv, rhv)
        }

        // Greater-than: `A > B`
        Node::GreaterThanOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            eval_greater_than(lhv, rhv, bin.pos)
        }

        // Non-greater-than: `A <= B`
        Node::GreaterThanOrEqualToOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            eval_greater_than_or_equal_to(lhv, rhv, bin.pos)
        }

        // Less than: `A < B`
        Node::LessThanOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            eval_less_than(lhv, rhv, bin.pos)
        }

        // Non-less-than: `A >= B`
        Node::LessThanOrEqualToOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            eval_less_than_or_equal_to(lhv, rhv, bin.pos)
        }



        // ===== Logic Operations =====

        // And: `A & B`
        Node::AndOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            if bool::from(lhv.clone()) {
                eval(&bin.rho, scope)
            } else {
                lhv
            }
        }

        // Or: `A | B`
        Node::OrOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            if bool::from(lhv.clone()) {
                lhv
            } else {
                eval(&bin.rho, scope)
            }
        }



        // ===== Conditional and alternative =====

        // Conditional: `A ? B`
        Node::ConditionalOp(bin) => {
            let cnd = bool::from(eval(&bin.lho, scope));
            if cnd {
                eval(&bin.rho, scope)
            } else {
                Value::Nothing
            }
        }

        // Alternative: `A ; B`
        Node::AlternativeOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            match lhv {
                Value::Nothing => eval(&bin.rho, scope),
                _ => lhv
            }
        }



        // ===== Function composition =====

        // Composition: `F << G`
        Node::CompositionOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            eval_binary!(eval_composition, lhv, rhv, bin.pos)
        }

        // Reverse composition: `F >> G`
        Node::ReverseCompositionOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            eval_binary!(eval_composition, rhv, lhv, bin.pos)
        }



        // ===== Syntax Errors =====

        // Missing closing bracket
        Node::Unclosed(leaf) => {
            Value::Undefined(Undef::Termination{
                expected: leaf.val,
                pos: leaf.pos,
            })
        }

        // Undefined token
        Node::UnexpectedToken(leaf) => {
            Value::Undefined(Undef::Token{
                token: leaf.val.clone(),
                pos: leaf.pos
            })
        }
    }
}

/// Evaluates an AST nodes that are supposed to resolve into names
///
/// This functions resolves values or tuple at the left side of an
/// assignment operation or at the left side of a function definition.
/// Those values can be either an identifier or a tuple of identifiers.
fn eval_names(node: &Node) -> Value {
    match node {

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

            eval_pairing(name1, name2)
        }

        // Any other value will return and undefined value
        node => Value::Undefined(Undef::Name{ pos: node.position() }),
    }
}





// ----------------------------------------------------------------------------
//  UNARY OPERATIONS
// ----------------------------------------------------------------------------

/// `+X` — identity for ALL Swan value types. (#10)
///
/// In JS this is literally `return this.operand.evaluate(context)` with no
/// type guard.  Restricting it to Numb only was a bug.
fn eval_affirm(val: Value) -> Value {
    val
}

/// `-X` — negation.
///
/// Currently only defined for `Numb`. When tuples are supported the operation
/// should be mapped element-wise over all items. (#11)
fn eval_negate(val: Value, pos: usize) -> Value {
    match val {
        Value::Numb(n) => Value::Numb(-n),
        other => Value::Undefined(Undef::UnaryOperation {
            op:  "negation",
            val: Box::new(other),
            pos,
        }),
    }
}

fn eval_lifting(name: Value, scope: &Scope, pos: usize) -> Value {
    match name {
        Value::Name(n) => scope.lift(&n),
        other => Value::Undefined(Undef::UnaryOperation {
            op:  "lifting",
            val: Box::new(other),
            pos,
        }),
    }
}





// ----------------------------------------------------------------------------
//  CORE BINARY OPERATIONS
// ----------------------------------------------------------------------------

/// `func arg` — applies a callable Swan value to an argument.
///
/// Dispatches on the type of `func`:
///
/// - `Value::Builtin(f)` — calls the native Rust function directly.
/// - `Value::Func { ... }` — user-defined function (not yet implemented).
/// - Anything else — returns `Undefined`.
fn eval_apply(func: Value, arg: Value, pos: usize) -> Value {
    func.map(move |f| eval_apply_1(f, arg.clone(), pos))
}

fn eval_apply_1(func: Value, arg: Value, pos: usize) -> Value {
    // // 1. Broadcast over Tuple on the left: applying each function in a tuple to `arg`
    // if matches!(func, Value::Tuple(_)) {
    //     let results: Vec<Value> = func
    //         .items()
    //         .map(|f| eval_apply(f, arg.clone(), pos))
    //         .collect();
    //     return match results.len() {
    //         0 => Value::Nothing,
    //         1 => results[0].clone(),
    //         _ => Value::Tuple(Rc::new(results)),
    //     };
    // }

    // 2. Dispatch single-item application by type
    match func {
        Value::Builtin(ref f) => f.call(arg),

        Value::Func { params, body, closure } => {
            let local_scope = closure.new_child();
            eval_assignment(&local_scope, &params, &arg);
            eval(&body, &local_scope)
        }

        // --- List Application ---
        Value::List(list) => {
            if matches!(arg, Value::Tuple(_)) {
                let items: Vec<Value> = arg
                    .items()
                    .map(|idx| eval_apply_list(&list, idx, pos))
                    .collect();
                match items.len() {
                    0 => Value::Nothing,
                    1 => items[0].clone(),
                    _ => Value::Tuple(Rc::new(items)),
                }
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
                match items.len() {
                    0 => Value::Nothing,
                    1 => items[0].clone(),
                    _ => Value::Tuple(Rc::new(items)),
                }
            } else {
                eval_apply_namespace(&scope, arg, pos)
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

/// Applies a List to a single index argument `n`.
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

/// Applies a Namespace to a single key argument `t`.
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

/// `X , Y` - pairing operation
fn eval_pairing(lhv: Value, rhv: Value) -> Value {
    let mut vec: Vec<_> = lhv.items().chain(rhv.items()).collect();
    match vec.len() {
        0 => Value::Nothing,
        1 => vec.pop().unwrap(),
        _ => Value::Tuple(Rc::new(vec)),
    }
}

/// `X = Y` - assignment operation
fn eval_assignment(scope: &Scope, names: &Value, values: &Value) {
    let mut names_iter = names.items();
    let mut values_iter = values.items();
    let mut last_name: Rc<str> = "__uname__".into();
    loop {
        let name = names_iter.next();
        let value = values_iter.next();
        match (name, value) {

            (Some(Value::Name(name)), Some(value)) => {
                scope.set(name.as_ref(), value);
                last_name = name;
            }

            (Some(Value::Name(name)), None) => {
                scope.set(name.as_ref(), Value::Nothing);
                last_name = name;
            }

            (None, Some(value)) => {
                let prev_value = scope.get(last_name.as_ref());
                scope.set(last_name.as_ref(), eval_pairing(prev_value, value));
            }

            (None, None) => {
                break;
            }

            _ => {
                scope.set("__uname__", Value::Nothing);
            }
        }
    }
}





// ----------------------------------------------------------------------------
//  Arithmetic binary operations
// ----------------------------------------------------------------------------

/// `X + Y` — type-homogeneous sum.
///
/// Matches the JS `SumOperation`: both operands must be the *same* Swan type
/// and that type must define a `sum` operation.  Mismatched types always
/// produce `Undefined`. (#12)
///
/// Currently implemented:
///   Numb  + Numb  → number addition
///   Text  + Text  → string concatenation
///   List  + List  → list concatenation
///   Namespace + Namespace → shallow merge (right wins on collision)
///
/// When tuples are supported, the operation should iterate `iterPairs` over
/// the two tuples and apply element-wise, assembling the results into a new
/// Tuple. (#11)
fn eval_add(lhv: Value, rhv: Value, pos: usize) -> Value {
    match (lhv, rhv) {

        (Value::Numb(a), Value::Numb(b)) => Value::Numb(a + b),

        (Value::Bool(a), Value::Bool(b)) => Value::Bool(a || b),

        (Value::Text(a), Value::Text(b)) => {
            // Concatenate two Rc<str> values without going through String twice.
            let mut s = String::with_capacity(a.len() + b.len());
            s.push_str(&a);
            s.push_str(&b);
            Value::Text(s.into())
        }

        (Value::List(a), Value::List(b)) => {
            // Concatenate two Rc<Vec<Value>>: clone items from both sides.
            let mut v = Vec::with_capacity(a.len() + b.len());
            v.extend_from_slice(&a);
            v.extend_from_slice(&b);
            Value::List(Rc::new(v))
        }

        (lhv, rhv) => Value::Undefined(Undef::BinaryOperation {
            op:  "sum",
            lhv: Box::new(lhv),
            rhv: Box::new(rhv),
            pos,
        }),
    }
}

/// `X - Y` — subtraction.
///
/// Currently implemented:
///   Numb  - Numb  → number subtraction
///
/// When tuples are supported, the operation should iterate `iterPairs` over
/// the two tuples and apply element-wise, assembling the results into a new
/// Tuple. (#11)
fn eval_subtraction(lhv: Value, rhv: Value, pos: usize) -> Value {
    match (lhv, rhv) {

        (Value::Numb(a), Value::Numb(b)) => Value::Numb(a - b),

        (lhv, rhv) => Value::Undefined(Undef::BinaryOperation {
            op:  "subtraction",
            lhv: Box::new(lhv),
            rhv: Box::new(rhv),
            pos,
        }),
    }
}

/// `X * Y` — product
///
/// Currently implemented:
///   Numb * Numb  → number product
///
fn eval_product(lhv: Value, rhv: Value, pos: usize) -> Value {
    match (lhv, rhv) {

        (Value::Numb(a), Value::Numb(b)) => Value::Numb(a * b),

        (Value::Bool(a), Value::Bool(b)) => Value::Bool(a && b),

        (lhv, rhv) => Value::Undefined(Undef::BinaryOperation {
            op:  "product",
            lhv: Box::new(lhv),
            rhv: Box::new(rhv),
            pos,
        }),
    }
}

/// `X / Y` — division
///
/// Currently implemented:
///   Numb / Numb  → number division
///
fn eval_division(lhv: Value, rhv: Value, pos: usize) -> Value {
    match (lhv, rhv) {

        (Value::Numb(a), Value::Numb(b)) => Value::Numb(a / b),

        (lhv, rhv) => Value::Undefined(Undef::BinaryOperation {
            op:  "division",
            lhv: Box::new(lhv),
            rhv: Box::new(rhv),
            pos,
        }),
    }
}

/// `X % Y` — modulo
///
/// Currently implemented:
///   Numb % Numb  → number modulo
///
fn eval_modulo(lhv: Value, rhv: Value, pos: usize) -> Value {
    match (lhv, rhv) {

        (Value::Numb(a), Value::Numb(b)) => Value::Numb(a % b),

        (lhv, rhv) => Value::Undefined(Undef::BinaryOperation {
            op:  "modulo",
            lhv: Box::new(lhv),
            rhv: Box::new(rhv),
            pos,
        }),
    }
}

/// `X ^ Y` — power
///
/// Currently implemented:
///   Numb ^ Numb  → number exponentiation
///
fn eval_power(lhv: Value, rhv: Value, pos: usize) -> Value {
    match (lhv, rhv) {

        (Value::Numb(a), Value::Numb(b)) => Value::Numb(a.powf(b)),

        (lhv, rhv) => Value::Undefined(Undef::BinaryOperation {
            op:  "power",
            lhv: Box::new(lhv),
            rhv: Box::new(rhv),
            pos,
        }),
    }
}





// ----------------------------------------------------------------------------
//  Comparison operations
// ----------------------------------------------------------------------------

/// `X == Y` — equal-to operation 
fn eval_equal_to(lhv: Value, rhv: Value) -> Value {
    Value::Bool(lhv == rhv)
}

/// `X != Y` — not-equal-to operation
fn eval_not_equal_to(lhv: Value, rhv: Value) -> Value {
    Value::Bool(lhv != rhv)
}

/// `X > Y` — greater-than operation
fn eval_greater_than(lhv: Value, rhv: Value, pos: usize) -> Value {
    if let Some(cmp) = lhv.partial_cmp(&rhv) {
        Value::Bool(matches!(cmp, Ordering::Greater))
    } else {
        Value::Undefined(Undef::BinaryOperation{
            op:  "order",
            lhv: Box::new(lhv),
            rhv: Box::new(rhv),
            pos: pos,
        })
    }
}

/// `X < Y` — less-than operation
fn eval_less_than(lhv: Value, rhv: Value, pos: usize) -> Value {
    if let Some(cmp) = lhv.partial_cmp(&rhv) {
        Value::Bool(matches!(cmp, Ordering::Less))
    } else {
        Value::Undefined(Undef::BinaryOperation{
            op:  "order",
            lhv: Box::new(lhv),
            rhv: Box::new(rhv),
            pos: pos,
        })
    }
}

/// `X >= Y` — greater-than-or-equal-to operation
fn eval_greater_than_or_equal_to(lhv: Value, rhv: Value, pos: usize) -> Value {

    // >= is evaluated as !<
    match eval_less_than(lhv, rhv, pos) {
        Value::Bool(b) => Value::Bool(!b),
        undef => undef,
    }
}

/// `X <= Y` — less-than-or-equal-to operation
fn eval_less_than_or_equal_to(lhv: Value, rhv: Value, pos: usize) -> Value {

    // <= is evaluated as !>
    match eval_greater_than(lhv, rhv, pos) {
        Value::Bool(b) => Value::Bool(!b),
        undef => undef,
    }
}





// ----------------------------------------------------------------------------
//  Function composition operations
// ----------------------------------------------------------------------------

/// `f << g` - function composition
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

    fn evaluate_bool(src: &str) -> bool {
        match evaluate(src) {
            Value::Bool(b) => b,
            _ => panic!("Boolean epression expected, found {}", src),
        }
    }

    mod literals {
        use super::*;

        #[test]
        fn number() {
            assert_eq!(evaluate("7.2"), Value::Numb(7.2));
        }

        #[test]
        fn string() {
            assert_eq!(evaluate("\"abc\""), Value::Text("abc".into()));
        }

        #[test]
        fn nothing() {

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
        fn tuple() {
            assert_eq!(evaluate("7 , 2 , 9"), Value::Tuple(Rc::new(vec![
                        Value::Numb(7.0), 
                        Value::Numb(2.0),
                        Value::Numb(9.0)
            ])));

            assert_eq!(evaluate("7 , (2,9)"), Value::Tuple(Rc::new(vec![
                        Value::Numb(7.0), 
                        Value::Numb(2.0),
                        Value::Numb(9.0)
            ])));

            assert_eq!(evaluate("(7,2) , 9"), Value::Tuple(Rc::new(vec![
                        Value::Numb(7.0), 
                        Value::Numb(2.0),
                        Value::Numb(9.0)
            ])));

            assert_eq!(evaluate("(7,2) , (9,3)"), Value::Tuple(Rc::new(vec![
                        Value::Numb(7.0), 
                        Value::Numb(2.0),
                        Value::Numb(9.0),
                        Value::Numb(3.0)
            ])));

            assert_eq!(evaluate("7 , 2 , () , 9"), Value::Tuple(Rc::new(vec![
                        Value::Numb(7.0), 
                        Value::Numb(2.0),
                        Value::Numb(9.0)
            ])));

            assert_eq!(evaluate("() , ()"), Value::Nothing);
            assert_eq!(evaluate("1 , ()"), Value::Numb(1.0));
            assert_eq!(evaluate("() , 1"), Value::Numb(1.0));
        }
    
        #[test]
        fn list() {
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
        fn namespace() {
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
    }

    mod core_operations {
        use super::*;

        #[test]
        fn function_definition() {

            assert_matches!(evaluate("(x, y) -> x+y"), Value::Func{params:_, body:_, closure:_});

            // Invalid left-hand names.
            assert_eq!(evaluate("(x, 12) -> 2*x"), Value::Undefined(Undef::BinaryOperation{
                op: "function_definition",
                lhv: Box::new(Value::Undefined(Undef::Name{pos:4})),
                rhv: Box::new(Value::Nothing),
                pos: 8,
            }));
        }

        #[test]
        fn function_application() {
            assert_eq!(evaluate("f = (a) -> 2*a , f 5"), Value::Numb(10.0));
            assert_eq!(evaluate("f = (a,b) -> a+b , f(3,7)"), Value::Numb(10.0));
            assert_eq!(evaluate("f = (a,b,c) -> (a,b)*c , f(2,3,4,5)"), Value::Tuple(Rc::new(vec![
                Value::Numb(8.0),
                Value::Numb(15.0),
            ])));
            assert_eq!(
                evaluate("(x->2*x , x->3*x) 4"), 
                Value::Tuple(Rc::new(vec![Value::Numb(8.0), Value::Numb(12.0)]))
            );
        }

        #[test]
        fn list_application() {
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
        }

        #[test]
        fn namespace_application() {
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
        }

        #[test]
        fn assignment() {
            // `x = 5` evaluates to Nothing; the binding is only visible through
            // a subsequent lookup. Because `=` binds tighter than `,`, this
            // parses as `(x = 5), x`, letting us observe the stored value.
            assert_eq!(evaluate("x = 5, x"), Value::Numb(5.0));

            // Assignment itself always yields Nothing.
            assert_eq!(evaluate("x = 5"), Value::Nothing);

            // Destructuring assignment: `(a, b) = (1, 2)` binds a=1, b=2.
            assert_eq!(
                evaluate("(a, b) = (1, 2), a, b"),
                Value::Tuple(Rc::new(vec![Value::Numb(1.0), Value::Numb(2.0)]))
            );

            // Re-assigning an existing name overwrites the previous value.
            assert_eq!(evaluate("x = 1, x = 2, x"), Value::Numb(2.0));

            // Invalid left-hand names.
            assert_eq!(evaluate("(x, 12) = (1, 2)"), Value::Undefined(Undef::BinaryOperation{
                op: "assignment",
                lhv: Box::new(Value::Undefined(Undef::Name{pos:4})),
                rhv: Box::new(Value::Nothing),
                pos: 8,
            }));
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
            assert_eq!(evaluate("(x, 12) : (1, 2)"), Value::Undefined(Undef::BinaryOperation{
                op: "labelling",
                lhv: Box::new(Value::Undefined(Undef::Name{pos:4})),
                rhv: Box::new(Value::Nothing),
                pos: 8,
            }));
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
            assert_eq!(evaluate("f = a -> 2*a , (2,3,4) => f"), Value::Tuple(Rc::new(vec![
                Value::Numb(4.0),
                Value::Numb(6.0),
                Value::Numb(8.0),
            ])));
            assert_eq!(evaluate("f = a -> 2*a , 2 => f"), Value::Numb(4.0));
            assert_eq!(evaluate("f = a -> 2*a , () => f"), Value::Nothing);
            assert_eq!(evaluate("f = a -> () , (1,2,3) => f"), Value::Nothing);
        }
    }

    mod unary_operations {
        use super::*;

        #[test]
        fn affirm() {
            // number
            assert_eq!(evaluate("+2"), Value::Numb(2.0));

            // +X should be a pure identity — it must NOT return Undef for
            // non-numeric types (was the bug described in review item #10).
            let scope = Scope::new();
            let text_val = Value::Text("hello".into());

            // Build a tiny AST manually: AffirmOp wrapping a Text literal.
            let text_leaf = crate::ast::Node::Text(crate::ast::Leaf { pos: 0, val: "hello".into() });
            let affirm_node = crate::ast::Node::AffirmOp(crate::ast::UnaryOp {
                pos: 0,
                opd: Box::new(text_leaf),
            });
            assert_eq!(eval(&affirm_node, &scope), text_val);
        }

        #[test]
        fn negate() {
            assert_eq!(evaluate("-2"), Value::Numb(-2.0));
            assert_eq!(evaluate("-(2, 3, 4)"), Value::Tuple(Rc::new(vec![
                Value::Numb(-2.0),
                Value::Numb(-3.0),
                Value::Numb(-4.0)
            ])));
        }

        #[test]
        fn logical_negation() {
            assert_eq!(evaluate("!TRUE"), Value::Bool(false));
            assert_eq!(evaluate("!FALSE"), Value::Bool(true));
            assert_eq!(evaluate("!1"), Value::Bool(false));
            assert_eq!(evaluate("!0"), Value::Bool(true));
            assert_eq!(evaluate("!!0"), Value::Bool(false));
            assert_eq!(evaluate("!!1"), Value::Bool(true));
        }

        #[test]
        fn lifting() {
            assert_eq!(evaluate("{x=1}.{x=2}.x"), Value::Numb(2.0));
            assert_eq!(evaluate("{x=1}.{x=2}.$x"), Value::Numb(1.0));
        }
    }

    mod arithmetic_operations {
        use super::*;

        #[test]
        fn sum() {
            assert_eq!(evaluate("() + ()"), Value::Nothing);

            assert_eq!(evaluate("1 + 2"), Value::Numb(3.0));

            assert_eq!(evaluate("TRUE + TRUE"), Value::Bool(true));
            assert_eq!(evaluate("TRUE + FALSE"), Value::Bool(true));
            assert_eq!(evaluate("FALSE + TRUE"), Value::Bool(true));
            assert_eq!(evaluate("FALSE + FALSE"), Value::Bool(false));

            assert_eq!(evaluate("\"abc\" + \"def\""), Value::Text("abcdef".into()));

            assert_eq!(evaluate("[1,2,3] + [4,5,6]"), Value::List(Rc::new(vec![
                        Value::Numb(1.0),
                        Value::Numb(2.0),
                        Value::Numb(3.0),
                        Value::Numb(4.0),
                        Value::Numb(5.0),
                        Value::Numb(6.0)
                    ])));

            assert_eq!(evaluate("(1, 2) + (3, 4)"), Value::Tuple(Rc::new(vec![
                        Value::Numb(4.0),
                        Value::Numb(6.0),
                    ])));

            match evaluate("1 + \"abc\"") {
                Value::Undefined(Undef::BinaryOperation { op, lhv, rhv, pos }) => {
                    assert_eq!(op, "sum");
                    assert_eq!(*lhv, Value::Numb(1.0));
                    assert_eq!(*rhv, Value::Text("abc".into()));
                    assert_eq!(pos, 2);
                }
                other => panic!("expected Undefined::BinaryOperation, got {:?}", other),
            }
        }

        #[test]
        fn subtraction() {
            assert_eq!(evaluate("() - ()"), Value::Nothing);

            assert_eq!(evaluate("7 - 2"), Value::Numb(5.0));

            assert_eq!(evaluate("(10, 20) - (1, 2)"), Value::Tuple(Rc::new(vec![
                        Value::Numb(9.0),
                        Value::Numb(18.0),
                    ])));

            match evaluate("1 - \"abc\"") {
                Value::Undefined(Undef::BinaryOperation { op, lhv, rhv, pos }) => {
                    assert_eq!(op, "subtraction");
                    assert_eq!(*lhv, Value::Numb(1.0));
                    assert_eq!(*rhv, Value::Text("abc".into()));
                    assert_eq!(pos, 2);
                }
                other => panic!("expected Undefined::BinaryOperation, got {:?}", other),
            }
        }

        #[test]
        fn product() {
            assert_eq!(evaluate("() * ()"), Value::Nothing);

            assert_eq!(evaluate("7 * 2"), Value::Numb(14.0));

            assert_eq!(evaluate("TRUE * TRUE"), Value::Bool(true));
            assert_eq!(evaluate("TRUE * FALSE"), Value::Bool(false));
            assert_eq!(evaluate("FALSE * TRUE"), Value::Bool(false));
            assert_eq!(evaluate("FALSE * FALSE"), Value::Bool(false));

            assert_eq!(evaluate("(2, 3) * (4, 5)"), Value::Tuple(Rc::new(vec![
                        Value::Numb(8.0),
                        Value::Numb(15.0),
                    ])));

            match evaluate("1 * \"abc\"") {
                Value::Undefined(Undef::BinaryOperation { op, lhv, rhv, pos }) => {
                    assert_eq!(op, "product");
                    assert_eq!(*lhv, Value::Numb(1.0));
                    assert_eq!(*rhv, Value::Text("abc".into()));
                    assert_eq!(pos, 2);
                }
                other => panic!("expected Undefined::BinaryOperation, got {:?}", other),
            }
        }

        #[test]
        fn division() {
            assert_eq!(evaluate("() / ()"), Value::Nothing);

            assert_eq!(evaluate("7 / 2"), Value::Numb(3.5));
            assert_eq!(evaluate("7 / 0"), Value::Numb(f64::INFINITY));

            assert_eq!(evaluate("(10, 20) / (2, 5)"), Value::Tuple(Rc::new(vec![
                        Value::Numb(5.0),
                        Value::Numb(4.0),
                    ])));

            match evaluate("1 / \"abc\"") {
                Value::Undefined(Undef::BinaryOperation { op, lhv, rhv, pos }) => {
                    assert_eq!(op, "division");
                    assert_eq!(*lhv, Value::Numb(1.0));
                    assert_eq!(*rhv, Value::Text("abc".into()));
                    assert_eq!(pos, 2);
                }
                other => panic!("expected Undefined::BinaryOperation, got {:?}", other),
            }
        }

        #[test]
        fn modulo() {
            assert_eq!(evaluate("() % ()"), Value::Nothing);

            assert_eq!(evaluate("8 % 3"), Value::Numb(2.0));

            assert_eq!(evaluate("(10, 20) % (3, 7)"), Value::Tuple(Rc::new(vec![
                        Value::Numb(1.0),
                        Value::Numb(6.0),
                    ])));

            match evaluate("1 % \"abc\"") {
                Value::Undefined(Undef::BinaryOperation { op, lhv, rhv, pos }) => {
                    assert_eq!(op, "modulo");
                    assert_eq!(*lhv, Value::Numb(1.0));
                    assert_eq!(*rhv, Value::Text("abc".into()));
                    assert_eq!(pos, 2);
                }
                other => panic!("expected Undefined::BinaryOperation, got {:?}", other),
            }
        }

        #[test]
        fn power() {
            assert_eq!(evaluate("() ^ ()"), Value::Nothing);

            assert_eq!(evaluate("2 ^ 3"), Value::Numb(8.0));

            assert_eq!(evaluate("(10, 2) ^ (2, 4)"), Value::Tuple(Rc::new(vec![
                        Value::Numb(100.0),
                        Value::Numb(16.0),
                    ])));

            match evaluate("1 ^ \"abc\"") {
                Value::Undefined(Undef::BinaryOperation { op, lhv, rhv, pos }) => {
                    assert_eq!(op, "power");
                    assert_eq!(*lhv, Value::Numb(1.0));
                    assert_eq!(*rhv, Value::Text("abc".into()));
                    assert_eq!(pos, 2);
                }
                other => panic!("expected Undefined::BinaryOperation, got {:?}", other),
            }
        }
    }

    mod comparison_operations {
        use super::*;

        #[test]
        fn equal_to() {

            // bool
            assert_eq!(evaluate_bool("TRUE == TRUE")  , true);
            assert_eq!(evaluate_bool("FALSE == FALSE"), true);
            assert_eq!(evaluate_bool("TRUE == FALSE") , false);

            // numb
            assert_eq!(evaluate_bool("1 == 1"), true);
            assert_eq!(evaluate_bool("1 == 2"), false);

            // text
            assert_eq!(evaluate_bool("\"abc\" == \"abc\""), true);
            assert_eq!(evaluate_bool("\"abc\" == \"xy\"") , false);

            // list
            assert_eq!(evaluate_bool("[1,2,3] == [1,2,3]"), true);
            assert_eq!(evaluate_bool("[1,2,3] == [1,2]")  , false);

            // namespace
            assert_eq!(evaluate_bool("{x:1,y:2} == {x:1,y:2}"), true);
            assert_eq!(evaluate_bool("{x:1,y:2} == {x:1,z:2}"), false);

            // tuple
            assert_eq!(evaluate_bool("(1,2,3) == (1,2,3)"), true);
            assert_eq!(evaluate_bool("(1,2,3) == (1,2)")  , false);

            // nothing
            assert_eq!(evaluate_bool("() == ()"), true);
            assert_eq!(evaluate_bool("() == FALSE  "), false);
            assert_eq!(evaluate_bool("() == 1      "), false);
            assert_eq!(evaluate_bool("() == \"abc\""), false);
            assert_eq!(evaluate_bool("() == [1,2,3]"), false);
            assert_eq!(evaluate_bool("() == {x: 10}"), false);

            // undefined
            assert_eq!(evaluate_bool("(-[]) == (-[])"), true);
            assert_eq!(evaluate_bool("(-[]) == (-{})"), false);
            assert_eq!(evaluate_bool("([]^2) == ([]^2)"), true);
            assert_eq!(evaluate_bool("([]^2) == ({}^2)"), false);
            assert_eq!(evaluate_bool("(@) == (@)"), true);
            assert_eq!(evaluate_bool("(~) == (@)"), false);
            assert_eq!(evaluate("(1,2"), evaluate("(1,2 "));
        }

        #[test]
        fn not_equal_to() {

            // bool
            assert_eq!(evaluate_bool("TRUE != TRUE"), false);
            assert_eq!(evaluate_bool("FALSE != FALSE"), false);
            assert_eq!(evaluate_bool("TRUE != FALSE"), true);

            // numb
            assert_eq!(evaluate_bool("1 != 1"), false);
            assert_eq!(evaluate_bool("1 != 2"), true);

            // text
            assert_eq!(evaluate_bool("\"abc\" != \"abc\""), false);
            assert_eq!(evaluate_bool("\"abc\" != \"xy\"") , true);

            // list
            assert_eq!(evaluate_bool("[1,2,3] != [1,2,3]"), false);
            assert_eq!(evaluate_bool("[1,2,3] != [1,2]")  , true);

            // namespace
            assert_eq!(evaluate_bool("{x:1,y:2} != {x:1,y:2}"), false);
            assert_eq!(evaluate_bool("{x:1,y:2} != {x:1,z:2}"), true);

            // tuple
            assert_eq!(evaluate_bool("(1,2,3) != (1,2,3)"), false);
            assert_eq!(evaluate_bool("(1,2,3) != (1,2)")  , true);

            // nothing
            assert_eq!(evaluate_bool("() != ()"), false);
            assert_eq!(evaluate_bool("() != FALSE  "), true);
            assert_eq!(evaluate_bool("() != 1      "), true);
            assert_eq!(evaluate_bool("() != \"abc\""), true);
            assert_eq!(evaluate_bool("() != [1,2,3]"), true);
            assert_eq!(evaluate_bool("() != {x: 10}"), true);

            // undefined
            assert_eq!(evaluate_bool("(-[]) != (-[])"), false);
            assert_eq!(evaluate_bool("(-[]) != (-{})"), true);
            assert_eq!(evaluate_bool("([]^2) != ([]^2)"), false);
            assert_eq!(evaluate_bool("([]^2) != ({}^2)"), true);
            assert_eq!(evaluate_bool("(@) != (@)"), false);
            assert_eq!(evaluate_bool("(~) != (@)"), true);
            assert_ne!(evaluate("(1,2"), evaluate("[1,2"));
        }

        #[test]
        fn greater_than() {

            // bool
            assert_eq!(evaluate_bool("TRUE > TRUE  "), false);
            assert_eq!(evaluate_bool("FALSE > FALSE"), false);
            assert_eq!(evaluate_bool("TRUE > FALSE "), true);
            assert_eq!(evaluate_bool("FALSE > TRUE "), false);

            // numb
            assert_eq!(evaluate_bool("1 > 1"), false);
            assert_eq!(evaluate_bool("1 > 2"), false);
            assert_eq!(evaluate_bool("2 > 1"), true);
            assert_eq!(evaluate_bool("2 > 2"), false);

            // text 
            assert_eq!(evaluate_bool("\"abc\" > \"abc\""), false);
            assert_eq!(evaluate_bool("\"abc\" > \"xyz\""), false);
            assert_eq!(evaluate_bool("\"xyz\" > \"abc\""), true);

            // list
            assert_eq!(evaluate_bool("[1,2,3] > [1,2,3]"), false);
            assert_eq!(evaluate_bool("[1,2,3] > [1,3,4]"), false);
            assert_eq!(evaluate_bool("[1,3,4] > [1,2,3]"), true);
            assert_eq!(evaluate_bool("[1,2  ] > [1,2,3]"), false);
            assert_eq!(evaluate_bool("[1,2,3] > [1,2  ]"), true);
            
            // TODO: namespace
            
            // tuple
            assert_eq!(evaluate_bool("(1,2,3) > (1,2,3)"), false);
            assert_eq!(evaluate_bool("(1,2,3) > (1,3,4)"), false);
            assert_eq!(evaluate_bool("(1,3,4) > (1,2,3)"), true);
            assert_eq!(evaluate_bool("(1,2  ) > (1,2,3)"), false);
            assert_eq!(evaluate_bool("(1,2,3) > (1,2  )"), true);

            // nothing
            assert_eq!(evaluate_bool("() > ()"), false);
            assert_eq!(evaluate_bool("() > FALSE  "), false);
            assert_eq!(evaluate_bool("() > 1      "), false);
            assert_eq!(evaluate_bool("() > \"abc\""), false);
            assert_eq!(evaluate_bool("() > [1,2,3]"), false);
            assert_eq!(evaluate_bool("() > {x: 10}"), false);
            assert_eq!(evaluate_bool("FALSE   > ()"), true);
            assert_eq!(evaluate_bool("1       > ()"), true);
            assert_eq!(evaluate_bool("\"abc\" > ()"), true);
            assert_eq!(evaluate_bool("[1,2,3] > ()"), true);
            assert_eq!(evaluate_bool("{x: 10} > ()"), true);

            // undefined order
            assert_eq!(evaluate("TRUE > 1"), Value::Undefined(Undef::BinaryOperation{
                op : "order", 
                lhv: Box::new(Value::Bool(true)), 
                rhv: Box::new(Value::Numb(1.0)), 
                pos: 5
            }));
        }

        #[test]
        fn less_than() {

            // bool
            assert_eq!(evaluate_bool("TRUE < TRUE  "), false);
            assert_eq!(evaluate_bool("FALSE < FALSE"), false);
            assert_eq!(evaluate_bool("TRUE < FALSE "), false);
            assert_eq!(evaluate_bool("FALSE < TRUE "), true);

            // numb
            assert_eq!(evaluate_bool("1 < 1"), false);
            assert_eq!(evaluate_bool("1 < 2"), true);
            assert_eq!(evaluate_bool("2 < 1"), false);
            assert_eq!(evaluate_bool("2 < 2"), false);

            // text 
            assert_eq!(evaluate_bool("\"abc\" < \"abc\""), false);
            assert_eq!(evaluate_bool("\"abc\" < \"xyz\""), true);
            assert_eq!(evaluate_bool("\"xyz\" < \"abc\""), false);

            // list
            assert_eq!(evaluate_bool("[1,2,3] < [1,2,3]"), false);
            assert_eq!(evaluate_bool("[1,2,3] < [1,3,4]"), true);
            assert_eq!(evaluate_bool("[1,3,4] < [1,2,3]"), false);
            assert_eq!(evaluate_bool("[1,2  ] < [1,2,3]"), true);
            assert_eq!(evaluate_bool("[1,2,3] < [1,2  ]"), false);
            
            // TODO: namespace
            
            // tuple
            assert_eq!(evaluate_bool("(1,2,3) < (1,2,3)"), false);
            assert_eq!(evaluate_bool("(1,2,3) < (1,3,4)"), true);
            assert_eq!(evaluate_bool("(1,3,4) < (1,2,3)"), false);
            assert_eq!(evaluate_bool("(1,2  ) < (1,2,3)"), true);
            assert_eq!(evaluate_bool("(1,2,3) < (1,2  )"), false);

            // nothing
            assert_eq!(evaluate_bool("() < ()"), false);
            assert_eq!(evaluate_bool("() < FALSE  "), true);
            assert_eq!(evaluate_bool("() < 1      "), true);
            assert_eq!(evaluate_bool("() < \"abc\""), true);
            assert_eq!(evaluate_bool("() < [1,2,3]"), true);
            assert_eq!(evaluate_bool("() < {x: 10}"), true);
            assert_eq!(evaluate_bool("FALSE   < ()"), false);
            assert_eq!(evaluate_bool("1       < ()"), false);
            assert_eq!(evaluate_bool("\"abc\" < ()"), false);
            assert_eq!(evaluate_bool("[1,2,3] < ()"), false);
            assert_eq!(evaluate_bool("{x: 10} < ()"), false);

            // undefined order
            assert_eq!(evaluate("TRUE < 1"), Value::Undefined(Undef::BinaryOperation{
                op : "order", 
                lhv: Box::new(Value::Bool(true)), 
                rhv: Box::new(Value::Numb(1.0)), 
                pos: 5
            }));
        }

        #[test]
        fn greater_than_or_equal_to() {

            // bool
            assert_eq!(evaluate_bool("TRUE >= TRUE  "), true);
            assert_eq!(evaluate_bool("FALSE >= FALSE"), true);
            assert_eq!(evaluate_bool("TRUE >= FALSE "), true);
            assert_eq!(evaluate_bool("FALSE >= TRUE "), false);

            // numb
            assert_eq!(evaluate_bool("1 >= 1"), true);
            assert_eq!(evaluate_bool("1 >= 2"), false);
            assert_eq!(evaluate_bool("2 >= 1"), true);
            assert_eq!(evaluate_bool("2 >= 2"), true);

            // text 
            assert_eq!(evaluate_bool("\"abc\" >= \"abc\""), true);
            assert_eq!(evaluate_bool("\"abc\" >= \"xyz\""), false);
            assert_eq!(evaluate_bool("\"xyz\" >= \"abc\""), true);

            // list
            assert_eq!(evaluate_bool("[1,2,3] >= [1,2,3]"), true);
            assert_eq!(evaluate_bool("[1,2,3] >= [1,3,4]"), false);
            assert_eq!(evaluate_bool("[1,3,4] >= [1,2,3]"), true);
            assert_eq!(evaluate_bool("[1,2  ] >= [1,2,3]"), false);
            assert_eq!(evaluate_bool("[1,2,3] >= [1,2  ]"), true);
            
            // TODO: namespace
            
            // tuple
            assert_eq!(evaluate_bool("(1,2,3) >= (1,2,3)"), true);
            assert_eq!(evaluate_bool("(1,2,3) >= (1,3,4)"), false);
            assert_eq!(evaluate_bool("(1,3,4) >= (1,2,3)"), true);
            assert_eq!(evaluate_bool("(1,2  ) >= (1,2,3)"), false);
            assert_eq!(evaluate_bool("(1,2,3) >= (1,2  )"), true);

            // nothing
            assert_eq!(evaluate_bool("() >= ()"), true);
            assert_eq!(evaluate_bool("() >= FALSE  "), false);
            assert_eq!(evaluate_bool("() >= 1      "), false);
            assert_eq!(evaluate_bool("() >= \"abc\""), false);
            assert_eq!(evaluate_bool("() >= [1,2,3]"), false);
            assert_eq!(evaluate_bool("() >= {x: 10}"), false);
            assert_eq!(evaluate_bool("FALSE   >= ()"), true);
            assert_eq!(evaluate_bool("1       >= ()"), true);
            assert_eq!(evaluate_bool("\"abc\" >= ()"), true);
            assert_eq!(evaluate_bool("[1,2,3] >= ()"), true);
            assert_eq!(evaluate_bool("{x: 10} >= ()"), true);

            // undefined order
            assert_eq!(evaluate("TRUE >= 1"), Value::Undefined(Undef::BinaryOperation{
                op : "order", 
                lhv: Box::new(Value::Bool(true)), 
                rhv: Box::new(Value::Numb(1.0)), 
                pos: 5
            }));
        }

        #[test]
        fn less_than_or_equal_to() {

            // bool
            assert_eq!(evaluate_bool("TRUE <= TRUE  "), true);
            assert_eq!(evaluate_bool("FALSE <= FALSE"), true);
            assert_eq!(evaluate_bool("TRUE <= FALSE "), false);
            assert_eq!(evaluate_bool("FALSE <= TRUE "), true);

            // numb
            assert_eq!(evaluate_bool("1 <= 1"), true);
            assert_eq!(evaluate_bool("1 <= 2"), true);
            assert_eq!(evaluate_bool("2 <= 1"), false);
            assert_eq!(evaluate_bool("2 <= 2"), true);

            // text 
            assert_eq!(evaluate_bool("\"abc\" <= \"abc\""), true);
            assert_eq!(evaluate_bool("\"abc\" <= \"xyz\""), true);
            assert_eq!(evaluate_bool("\"xyz\" <= \"abc\""), false);

            // list
            assert_eq!(evaluate_bool("[1,2,3] <= [1,2,3]"), true);
            assert_eq!(evaluate_bool("[1,2,3] <= [1,3,4]"), true);
            assert_eq!(evaluate_bool("[1,3,4] <= [1,2,3]"), false);
            assert_eq!(evaluate_bool("[1,2  ] <= [1,2,3]"), true);
            assert_eq!(evaluate_bool("[1,2,3] <= [1,2  ]"), false);
            
            // TODO: namespace
            
            // tuple
            assert_eq!(evaluate_bool("(1,2,3) <= (1,2,3)"), true);
            assert_eq!(evaluate_bool("(1,2,3) <= (1,3,4)"), true);
            assert_eq!(evaluate_bool("(1,3,4) <= (1,2,3)"), false);
            assert_eq!(evaluate_bool("(1,2  ) <= (1,2,3)"), true);
            assert_eq!(evaluate_bool("(1,2,3) <= (1,2  )"), false);

            // nothing
            assert_eq!(evaluate_bool("() <= ()"), true);
            assert_eq!(evaluate_bool("() <= FALSE  "), true);
            assert_eq!(evaluate_bool("() <= 1      "), true);
            assert_eq!(evaluate_bool("() <= \"abc\""), true);
            assert_eq!(evaluate_bool("() <= [1,2,3]"), true);
            assert_eq!(evaluate_bool("() <= {x: 10}"), true);
            assert_eq!(evaluate_bool("FALSE   <= ()"), false);
            assert_eq!(evaluate_bool("1       <= ()"), false);
            assert_eq!(evaluate_bool("\"abc\" <= ()"), false);
            assert_eq!(evaluate_bool("[1,2,3] <= ()"), false);
            assert_eq!(evaluate_bool("{x: 10} <= ()"), false);

            // undefined order
            assert_eq!(evaluate("TRUE <= 1"), Value::Undefined(Undef::BinaryOperation{
                op : "order", 
                lhv: Box::new(Value::Bool(true)), 
                rhv: Box::new(Value::Numb(1.0)), 
                pos: 5
            }));
        }
    }

    mod selection_and_alternative_operations {
        use super::*;

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

            // Tuples: truthy if at least one element converts to true
            assert_eq!(evaluate("(FALSE, 0, 1) ? \"has_true\""), Value::Text("has_true".into()));
            assert_eq!(evaluate("(FALSE, 0, \"\") ? \"all_false\""), Value::Nothing);
        }

        #[test]
        fn alternative() {
            assert_eq!(evaluate("() ; 2"), Value::Numb(2.0));
            assert_eq!(evaluate("1 ; 2"), Value::Numb(1.0));
            assert_eq!(evaluate("FALSE ; 2"), Value::Bool(false));
        }
    }

    mod logic_operations {
        use super::*;

        #[test]
        fn and_op() {
            // True & RHS -> returns RHS
            assert_eq!(evaluate("TRUE & 10"), Value::Numb(10.0));
            assert_eq!(evaluate("TRUE & FALSE"), Value::Bool(false));
            assert_eq!(evaluate("1 & \"hello\""), Value::Text("hello".into()));
            assert_eq!(evaluate("(FALSE, 1) & 42"), Value::Numb(42.0));

            // False & RHS -> short-circuits and returns LHS
            assert_eq!(evaluate("FALSE & 10"), Value::Bool(false));
            assert_eq!(evaluate("0 & 10"), Value::Numb(0.0));
            assert_eq!(evaluate("\"\" & 10"), Value::Text("".into()));
            assert_eq!(evaluate("() & 10"), Value::Nothing);
            assert_eq!(evaluate("(FALSE, 0) & 42"), Value::Tuple(Rc::new(vec![
                Value::Bool(false),
                Value::Numb(0.0),
            ])));
        }

        #[test]
        fn or_op() {
            // True | RHS -> short-circuits and returns LHS
            assert_eq!(evaluate("TRUE | 10"), Value::Bool(true));
            assert_eq!(evaluate("1 | 10"), Value::Numb(1.0));
            assert_eq!(evaluate("\"yes\" | 10"), Value::Text("yes".into()));
            assert_eq!(evaluate("(FALSE, 1) | 42"), Value::Tuple(Rc::new(vec![
                Value::Bool(false),
                Value::Numb(1.0),
            ])));

            // False | RHS -> returns RHS
            assert_eq!(evaluate("FALSE | 10"), Value::Numb(10.0));
            assert_eq!(evaluate("0 | \"fallback\""), Value::Text("fallback".into()));
            assert_eq!(evaluate("\"\" | TRUE"), Value::Bool(true));
            assert_eq!(evaluate("() | 5"), Value::Numb(5.0));
            assert_eq!(evaluate("(FALSE, 0) | 42"), Value::Numb(42.0));
        }
    }

    mod function_composition {
        use super::*;

        #[test]
        fn composition() {
            assert_eq!(evaluate("f = y -> 2*y , g = x -> x+1 , (f<<g)(3)"), Value::Numb(8.0));
        }

        #[test]
        fn reverse_composition() {
            assert_eq!(evaluate("f = y -> 2*y , g = x -> x+1 , (f>>g)(3)"), Value::Numb(7.0));
        }
    }

    mod precedence {
        use super::*;

        #[test]
        fn application_and_subcontexting() {
            assert_eq!(evaluate("ns = {f: x -> 2*x}, ns.f 3"), Value::Numb(6.0));
            assert_eq!(evaluate("ns = {a=10}, f = x -> 2*x, f ns.a"), Value::Numb(20.0));
            assert_eq!(evaluate("f = x -> {y:x}, (f 3).y"), Value::Numb(3.0));
        }
    }
}
