// src/builtins.rs
//
// Built-in functions and namespaces that are pre-loaded into the root scope.
//
// # How to add a new built-in function
//
//   1. Write a plain Rust function `fn my_fn(arg: Value) -> Value { ... }`.
//   2. Wrap it: `Value::Builtin(BuiltinFn::new("my_fn", my_fn))`.
//   3. `scope.set("my_fn", ...)` in `root_scope()`, or insert it into one of
//      the namespace builders (`math_namespace()`, etc.).
//
// # Multi-argument functions / Curried Higher-Order Functions
//
//   Functions of type f(x, y) that require two parameters are implemented as
//   higher-order functions f(x) that return a g(y) function.
//   For example: `pow10 = math.pow 10`, `pow10 2` returns 1024.
//
// # Error convention
//
//   Never panic. Return `Value::Undefined(Undef::UnaryOperation { ... })` for
//   type errors. Use the function's own `&'static str` name as the `op` field
//   so the caller can identify the source of the error.

use std::rc::Rc;
use crate::value::{BuiltinFn, Scope, Undef, Value};
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::eval::eval;

mod helpers;
use helpers::*;

mod text;
mod nspace;
mod undef;
mod math;







// ----------------------------------------------------------------------------
//  Root scope factory
// ----------------------------------------------------------------------------

/// Build a fresh root `Scope` pre-populated with all built-in bindings.
///
/// Every evaluation session should start from a scope created here (or from a
/// child of one) so that built-in names are always available.
///
/// ```
/// use swan_core::builtins::root_scope;
/// use swan_core::eval::eval;
/// use swan_core::lexer::Lexer;
/// use swan_core::parser::Parser;
///
/// let scope = root_scope();
/// let mut parser = Parser::new(Lexer::new("1+2"));
/// let ast = parser.parse();
/// assert_eq!(eval(&ast, &scope), swan_core::value::Value::Numb(3.0));
/// ```
pub fn root_scope() -> Scope {
    let scope = Scope::new();

    // --- Builtin Namespaces ---
    scope.set("bool"  , create_bool_namespace(&scope));
    scope.set("numb"  , create_numb_namespace(&scope));
    scope.set("text"  , text::create_namespace(&scope));
    scope.set("list"  , create_list_namespace(&scope));
    scope.set("nspace", nspace::create_namespace(&scope));
    scope.set("undef" , undef::create_namespace(&scope));
    scope.set("math"  , math::create_namespace(&scope));
    scope.set("time"  , create_time_namespace(&scope));

    // --- Builtin Constants ---
    scope.set("TRUE" , Value::Bool(true));
    scope.set("FALSE", Value::Bool(false));

    // --- Builtin Functions ---
    scope.set("type" , Value::Builtin(BuiltinFn::new("type" , root_type)));
    scope.set("parse", Value::Builtin(BuiltinFn::new("parse", root_parse)));

    // --- Return empty child ---
    scope.new_child()
}





// ----------------------------------------------------------------------------
//  built-in functions
// ----------------------------------------------------------------------------

fn root_type(val: Value) -> Value {
    match val {
        Value::Nothing => Value::Nothing,
        Value::Tuple(items) => {
            let types: Vec<Value> = items
                .iter()
                .map(|it| Value::Text(type_name(it).into()))
                .collect();
            normalize_tuple(types)
        }
        other => Value::Text(type_name(&other).into()),
    }
}

fn type_name(val: &Value) -> &'static str {
    match val {
        Value::Nothing => "Nothing",
        Value::Bool(_) => "Bool",
        Value::Numb(_) => "Numb",
        Value::Text(_) => "Text",
        Value::List(_) => "List",
        Value::Namespace(_) => "Namespace",
        Value::Func { .. } | Value::Builtin(_) => "Func",
        Value::Undefined(_) => "Undefined",
        Value::Tuple(_) => "Tuple",
        val => panic!("Unknown value type: {:?}", val),
    }
}

/// Builtin `parse` function: takes a string expression `S` and returns an `eval` function.
fn root_parse(arg: Value) -> Value {
    match arg {
        Value::Text(source) => {
            let mut parser = Parser::new(Lexer::new(&source));
            let ast = parser.parse();

            // Return an `eval` function capturing the parsed AST node
            Value::Builtin(BuiltinFn::new("eval", move |ns_arg| {
                match ns_arg {
                    Value::Namespace(scope) => eval(&ast, &scope),
                    other => undef_unary!("eval", other),
                }
            }))
        }
        other => undef_unary!("parse", other),
    }
}






// ----------------------------------------------------------------------------
//  `bool` built-in namespace
// ----------------------------------------------------------------------------

