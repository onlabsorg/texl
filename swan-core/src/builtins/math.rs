
use crate::value::{BuiltinFn, Scope, Value};

use super::helpers::*;



// ----------------------------------------------------------------------------
//  `math` built-in namespace
// ----------------------------------------------------------------------------

/// Constructs and populates the `math` built-in namespace.
///
/// Returns a [`Value::Namespace`] scope containing trigonometric, logarithmic,
/// exponential, rounding, and utility functions, alongside fundamental constants.
///
/// # Curried Two-Argument Functions
/// Functions expecting two arguments (such as `pow`, `log`, `atan2`, `hypot`) are
/// implemented as curried higher-order functions $f(x) \to g(y)$. Calling $f(x)$
/// returns a callable closure expecting $y$.
///
/// # Example
/// ```text
/// pow10 = math.pow 10;
/// pow10 2  # returns 1024.0 (2 ^ 10)
/// ```
pub fn create_namespace(parent: &Scope) -> Value {
    let math_scope = parent.new_child();

    // --- Single-argument math functions (f64 -> f64) ---
    math_scope.set("sqrt",  builtin_f64_1("math.sqrt",  f64::sqrt));
    math_scope.set("abs",   builtin_f64_1("math.abs",   f64::abs));
    math_scope.set("floor", builtin_f64_1("math.floor", f64::floor));
    math_scope.set("ceil",  builtin_f64_1("math.ceil",  f64::ceil));
    math_scope.set("round", builtin_f64_1("math.round", f64::round));
    math_scope.set("trunc", builtin_f64_1("math.trunc", f64::trunc));

    // Trigonometric functions
    math_scope.set("sin",   builtin_f64_1("math.sin",   f64::sin));
    math_scope.set("cos",   builtin_f64_1("math.cos",   f64::cos));
    math_scope.set("tan",   builtin_f64_1("math.tan",   f64::tan));
    math_scope.set("asin",  builtin_f64_1("math.asin",  f64::asin));
    math_scope.set("acos",  builtin_f64_1("math.acos",  f64::acos));
    math_scope.set("atan",  builtin_f64_1("math.atan",  f64::atan));

    // Hyperbolic functions
    math_scope.set("sinh",  builtin_f64_1("math.sinh",  f64::sinh));
    math_scope.set("cosh",  builtin_f64_1("math.cosh",  f64::cosh));
    math_scope.set("tanh",  builtin_f64_1("math.tanh",  f64::tanh));
    math_scope.set("asinh", builtin_f64_1("math.asinh", f64::asinh));
    math_scope.set("acosh", builtin_f64_1("math.acosh", f64::acosh));
    math_scope.set("atanh", builtin_f64_1("math.atanh", f64::atanh));

    // Exponentials and Logarithms
    math_scope.set("exp",   builtin_f64_1("math.exp",   f64::exp));
    math_scope.set("ln",    builtin_f64_1("math.ln",    f64::ln));
    math_scope.set("log2",  builtin_f64_1("math.log2",  f64::log2));
    math_scope.set("log10", builtin_f64_1("math.log10", f64::log10));

    // --- Curried two-argument math functions (f64 -> f64 -> f64) ---
    // `pow`:  pow(exp) -> g(base) => base.powf(exp)
    math_scope.set("pow",   builtin_f64_curried("math.pow",   |exp, base| base.powf(exp)));
    // `log`:  log(base) -> g(x) => x.log(base)
    math_scope.set("log",   builtin_f64_curried("math.log",   |base, x| x.log(base)));
    // `atan2`: atan2(x) -> g(y) => y.atan2(x)
    math_scope.set("atan2", builtin_f64_curried("math.atan2", |x, y| y.atan2(x)));
    // `hypot`: hypot(x) -> g(y) => x.hypot(y)
    math_scope.set("hypot", builtin_f64_curried("math.hypot", |x, y| x.hypot(y)));

    // --- Variadic / Multi-argument math functions ---
    math_scope.set("max", Value::Builtin(BuiltinFn::new("math.max", math_max)));
    math_scope.set("min", Value::Builtin(BuiltinFn::new("math.min", math_min)));

    // --- Mathematical constants ---
    math_scope.set("PI",       Value::Numb(std::f64::consts::PI));
    math_scope.set("E",        Value::Numb(std::f64::consts::E));
    math_scope.set("INF",      Value::Numb(f64::INFINITY));
    math_scope.set("INFINITY", Value::Numb(f64::INFINITY));

    Value::Namespace(math_scope)
}

