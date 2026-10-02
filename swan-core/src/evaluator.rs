// src/eval.rs

use std::rc::Rc;
use crate::ast::Node;
use crate::value::{Scope, Value, Undef, BuiltinFn};


mod affirm_operation;
mod negation_operation;
mod logical_negation_operation;
mod lifting_operation;

mod pairing_operation;
mod assignment_operation;
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
/// Requires 4 arguments: `($lhv, $rhv, $eval_fn, $pos)`
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
            map_values!(names, |n| lifting_operation::eval(n, &scope, unary.pos))
        }



        // ===== Core Operations =====

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
            map_values!(func, |f| eval_apply(f, args.clone(), bin.pos))
        }

        // Pairing operation: `X , Y`
        Node::PairingOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            pairing_operation::eval(lhv, rhv)
        }

        // Mapping: `T => F`
        Node::MappingOp(bin) => {
            let args = eval(&bin.lho, scope);
            let func = eval(&bin.rho, scope);
            map_values!(args, |a: Value| map_values!(func.clone(), |f| eval_apply(f, a.clone(), bin.pos)))
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
            assignment_operation::eval(scope, &names, &values);
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
            assignment_operation::eval(scope, &names, &values);
            values
        }

        // `ns.expression` — namespace sub-contexting
        Node::SubcontextingOp(bin) => {
            // The left-hand side must evaluate to a `Value::Namespace`.
            // The right-hand side must be an `Identifier` node; its string is used
            // as the lookup key, not its evaluated value.
            let nst = eval(&bin.lho, scope);
            map_values!(nst, |ns| match ns {
                Value::Namespace(ns_scope) => {
                    eval(&bin.rho, &ns_scope)
                }
                lhv => Value::Undefined(Undef::BinaryOperation {
                    op:  "subcontexting",
                    lhv: Box::new(lhv),
                    rhv: Box::new(Value::Nothing),  // rhv can´t be evaluated without a scope
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

        // And: `A & B`
        Node::AndOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            map_pairs!(lhv, rhv, |l: Value, r: Value| {
                if bool::from(l.clone()) { r } else { l }
            })
        }

        // Or: `A | B`
        Node::OrOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            map_pairs!(lhv, rhv, |l: Value, r: Value| {
                if bool::from(l.clone()) { l } else { r }
            })
        }



        // ===== Conditional operations =====

        // Conditional: `A ? B`
        Node::ConditionalOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            map_pairs!(lhv, rhv, |l,r| {
                if bool::from(l) { r } else { Value::Nothing }
            })
        }

        // Alternative: `A ; B`
        Node::AlternativeOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            match lhv {
                Value::Nothing => eval(&bin.rho, scope),
                _ => lhv
            }
        }



        // ===== Composition operations =====

        // Composition: `F << G`
        Node::CompositionOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            map_pairs!(lhv, rhv, |l,r| eval_composition(l, r, bin.pos))
        }

        // Reverse composition: `F >> G`
        Node::ReverseCompositionOp(bin) => {
            let lhv = eval(&bin.lho, scope);
            let rhv = eval(&bin.rho, scope);
            map_pairs!(lhv, rhv, |l,r| eval_composition(r, l, bin.pos))
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
/// - `Value::Builtin(f)` — calls the native Rust function directly.
/// - `Value::Func { ... }` — user-defined function (not yet implemented).
/// - Anything else — returns `Undefined`.
fn eval_apply(func: Value, arg: Value, pos: usize) -> Value {
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




// ----------------------------------------------------------------------------
//  FUNCTION COMPOSITION OPERATION
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
        assert_eq!(evaluate("(x, 12) -> 2*x"), Value::Undefined(Undef::BinaryOperation{
            op: "function_definition",
            lhv: Box::new(Value::Undefined(Undef::Name{pos:4})),
            rhv: Box::new(Value::Nothing),
            pos: 8,
        }));
    }

    #[test]
    fn function_application() {

        // Func
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

        // False & RHS -> short-circuits and returns LHS
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
        // True | RHS -> short-circuits and returns LHS
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
    fn precedence() {
        assert_eq!(evaluate("ns = {f: x -> 2*x}, ns.f 3"), Value::Numb(6.0));
        assert_eq!(evaluate("ns = {a=10}, f = x -> 2*x, f ns.a"), Value::Numb(20.0));
        assert_eq!(evaluate("f = x -> {y:x}, (f 3).y"), Value::Numb(3.0));
    }
}