pub fn create_bool_namespace(parent: &Scope) -> Value {
    let bool_scope = parent.new_child();
    bool_scope.set("TRUE" , Value::Bool(true));
    bool_scope.set("FALSE", Value::Bool(false));
    bool_scope.set("from" , Value::Builtin(BuiltinFn::new("bool.from", bool_from)));
    Value::Namespace(bool_scope)
}

fn bool_from(val: Value) -> Value {
    Value::Bool(bool::from(val))
}





// ----------------------------------------------------------------------------
//  `numb` built-in namespace 
// ----------------------------------------------------------------------------

pub fn create_numb_namespace(parent: &Scope) -> Value {
    let numb_scope = parent.new_child();
    numb_scope.set("from" , Value::Builtin(BuiltinFn::new("numb.from", numb_from)));
    Value::Namespace(numb_scope)
}

fn numb_from(val: Value) -> Value {
    match &val {
        Value::Nothing  => Value::Numb(0.0),
        Value::Numb(n)  => Value::Numb(*n),
        Value::Bool(b)  => Value::Numb(if *b { 1.0 } else { 0.0 }),
        Value::Text(s)  => match s.parse::<f64>() {
            Ok(n)  => Value::Numb(n),
            Err(_) => undef_unary!("numb.from", val),
        },
        _ => undef_unary!("numb.from", val),
    }
}





// ----------------------------------------------------------------------------
//  `list` built-in namespace
// ----------------------------------------------------------------------------

pub fn create_list_namespace(parent: &Scope) -> Value {
    let list_scope = parent.new_child();
    list_scope.set("enum"   , Value::Builtin(BuiltinFn::new("list.enum"   , list_enum)));
    list_scope.set("size"   , Value::Builtin(BuiltinFn::new("list.size"   , list_size)));
    list_scope.set("reverse", Value::Builtin(BuiltinFn::new("list.reverse", list_reverse)));
    list_scope.set("find"   , Value::Builtin(BuiltinFn::new("list.find"   , list_find)));
    list_scope.set("rfind"  , Value::Builtin(BuiltinFn::new("list.rfind"  , list_rfind)));
    list_scope.set("head"   , Value::Builtin(BuiltinFn::new("list.head"   , list_head)));
    list_scope.set("tail"   , Value::Builtin(BuiltinFn::new("list.tail"   , list_tail)));
    Value::Namespace(list_scope)
}

fn list_enum(val: Value) -> Value {
    match val {
        Value::List(l) => normalize_tuple(l.as_ref().clone()),
        other => undef_unary!("list.enum", other),
    }
}

fn list_size(val: Value) -> Value {
    match val {
        Value::List(l) => Value::Numb(l.len() as f64),
        other => undef_unary!("list.size", other),
    }
}

fn list_reverse(val: Value) -> Value {
    match val {
        Value::List(l) => {
            let mut rev = l.as_ref().clone();
            rev.reverse();
            Value::List(Rc::new(rev))
        }
        other => undef_unary!("list.reverse", other),
    }
}

fn list_find(item: Value) -> Value {
    Value::Builtin(BuiltinFn::new("list.find", move |list| {
        match list {
            Value::List(l) => {
                let idx = l.iter().position(|x| x == &item);
                match idx {
                    Some(i) => Value::Numb(i as f64),
                    None => Value::Numb(-1.0),
                }
            }
            other => undef_unary!("list.find", other),
        }
    }))
}

fn list_rfind(item: Value) -> Value {
    Value::Builtin(BuiltinFn::new("list.rfind", move |list| {
        match list {
            Value::List(l) => {
                let idx = l.iter().rposition(|x| x == &item);
                match idx {
                    Some(i) => Value::Numb(i as f64),
                    None => Value::Numb(-1.0),
                }
            }
            other => undef_unary!("list.rfind", other),
        }
    }))
}

fn list_head(n: Value) -> Value {
    Value::Builtin(BuiltinFn::new("list.head", move |list| {
        match (&n, list) {
            (Value::Numb(n_val), Value::List(l)) => {
                let len = l.len() as i64;
                let n_idx = *n_val as i64;
                let end = if n_idx >= 0 {
                    n_idx.min(len) as usize
                } else {
                    (len + n_idx).max(0) as usize
                };
                Value::List(Rc::new(l[..end].to_vec()))
            }
            (_, other) => undef_unary!("list.head", other),
        }
    }))
}