/// Evaluates `math.max` over a single scalar or a tuple of numeric values.
///
/// Iterates over all values yielded by `arg.items()`. Returns the maximum number.
///
/// # Errors
/// Returns `Value::Undefined` if:
/// - Any element in `arg` is not a `Value::Numb`.
/// - The input tuple/argument yields zero elements.
fn math_max(arg: Value) -> Value {
    let mut count = 0;
    let mut max_val = f64::NEG_INFINITY;

    for item in arg.items() {
        match item {
            Value::Numb(n) => {
                if n > max_val {
                    max_val = n;
                }
                count += 1;
            }
            _ => return Value::undef_unary("math.max", arg, 0),
        }
    }

    if count == 0 {
        Value::undef_unary("math.max", arg, 0)
    } else {
        Value::Numb(max_val)
    }
}

/// Evaluates `math.min` over a single scalar or a tuple of numeric values.
///
/// Iterates over all values yielded by `arg.items()`. Returns the minimum number.
///
/// # Errors
/// Returns `Value::Undefined` if:
/// - Any element in `arg` is not a `Value::Numb`.
/// - The input tuple/argument yields zero elements.
fn math_min(arg: Value) -> Value {
    let mut count = 0;
    let mut min_val = f64::INFINITY;

    for item in arg.items() {
        match item {
            Value::Numb(n) => {
                if n < min_val {
                    min_val = n;
                }
                count += 1;
            }
            _ => return Value::undef_unary("math.min", arg, 0),
        }
    }

    if count == 0 {
        Value::undef_unary("math.min", arg, 0)
    } else {
        Value::Numb(min_val)
    }
}



// ----------------------------------------------------------------------------
//  PRIVATE HELPERS FOR MATH BUILT-INS
// ----------------------------------------------------------------------------

/// Wraps a standard single-argument float function (`f64 -> f64`) into a [`Value::Builtin`].
///
/// - Accepts a single [`Value::Numb`] operand.
/// - Propagates `NaN` results (e.g., `math.sqrt(-1.0)`) or invalid argument types as `Value::Undefined`.
fn builtin_f64_1(name: &'static str, f: fn(f64) -> f64) -> Value {
    map_1(name, move |arg| {
        match arg {
            Value::Numb(x) => {
                let result = f(x);
                // Propagate NaN (e.g. sqrt(-1)) as Undefined.
                if result.is_nan() {
                    Value::undef_unary(name, Value::Numb(x), 0)
                } else {
                    Value::Numb(result)
                }
            }
            other => Value::undef_unary(name, other, 0),
        }
    })
}

/// Wraps a curried float function (`f64 -> f64 -> f64`) into a nested [`Value::Builtin`].
///
/// Calling `f(x)` produces a second function `g(y)` that computes `result(x, y)`.
/// Returns `Value::Undefined` if either argument is non-numeric or if the result evaluates to `NaN`.
fn builtin_f64_curried(name: &'static str, f: fn(f64, f64) -> f64) -> Value {
    map_curried(name, move |arg1, arg2| {

        match (&arg1, arg2) {
            (Value::Numb(a), Value::Numb(b)) => {
                let result = f(*a, b);
                if result.is_nan() {
                    Value::undef_unary(name, Value::Numb(b), 0)
                } else {
                    Value::Numb(result)
                }
            }
            (_, other) => Value::undef_unary(name, other, 0),
        }
    })
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
        scope.set("math" , create_namespace(&scope));
        scope.set("TRUE" , Value::Bool(true));
        scope.set("FALSE", Value::Bool(false));
        scope.new_child()
    }

    fn run(src: &str) -> Value {
        let scope = root_scope();
        let mut parser = Parser::new(Lexer::new(src));
        let ast = parser.parse();
        eval(&ast, &scope)
    }


    // ---------------------------------------------------------------------
    //  Single-argument functions (builtin_f64_1)
    // ---------------------------------------------------------------------

    #[test]
    fn test_sqrt() {

        // Numbers
        assert_eq!(run("math.sqrt 9"), Value::Numb(3.0));
        assert_eq!(run("math.sqrt 4"), Value::Numb(2.0));

        // Undefined
        assert_eq!(run("math.sqrt(-1)")    , Value::undef_unary("math.sqrt", Value::Numb(-1.0), 0));
        assert_eq!(run("math.sqrt TRUE")   , Value::undef_unary("math.sqrt", Value::Bool(true), 0));
        assert_eq!(run("math.sqrt \"abc\""), Value::undef_unary("math.sqrt", Value::Text("abc".into()), 0));

        // Nothing
        assert_eq!(run("math.sqrt()"), Value::Nothing);

        // Tuples
        assert_eq!(run("math.sqrt(9,4)"), Value::Tuple(Rc::new(vec![
            Value::Numb(3.0),
            Value::Numb(2.0),
        ])));
    }

    #[test]
    fn test_abs() {

        // Numbers
        assert_eq!(run("math.abs(-5)"), Value::Numb(5.0));
        assert_eq!(run("math.abs 3"),   Value::Numb(3.0));

        // Undefined
        assert_eq!(run("math.abs TRUE")   , Value::undef_unary("math.abs", Value::Bool(true), 0));
        assert_eq!(run("math.abs \"abc\""), Value::undef_unary("math.abs", Value::Text("abc".into()), 0));

        // Nothing
        assert_eq!(run("math.abs()"), Value::Nothing);

        // Tuple
        assert_eq!(run("math.abs(-5,3)"), Value::Tuple(Rc::new(vec![
            Value::Numb(5.0),
            Value::Numb(3.0),
        ])));
    }

    #[test]
    fn test_floor() {
        // Numb
        assert_eq!(run("math.floor 3.7"), Value::Numb(3.0));
        assert_eq!(run("math.floor(-3.2)"), Value::Numb(-4.0));

        // Undefined (type error)
        assert!(matches!(run(r#"math.floor "3.7""#), Value::Undefined(_)));

        // Nothing
        assert!(matches!(run("math.floor()"), Value::Nothing));

        // Tuple
        assert_eq!(
            run("math.floor(1.2, 3.8)"),
            Value::Tuple(Rc::new(vec![Value::Numb(1.0), Value::Numb(3.0)]))
        );
    }

    #[test]
    fn test_ceil() {
        // Numb
        assert_eq!(run("math.ceil 3.2"), Value::Numb(4.0));
        assert_eq!(run("math.ceil(-3.7)"), Value::Numb(-3.0));

        // Undefined (type error)
        assert!(matches!(run("math.ceil [1, 2]"), Value::Undefined(_)));

        // Nothing
        assert!(matches!(run("math.ceil()"), Value::Nothing));

        // Tuple
        assert_eq!(
            run("math.ceil(1.2, 3.8)"),
            Value::Tuple(Rc::new(vec![Value::Numb(2.0), Value::Numb(4.0)]))
        );
    }

    #[test]
    fn test_round() {
        // Numb
        assert_eq!(run("math.round 3.5"), Value::Numb(4.0));
        assert_eq!(run("math.round 3.2"), Value::Numb(3.0));

        // Undefined (type error)
        assert!(matches!(run("math.round {x: 1}"), Value::Undefined(_)));

        // Nothing
        assert!(matches!(run("math.round()"), Value::Nothing));

        // Tuple
        assert_eq!(
            run("math.round(1.2, 3.8)"),
            Value::Tuple(Rc::new(vec![Value::Numb(1.0), Value::Numb(4.0)]))
        );
    }

    #[test]
    fn test_trunc() {
        // Numb
        assert_eq!(run("math.trunc 3.7"), Value::Numb(3.0));
        assert_eq!(run("math.trunc(-3.7)"), Value::Numb(-3.0));

        // Undefined (type error)
        assert!(matches!(run("math.trunc(TRUE)"), Value::Undefined(_)));

        // Nothing
        assert!(matches!(run("math.trunc()"), Value::Nothing));

        // Tuple
        assert_eq!(
            run("math.trunc(1.9, -2.9)"),
            Value::Tuple(Rc::new(vec![Value::Numb(1.0), Value::Numb(-2.0)]))
        );
    }

    #[test]
    fn test_sin() {
        // Numb
        assert_eq!(run("math.sin 0"), Value::Numb(0.0));

        // Undefined (type error)
        assert!(matches!(run(r#"math.sin "zero""#), Value::Undefined(_)));

        // Nothing
        assert!(matches!(run("math.sin()"), Value::Nothing));

        // Tuple
        assert_eq!(
            run("math.sin(0, 0)"),
            Value::Tuple(Rc::new(vec![Value::Numb(0.0), Value::Numb(0.0)]))
        );
    }

    #[test]
    fn test_cos() {
        // Numb
        assert_eq!(run("math.cos 0"), Value::Numb(1.0));

        // Undefined (type error)
        assert!(matches!(run(r#"math.cos "zero""#), Value::Undefined(_)));

        // Nothing
        assert!(matches!(run("math.cos()"), Value::Nothing));

        // Tuple
        assert_eq!(
            run("math.cos(0, 0)"),
            Value::Tuple(Rc::new(vec![Value::Numb(1.0), Value::Numb(1.0)]))
        );
    }

    #[test]
    fn test_tan() {
        // Numb
        assert_eq!(run("math.tan 0"), Value::Numb(0.0));

        // Undefined (type error)
        assert!(matches!(run(r#"math.tan "zero""#), Value::Undefined(_)));

        // Nothing
        assert!(matches!(run("math.tan()"), Value::Nothing));

        // Tuple
        assert_eq!(
            run("math.tan(0, 0)"),
            Value::Tuple(Rc::new(vec![Value::Numb(0.0), Value::Numb(0.0)]))
        );
    }

    #[test]
    fn test_asin() {
        // Numb
        assert_eq!(run("math.asin 0"), Value::Numb(0.0));

        // Undefined (out of domain / type error)
        assert!(matches!(run("math.asin 2"), Value::Undefined(_)));
        assert!(matches!(run(r#"math.asin "0""#), Value::Undefined(_)));

        // Nothing
        assert!(matches!(run("math.asin()"), Value::Nothing));

        // Tuple
        assert_eq!(
            run("math.asin(0, 0)"),
            Value::Tuple(Rc::new(vec![Value::Numb(0.0), Value::Numb(0.0)]))
        );
    }

    #[test]
    fn test_acos() {
        // Numb
        assert_eq!(run("math.acos 1"), Value::Numb(0.0));

        // Undefined (out of domain / type error)
        assert!(matches!(run("math.acos 2"), Value::Undefined(_)));
        assert!(matches!(run(r#"math.acos "1""#), Value::Undefined(_)));

        // Nothing
        assert!(matches!(run("math.acos()"), Value::Nothing));

        // Tuple
        assert_eq!(
            run("math.acos(1, 1)"),
            Value::Tuple(Rc::new(vec![Value::Numb(0.0), Value::Numb(0.0)]))
        );
    }

    #[test]
    fn test_atan() {
        // Numb
        assert_eq!(run("math.atan 0"), Value::Numb(0.0));

        // Undefined (type error)
        assert!(matches!(run(r#"math.atan "0""#), Value::Undefined(_)));

        // Nothing
        assert!(matches!(run("math.atan()"), Value::Nothing));

        // Tuple
        assert_eq!(
            run("math.atan(0, 0)"),
            Value::Tuple(Rc::new(vec![Value::Numb(0.0), Value::Numb(0.0)]))
        );
    }

    #[test]
    fn test_sinh() {
        // Numb
        assert_eq!(run("math.sinh 0"), Value::Numb(0.0));

        // Undefined (type error)
        assert!(matches!(run(r#"math.sinh "0""#), Value::Undefined(_)));

        // Nothing
        assert!(matches!(run("math.sinh()"), Value::Nothing));

        // Tuple
        assert_eq!(
            run("math.sinh(0, 0)"),
            Value::Tuple(Rc::new(vec![Value::Numb(0.0), Value::Numb(0.0)]))
        );
    }

    #[test]
    fn test_cosh() {
        // Numb
        assert_eq!(run("math.cosh 0"), Value::Numb(1.0));

        // Undefined (type error)
        assert!(matches!(run(r#"math.cosh "0""#), Value::Undefined(_)));

        // Nothing
        assert!(matches!(run("math.cosh()"), Value::Nothing));

        // Tuple
        assert_eq!(
            run("math.cosh(0, 0)"),
            Value::Tuple(Rc::new(vec![Value::Numb(1.0), Value::Numb(1.0)]))
        );
    }

    #[test]
    fn test_tanh() {
        // Numb
        assert_eq!(run("math.tanh 0"), Value::Numb(0.0));

        // Undefined (type error)
        assert!(matches!(run(r#"math.tanh "0""#), Value::Undefined(_)));

        // Nothing
        assert!(matches!(run("math.tanh()"), Value::Nothing));

        // Tuple
        assert_eq!(
            run("math.tanh(0, 0)"),
            Value::Tuple(Rc::new(vec![Value::Numb(0.0), Value::Numb(0.0)]))
        );
    }

    #[test]
    fn test_asinh() {
        // Numb
        assert_eq!(run("math.asinh 0"), Value::Numb(0.0));

        // Undefined (type error)
        assert!(matches!(run(r#"math.asinh "0""#), Value::Undefined(_)));

        // Nothing
        assert!(matches!(run("math.asinh()"), Value::Nothing));

        // Tuple
        assert_eq!(
            run("math.asinh(0, 0)"),
            Value::Tuple(Rc::new(vec![Value::Numb(0.0), Value::Numb(0.0)]))
        );
    }

    #[test]
    fn test_acosh() {
        // Numb
        assert_eq!(run("math.acosh 1"), Value::Numb(0.0));

        // Undefined (out of domain / type error)
        assert!(matches!(run("math.acosh 0.5"), Value::Undefined(_)));
        assert!(matches!(run(r#"math.acosh "1""#), Value::Undefined(_)));

        // Nothing
        assert!(matches!(run("math.acosh()"), Value::Nothing));

        // Tuple
        assert_eq!(
            run("math.acosh(1, 1)"),
            Value::Tuple(Rc::new(vec![Value::Numb(0.0), Value::Numb(0.0)]))
        );
    }

    #[test]
    fn test_atanh() {
        // Numb
        assert_eq!(run("math.atanh 0"), Value::Numb(0.0));

        // Undefined (out of domain / type error)
        assert!(matches!(run("math.atanh 2"), Value::Undefined(_)));
        assert!(matches!(run(r#"math.atanh "0""#), Value::Undefined(_)));

        // Nothing
        assert!(matches!(run("math.atanh()"), Value::Nothing));

        // Tuple
        assert_eq!(
            run("math.atanh(0, 0)"),
            Value::Tuple(Rc::new(vec![Value::Numb(0.0), Value::Numb(0.0)]))
        );
    }

    #[test]
    fn test_exp() {
        // Numb
        assert_eq!(run("math.exp 0"), Value::Numb(1.0));

        // Undefined (type error)
        assert!(matches!(run(r#"math.exp "0""#), Value::Undefined(_)));

        // Nothing
        assert!(matches!(run("math.exp()"), Value::Nothing));

        // Tuple
        assert_eq!(
            run("math.exp(0, 0)"),
            Value::Tuple(Rc::new(vec![Value::Numb(1.0), Value::Numb(1.0)]))
        );
    }

    #[test]
    fn test_ln() {
        // Numb
        assert_eq!(run("math.ln(math.E)"), Value::Numb(1.0));

        // Undefined (out of domain / type error)
        assert!(matches!(run("math.ln(-1)"), Value::Undefined(_)));
        assert!(matches!(run(r#"math.ln "e""#), Value::Undefined(_)));

        // Nothing
        assert!(matches!(run("math.ln()"), Value::Nothing));

        // Tuple
        assert_eq!(
            run("math.ln(math.E, math.E)"),
            Value::Tuple(Rc::new(vec![Value::Numb(1.0), Value::Numb(1.0)]))
        );
    }

    #[test]
    fn test_log2() {
        // Numb
        assert_eq!(run("math.log2 8"), Value::Numb(3.0));

        // Undefined (out of domain / type error)
        assert!(matches!(run("math.log2(-1)"), Value::Undefined(_)));
        assert!(matches!(run(r#"math.log2 "8""#), Value::Undefined(_)));

        // Nothing
        assert!(matches!(run("math.log2()"), Value::Nothing));

        // Tuple
        assert_eq!(
            run("math.log2(8, 16)"),
            Value::Tuple(Rc::new(vec![Value::Numb(3.0), Value::Numb(4.0)]))
        );
    }

    #[test]
    fn test_log10() {
        // Numb
        assert_eq!(run("math.log10 100"), Value::Numb(2.0));

        // Undefined (out of domain / type error)
        assert!(matches!(run("math.log10(-1)"), Value::Undefined(_)));
        assert!(matches!(run(r#"math.log10 "100""#), Value::Undefined(_)));

        // Nothing
        assert!(matches!(run("math.log10()"), Value::Nothing));

        // Tuple
        assert_eq!(
            run("math.log10(100, 1000)"),
            Value::Tuple(Rc::new(vec![Value::Numb(2.0), Value::Numb(3.0)]))
        );
    }


    // ---------------------------------------------------------------------
    //  Curried higher-order functions (builtin_f64_curried)
    // ---------------------------------------------------------------------

    #[test]
    fn test_pow() {

        // Numbers
        assert_eq!(run("pow10 = math.pow 10, pow10 2"), Value::Numb(1024.0));
        assert_eq!(run("sq = math.pow 2, sq 5"),        Value::Numb(25.0));
        assert_eq!(run("math.pow 3 2"),                 Value::Numb(8.0));

        // Undefined
        assert_eq!(run("math.pow TRUE 3")   , Value::undef_unary("math.pow", Value::Numb(3.0), 0));
        assert_eq!(run("math.pow \"abc\" 3"), Value::undef_unary("math.pow", Value::Numb(3.0), 0));

        // Nothing
        assert_eq!(run("math.pow()"), Value::Nothing);

        // Tuples
        assert_eq!(run("math.pow(3, 4) 2"), Value::Tuple(Rc::new(vec![
            Value::Numb(8.0),
            Value::Numb(16.0),
        ])));
        assert_eq!(run("math.pow(3, 4) (2, 5)"), Value::Tuple(Rc::new(vec![
            Value::Numb(  8.0), 
            Value::Numb(125.0),
            Value::Numb( 16.0), 
            Value::Numb(625.0),
        ])));
    }

    #[test]
    fn test_log() {

        // Numbers
        assert_eq!(run("log10 = math.log 10, log10 100"), Value::Numb(2.0));
        assert_eq!(run("math.log 2 8"),                   Value::Numb(3.0));

        // Undefined
        assert_eq!(run("math.log TRUE 3")   , Value::undef_unary("math.log", Value::Numb(3.0), 0));
        assert_eq!(run("math.log \"abc\" 3"), Value::undef_unary("math.log", Value::Numb(3.0), 0));

        // Nothing
        assert_eq!(run("math.log()"), Value::Nothing);

        // Tuples
        assert_eq!(run("math.log(10, 100) 10000"), Value::Tuple(Rc::new(vec![
            Value::Numb(4.0),
            Value::Numb(2.0),
        ])));
        assert_eq!(run("math.log(10, 100) (100, 10000)"), Value::Tuple(Rc::new(vec![
            Value::Numb(2.0), 
            Value::Numb(4.0),
            Value::Numb(1.0), 
            Value::Numb(2.0),
        ])));
    }

    #[test]
    fn test_atan2() {
        // Numb
        assert_eq!(
            run("atan_x = math.atan2 0, atan_x 1"),
            Value::Numb(std::f64::consts::FRAC_PI_2)
        );
        assert_eq!(run("math.atan2 1 0"), Value::Numb(0.0));

        // Undefined (type errors)
        assert!(matches!(run(r#"math.atan2 "0" 1"#), Value::Undefined(_)));
        assert!(matches!(run(r#"math.atan2 0 "1""#), Value::Undefined(_)));

        // Nothing
        assert!(matches!(run("math.atan2 0 ()"), Value::Nothing));
        assert!(matches!(run("math.atan2 () 1"), Value::Nothing));

        // Tuple
        assert_eq!(
            run("math.atan2 1 (0, 0)"),
            Value::Tuple(Rc::new(vec![Value::Numb(0.0), Value::Numb(0.0)]))
        );
    }

    #[test]
    fn test_hypot() {
        // Numb
        assert_eq!(run("h3 = math.hypot 3, h3 4"), Value::Numb(5.0));
        assert_eq!(run("math.hypot 3 4"), Value::Numb(5.0));

        // Undefined (type errors)
        assert!(matches!(run(r#"math.hypot "3" 4"#), Value::Undefined(_)));
        assert!(matches!(run(r#"math.hypot 3 "4""#), Value::Undefined(_)));

        // Nothing
        assert!(matches!(run("math.hypot 3 ()"), Value::Nothing));
        assert!(matches!(run("math.hypot () 4"), Value::Nothing));

        // Tuple
        assert_eq!(
            run("math.hypot 3 (4, 4)"),
            Value::Tuple(Rc::new(vec![Value::Numb(5.0), Value::Numb(5.0)]))
        );
    }


    // ---------------------------------------------------------------------
    //  Multi-argument functions
    // ---------------------------------------------------------------------

    #[test]
    fn test_max() {
        // Numb / Tuple
        assert_eq!(run("math.max(1, 22, 3, 4)"), Value::Numb(22.0));
        assert_eq!(run("math.max 42"), Value::Numb(42.0));
        assert_eq!(run("math.max(-10, -5, -20)"), Value::Numb(-5.0));

        // Undefined (type error / empty)
        assert!(matches!(run(r#"math.max(1, "two", 3)"#), Value::Undefined(_)));

        // Nothing (0 items -> Undefined)
        assert!(matches!(run("math.max()"), Value::Undefined(_)));
    }

    #[test]
    fn test_min() {
        // Numb / Tuple
        assert_eq!(run("math.min(1, 22, 3, 4)"), Value::Numb(1.0));
        assert_eq!(run("math.min 42"), Value::Numb(42.0));
        assert_eq!(run("math.min(-10, -5, -20)"), Value::Numb(-20.0));

        // Undefined (type error / empty)
        assert!(matches!(run(r#"math.min(1, [2], 3)"#), Value::Undefined(_)));

        // Nothing (0 items -> Undefined)
        assert!(matches!(run("math.min()"), Value::Undefined(_)));
    }


    // ---------------------------------------------------------------------
    //  Constants
    // ---------------------------------------------------------------------

    #[test]
    fn test_constants() {
        assert_eq!(run("math.PI"), Value::Numb(std::f64::consts::PI));
        assert_eq!(run("math.E"), Value::Numb(std::f64::consts::E));
        assert_eq!(run("math.INF"), Value::Numb(f64::INFINITY));
        assert_eq!(run("math.INFINITY"), Value::Numb(f64::INFINITY));
    }
}