fn list_tail(n: Value) -> Value {
    Value::Builtin(BuiltinFn::new("list.tail", move |list| {
        match (&n, list) {
            (Value::Numb(n_val), Value::List(l)) => {
                let len = l.len() as i64;
                let n_idx = *n_val as i64;
                let start = if n_idx >= 0 {
                    n_idx.min(len) as usize
                } else {
                    (len + n_idx).max(0) as usize
                };
                Value::List(Rc::new(l[start..].to_vec()))
            }
            (_, other) => undef_unary!("list.tail", other),
        }
    }))
}

















// ----------------------------------------------------------------------------
//  `time` built-in namespace
// ----------------------------------------------------------------------------

pub fn create_time_namespace(parent: &Scope) -> Value {
    let time_scope = parent.new_child();
    time_scope.set("now",           Value::Builtin(BuiltinFn::new("time.now",           time_now)));
    time_scope.set("timezone",      Value::Builtin(BuiltinFn::new("time.timezone",      time_timezone)));
    time_scope.set("to_date",       Value::Builtin(BuiltinFn::new("time.to_date",       time_to_date)));
    time_scope.set("to_UTC_date",   Value::Builtin(BuiltinFn::new("time.to_UTC_date",   time_to_utc_date)));
    time_scope.set("from_date",     Value::Builtin(BuiltinFn::new("time.from_date",     time_from_date)));
    time_scope.set("from_UTC_date", Value::Builtin(BuiltinFn::new("time.from_UTC_date", time_from_utc_date)));
    time_scope.set("to_ISO_string", Value::Builtin(BuiltinFn::new("time.to_ISO_string", time_to_iso_string)));
    time_scope.set("from_string",   Value::Builtin(BuiltinFn::new("time.from_string",   time_from_string)));
    time_scope.set("week_day",      Value::Builtin(BuiltinFn::new("time.week_day",      time_week_day)));
    time_scope.set("week_number",   Value::Builtin(BuiltinFn::new("time.week_number",   time_week_number)));
    Value::Namespace(time_scope)
}

fn time_now(_arg: Value) -> Value {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => Value::Numb(d.as_secs_f64()),
        Err(_) => Value::Numb(0.0),
    }
}

fn time_timezone(_arg: Value) -> Value {
    Value::Numb(0.0)
}

fn time_to_utc_date(val: Value) -> Value {
    match val {
        Value::Numb(t) => {
            let t_floor = t.floor() as i64;
            let sub_sec = t - (t_floor as f64);
            let days = t_floor.div_euclid(86400);
            let rem_secs = t_floor.rem_euclid(86400);
            let hours = rem_secs / 3600;
            let minutes = (rem_secs % 3600) / 60;
            let seconds = (rem_secs % 60) as f64 + sub_sec;
            let (y, m, d) = civil_from_days(days);

            let scope = Scope::new();
            scope.set("year", Value::Numb(y as f64));
            scope.set("month", Value::Numb(m as f64));
            scope.set("day", Value::Numb(d as f64));
            scope.set("hours", Value::Numb(hours as f64));
            scope.set("minutes", Value::Numb(minutes as f64));
            scope.set("seconds", Value::Numb(seconds));
            Value::Namespace(scope)
        }
        other => undef_unary!("time.to_UTC_date", other),
    }
}

fn time_to_date(val: Value) -> Value {
    match val {
        Value::Numb(_) => time_to_utc_date(val),
        other => undef_unary!("time.to_date", other),
    }
}

fn time_from_utc_date(val: Value) -> Value {
    match val {
        Value::Namespace(scope) => {
            let y = match scope.get("year") {
                Value::Numb(n) => n as i64,
                _ => 0,
            };
            let m = match scope.get("month") {
                Value::Numb(n) => n as u32,
                _ => 1,
            };
            let d = match scope.get("day") {
                Value::Numb(n) => n as u32,
                _ => 1,
            };
            let h = match scope.get("hours") {
                Value::Numb(n) => n as i64,
                _ => 0,
            };
            let min = match scope.get("minutes") {
                Value::Numb(n) => n as i64,
                _ => 0,
            };
            let s = match scope.get("seconds") {
                Value::Numb(n) => n,
                _ => 0.0,
            };
            let days = days_from_civil(y, m, d);
            let total_secs = (days * 86400 + h * 3600 + min * 60) as f64 + s;
            Value::Numb(total_secs)
        }
        other => undef_unary!("time.from_UTC_date", other),
    }
}

fn time_from_date(val: Value) -> Value {
    match val {
        Value::Namespace(_) => time_from_utc_date(val),
        other => undef_unary!("time.from_date", other),
    }
}

fn time_to_iso_string(val: Value) -> Value {
    match val {
        Value::Numb(t) => {
            let t_floor = t.floor() as i64;
            let sub_sec = t - (t_floor as f64);
            let days = t_floor.div_euclid(86400);
            let rem_secs = t_floor.rem_euclid(86400);
            let hours = rem_secs / 3600;
            let minutes = (rem_secs % 3600) / 60;
            let seconds = (rem_secs % 60) as f64 + sub_sec;
            let (y, m, d) = civil_from_days(days);
            let s = format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:06.3}Z", y, m, d, hours, minutes, seconds);
            Value::Text(s.into())
        }
        other => undef_unary!("time.to_ISO_string", other),
    }
}

fn time_from_string(val: Value) -> Value {
    match val {
        Value::Text(s) => match parse_iso_string(&s) {
            Some(t) => Value::Numb(t),
            None => undef_unary!("time.from_string", Value::Text(s)), 
        },
        other => undef_unary!("time.from_string", other),
    }
}

fn time_week_day(val: Value) -> Value {
    match val {
        Value::Numb(t) => {
            let days = (t.floor() as i64).div_euclid(86400);
            let wd = (days + 4).rem_euclid(7);
            Value::Numb(wd as f64)
        }
        other => undef_unary!("time.week_day", other),
    }
}

fn time_week_number(val: Value) -> Value {
    match val {
        Value::Numb(t) => {
            let days = (t.floor() as i64).div_euclid(86400);
            let wday_mon0 = (days + 3).rem_euclid(7);
            let thu_days = days + 3 - wday_mon0;
            let (thu_y, _, _) = civil_from_days(thu_days);
            let jan4_days = days_from_civil(thu_y, 1, 4);
            let jan4_wday_mon0 = (jan4_days + 3).rem_euclid(7);
            let jan4_thu_days = jan4_days + 3 - jan4_wday_mon0;
            let wn = 1 + (thu_days - jan4_thu_days) / 7;
            Value::Numb(wn as f64)
        }
        other => undef_unary!("time.week_number", other),
    }
}





// ----------------------------------------------------------------------------
//  PRIVATE HELPERS
// ----------------------------------------------------------------------------

/// Convert days since 1970-01-01 to (year, month, day) in civil Gregorian calendar.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// Convert (year, month, day) to days since 1970-01-01 in civil Gregorian calendar.
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u32;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + (doe as i64) - 719468
}

/// Parse an ISO-8601 date/time string into epoch seconds.
fn parse_iso_string(s: &str) -> Option<f64> {
    let s = s.trim();
    let parts: Vec<&str> = s.split(|c| c == 'T' || c == 't' || c == ' ').collect();
    if parts.is_empty() {
        return None;
    }
    let date_parts: Vec<&str> = parts[0].split('-').collect();
    if date_parts.len() != 3 {
        return None;
    }
    let y: i64 = date_parts[0].parse().ok()?;
    let m: u32 = date_parts[1].parse().ok()?;
    let d: u32 = date_parts[2].parse().ok()?;
    let days = days_from_civil(y, m, d);

    let (h, min, sec) = if parts.len() > 1 {
        let time_str = parts[1].trim_end_matches(|c| c == 'Z' || c == 'z');
        let time_parts: Vec<&str> = time_str.split(':').collect();
        if time_parts.len() < 2 {
            return None;
        }
        let h: i64 = time_parts[0].parse().ok()?;
        let min: i64 = time_parts[1].parse().ok()?;
        let sec: f64 = if time_parts.len() >= 3 {
            time_parts[2].parse().ok()?
        } else {
            0.0
        };
        (h, min, sec)
    } else {
        (0, 0, 0.0)
    };

    let total_secs = (days * 86400 + h * 3600 + min * 60) as f64 + sec;
    Some(total_secs)
}





// ----------------------------------------------------------------------------
//  Tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::eval;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn run(src: &str) -> Value {
        let scope = root_scope();
        let mut parser = Parser::new(Lexer::new(src));
        let ast = parser.parse();
        eval(&ast, &scope)
    }

    mod root_builtins {
        use super::*;

        #[test]
        fn test_type() {
            assert_eq!(run("type(1 == 1)"), Value::Text("Bool".into()));
            assert_eq!(run("type 10"), Value::Text("Numb".into()));
            assert_eq!(run(r#"type "abc""#), Value::Text("Text".into()));
            assert_eq!(run("type [1, 2]"), Value::Text("List".into()));
            assert_eq!(run("type {a: 1}"), Value::Text("Namespace".into()));
            assert_eq!(run("type(x -> x)"), Value::Text("Func".into()));
            assert_eq!(run("type()"), Value::Nothing);
            assert_eq!(
                run(r#"type(10, "abc", [1])"#),
                Value::Tuple(Rc::new(vec![
                    Value::Text("Numb".into()),
                    Value::Text("Text".into()),
                    Value::Text("List".into()),
                ]))
            );
        }

        #[test]
        fn test_parse() {
            // Basic dynamic evaluation with scope
            assert_eq!(
                run(r#"eval_fn = parse "a + b", eval_fn { a = 10, b = 20 }"#),
                Value::Numb(30.0)
            );

            // Dynamic evaluation returning a function/closure
            assert_eq!(
                run(r#"double = (parse "x -> x * 2") {}, double 5"#),
                Value::Numb(10.0)
            );

            // Invalid argument type returns Undefined
            assert!(matches!(run("parse 123"), Value::Undefined(_)));
        }
    }

    mod bool_ns {
        use super::*;

        #[test]
        fn test_true_false_constants() {
            assert_eq!(run("bool.TRUE"),  Value::Bool(true));
            assert_eq!(run("bool.FALSE"), Value::Bool(false));
        }

        #[test]
        fn test_from_function() {
            assert_eq!(run("bool.from 1"),   Value::Bool(true));
            assert_eq!(run("bool.from 0"),   Value::Bool(false));
            assert_eq!(run("bool.from(bool.TRUE)"), Value::Bool(true));
            assert_eq!(run("bool.from(1,0,0,1)"), Value::Bool(true));
            assert_eq!(run("bool.from(0,0,0,0)"), Value::Bool(false));
        }
    }

    mod numb_ns {
        use super::*;

        #[test]
        fn test_from_func() {
            assert_eq!(run("numb.from()")          , Value::Numb(0.0));
            assert_eq!(run("numb.from(1.5)")       , Value::Numb(1.5));
            assert_eq!(run("numb.from(bool.TRUE)") , Value::Numb(1.0));
            assert_eq!(run("numb.from(bool.FALSE)"), Value::Numb(0.0));
            assert_eq!(run(r#"numb.from "42""#)    , Value::Numb(42.0));
        }
    }

    mod list_ns {
        use super::*;

        #[test]
        fn test_size_and_reverse() {
            assert_eq!(run("list.size [1, 2, 3]"), Value::Numb(3.0));
            assert_eq!(
                run("list.reverse [1, 2, 3]"),
                Value::List(Rc::new(vec![Value::Numb(3.0), Value::Numb(2.0), Value::Numb(1.0)]))
            );
        }

        #[test]
        fn test_find_and_rfind() {
            assert_eq!(run("list.find 20 [10, 20, 30, 20, 10]"), Value::Numb(1.0));
            assert_eq!(run("list.rfind 20 [10, 20, 30, 20, 10]"), Value::Numb(3.0));
            assert_eq!(run("list.find 99 [10, 20]"), Value::Numb(-1.0));
        }

        #[test]
        fn test_head_and_tail() {
            assert_eq!(
                run("list.head 2 [1, 2, 3, 4]"),
                Value::List(Rc::new(vec![Value::Numb(1.0), Value::Numb(2.0)]))
            );
            assert_eq!(
                run("list.tail 2 [1, 2, 3, 4]"),
                Value::List(Rc::new(vec![Value::Numb(3.0), Value::Numb(4.0)]))
            );
        }

        #[test]
        fn test_enum() {
            assert_eq!(
                run("list.enum [10, 20]"),
                Value::Tuple(Rc::new(vec![Value::Numb(10.0), Value::Numb(20.0)]))
            );
        }
    }



    mod time_ns {
        use super::*;

        #[test]
        fn test_now_and_timezone() {
            assert!(matches!(run("time.now()"), Value::Numb(_)));
            assert_eq!(run("time.timezone()"), Value::Numb(0.0));
        }

        #[test]
        fn test_to_iso_and_from_string() {
            assert_eq!(
                run("time.to_ISO_string 1639513675.900"),
                Value::Text("2021-12-14T20:27:55.900Z".into())
            );
            assert_eq!(
                run(r#"time.from_string "2021-12-14T20:27:55.900Z""#),
                Value::Numb(1639513675.9)
            );
        }

        #[test]
        fn test_to_date_and_from_date() {
            assert_eq!(
                run("d = time.to_UTC_date 1642290213.284, time.from_UTC_date d"),
                Value::Numb(1642290213.284)
            );
        }

        #[test]
        fn test_week_day_and_week_number() {
            // 2021-12-14 was a Tuesday (2) in week 50
            assert_eq!(run("time.week_day 1639513675.900"), Value::Numb(2.0));
            assert_eq!(run("time.week_number 1639513675.900"), Value::Numb(50.0));
        }
    }
}
