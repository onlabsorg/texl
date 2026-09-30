//! The `time` built-in namespace.
//!
//! Timestamps are `Numb` values holding seconds since the Unix epoch
//! (1970-01-01T00:00:00Z), always interpreted as UTC. Every function except
//! `now` is mapped over tuples via `map_1`: `Nothing` gives `Nothing`, a tuple
//! is mapped element-wise, and an unsupported argument type gives `Undefined`.
//!
//! Namespace members:
//! - `now()`         : current time
//! - `to_date(t)`    : timestamp -> namespace `{year, month, day, hour, minute, second}`
//! - `from_date(d)`  : date namespace -> timestamp (missing fields default to 1970-01-01T00:00:00)
//! - `week_day(t)`   : 0 = Sunday ... 6 = Saturday
//! - `week_number(t)`: ISO-8601 week number
//! - `to_string(t)`  : ISO-8601 text, e.g. `2026-05-28T20:26:40.000Z`
//! - `from_string(s)`: parse ISO-8601 text (date, or date + time)

use crate::value::{BuiltinFn, Scope, Undef, Value};
use super::helpers::*;



// ----------------------------------------------------------------------------
//  `time` built-in namespace
// ----------------------------------------------------------------------------

/// Builds the `time` namespace as a child of `parent`.
pub fn create_namespace(parent: &Scope) -> Value {
    let time_scope = parent.new_child();

    time_scope.set("now", Value::Builtin(BuiltinFn::new("time.now", time_now)));

    time_scope.set("to_date", map_1("time.to_date", time_to_date));
    time_scope.set("from_date", map_1("time.from_date", time_from_date));

    time_scope.set("week_day", map_1("time.week_day", time_week_day));
    time_scope.set("week_number", map_1("time.week_number", time_week_number));

    time_scope.set("to_string", map_1("time.to_string", time_to_string));
    time_scope.set("from_string", map_1("time.from_string", time_from_string));

    Value::Namespace(time_scope)
}



// ----------------------------------------------------------------------------
//  Built-in functions
// ----------------------------------------------------------------------------

/// `time.now()`: current time as epoch seconds (fractional). Any argument is ignored.
/// Returns 0 if the system clock is before the epoch.
fn time_now(_arg: Value) -> Value {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => Value::Numb(d.as_secs_f64()),
        Err(_) => Value::Numb(0.0),
    }
}

/// Returns a fresh namespace with `year, month, day, hour, minute, second`,
/// with `second` possibly fractional.
fn time_to_date(t: Value) -> Value {
    match t {
        Value::Numb(t) => {
            let dt = datetime_from_epoch(t);
            let scope = Scope::new();
            scope.set("year", Value::Numb(dt.year as f64));
            scope.set("month", Value::Numb(dt.month as f64));
            scope.set("day", Value::Numb(dt.day as f64));
            scope.set("hour", Value::Numb(dt.hour as f64));
            scope.set("minute", Value::Numb(dt.minute as f64));
            scope.set("second", Value::Numb(dt.second));
            Value::Namespace(scope)
        }
        other => undef_unary!("time.to_date", other),
    }
}

/// Missing or non-numeric fields default to year 0 / month 1 / day 1 / 00:00:00.
fn time_from_date(date: Value) -> Value {
    match date {
        Value::Namespace(scope) => {
            let y = numb_field(&scope, "year", 0.0) as i64;
            let m = numb_field(&scope, "month", 1.0) as i64;
            let d = numb_field(&scope, "day", 1.0) as i64;
            let h = numb_field(&scope, "hour", 0.0) as i64;
            let min = numb_field(&scope, "minute", 0.0) as i64;
            let s = numb_field(&scope, "second", 0.0);
            Value::Numb(epoch_from_parts(y, m, d, h, min, s))
        }
        other => undef_unary!("time.from_date", other),
    }
}

/// Formats the given timestamp as `YYYY-MM-DDTHH:MM:SS.mmmZ` (UTC, millisecond precision).
fn time_to_string(val: Value) -> Value {
    match val {
        Value::Numb(t) => {
            let dt = datetime_from_epoch(t);
            Value::Text(format!(
                "{:04}-{:02}-{:02}T{:02}:{:02}:{:06.3}Z",
                dt.year, dt.month, dt.day, dt.hour, dt.minute, dt.second
            ).into())
        }
        other => undef_unary!("time.to_string", other),
    }
}

/// Parses an ISO-8601 date/time string. Accepts `YYYY-MM-DD`, optionally followed by
/// `T`, `t` or a space, then `HH:MM` or `HH:MM:SS[.fff]`, with an optional trailing `Z`/`z`.
/// A missing time means midnight. Returns `Undefined` for unparsable text.
fn time_from_string(val: Value) -> Value {
    match val {
        Value::Text(s) => match parse_iso_string(&s) {
            Some(t) => Value::Numb(t),
            None => undef_unary!("time.from_string", Value::Text(s)),
        },
        other => undef_unary!("time.from_string", other),
    }
}

/// `time.week_day(t)`: 0 = Sunday … 6 = Saturday.
fn time_week_day(val: Value) -> Value {
    match val {
        Value::Numb(t) => {
            let days = (t.floor() as i64).div_euclid(86400);
            // 1970-01-01 (day 0) was a Thursday, i.e. weekday 4
            let wd = (days + 4).rem_euclid(7);
            Value::Numb(wd as f64)
        }
        other => undef_unary!("time.week_day", other),
    }
}

/// `time.week_number(t)`: ISO-8601 week number (weeks start Monday; week 1 contains the year's first Thursday).
fn time_week_number(val: Value) -> Value {
    match val {
        Value::Numb(t) => {
            let days = (t.floor() as i64).div_euclid(86400);
            // weekday with Monday = 0 (day 0 was a Thursday, so +3)
            let wday_mon0 = (days + 3).rem_euclid(7);
            // the Thursday of the current ISO week. Its calendar year is the ISO year.
            let thu_days = days + 3 - wday_mon0;
            let (thu_y, _, _) = civil_from_days(thu_days);
            // January 4th is always in ISO week 1. `jan4_thu_days` is the Thursday of that week.
            let jan4_days = days_from_civil(thu_y, 1, 4);
            let jan4_wday_mon0 = (jan4_days + 3).rem_euclid(7);
            let jan4_thu_days = jan4_days + 3 - jan4_wday_mon0;
            // 1 + the number of whole weeks between the two Thursdays
            let wn = 1 + (thu_days - jan4_thu_days) / 7;
            Value::Numb(wn as f64)
        }
        other => undef_unary!("time.week_number", other),
    }
}



// ----------------------------------------------------------------------------
//  Private helpers
// ----------------------------------------------------------------------------

/// A broken-down UTC date and time.
struct DateTime {
    year: i64, month: u32, day: u32,
    hour: i64, minute: i64, second: f64, // `second` includes the fractional part
}

/// Splits epoch seconds into calendar fields (UTC). Works for negative times too.
fn datetime_from_epoch(t: f64) -> DateTime {
    let t_floor = t.floor() as i64;
    let sub_sec = t - (t_floor as f64);
    let days = t_floor.div_euclid(86400);
    let rem_secs = t_floor.rem_euclid(86400);
    let (year, month, day) = civil_from_days(days);
    DateTime {
        year, month, day,
        hour: rem_secs / 3600,
        minute: (rem_secs % 3600) / 60,
        second: (rem_secs % 60) as f64 + sub_sec,
    }
}

/// Combines calendar fields (UTC) into epoch seconds.
fn epoch_from_parts(y: i64, m: i64, d: i64, h: i64, min: i64, sec: f64) -> f64 {
    let days = days_from_civil(y, m, d);
    (days * 86400 + h * 3600 + min * 60) as f64 + sec
}

/// Reads a numeric field from a date namespace, or returns `default`
/// if the field is missing or not a `Numb`.
fn numb_field(scope: &Scope, name: &str, default: f64) -> f64 {
    match scope.get(name) {
        Value::Numb(n) => n,
        _ => default,
    }
}

/// Convert days since 1970-01-01 to (year, month, day) in civil Gregorian calendar.
///
/// This implementation uses Howard Hinnant's `civil_from_days` algorithm,
/// which works for the proleptic Gregorian calendar (i.e., it's valid
/// for dates before 1582 as well, though the results might not match
/// historical records).
///
/// `z`   : days since 0000-03-01
/// `era` : 400-year cycle
/// `doe` : day of era
/// `yoe` : year of era
/// `doy` : day of year counted from March 1
/// `mp`  : month counted from March
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
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
///
/// This implementation uses Howard Hinnant's `days_from_civil` algorithm,
/// which works for the proleptic Gregorian calendar.
///
/// `y`   : year (adjusted for March-based year start)
/// `era` : 400-year cycle
/// `yoe` : year of era
/// `doy` : day of year (0-indexed, from March 1st)
/// `doe` : day of era
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Parse an ISO-8601 date/time string into epoch seconds.
///
/// Accepts forms like:
/// - `YYYY-MM-DD` (time defaults to 00:00:00Z)
/// - `YYYY-MM-DDTHH:MM`
/// - `YYYY-MM-DDtHH:MM:SS`
/// - `YYYY-MM-DD HH:MM:SS.fffZ`
/// An optional trailing `Z` or `z` is ignored. Returns `None` on any malformed input,
/// including out-of-range month/day values.
fn parse_iso_string(s: &str) -> Option<f64> {
    let s = s.trim();
    // Split into date and optional time parts
    let parts: Vec<&str> = s.split(|c| c == 'T' || c == 't' || c == ' ').collect();
    if parts.is_empty() {
        return None;
    }

    // Parse date part (YYYY-MM-DD)
    let date_parts: Vec<&str> = parts[0].split('-').collect();
    if date_parts.len() != 3 {
        return None;
    }
    let y: i64 = date_parts[0].parse().ok()?;
    let m: i64 = date_parts[1].parse().ok()?;
    let d: i64 = date_parts[2].parse().ok()?;

    // Validate month and day ranges
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }

    // Parse optional time part (HH:MM:SS[.fff])
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

    Some(epoch_from_parts(y, m, d, h, min, sec))
}



// ----------------------------------------------------------------------------
//  Unit tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;
    use crate::evaluator::eval;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    /// Minimal root scope: the `time` namespace plus the boolean constants.
    /// Returns a child scope, mirroring how user code is evaluated.
    fn root_scope() -> Scope {
        let scope = Scope::new();
        scope.set("time", create_namespace(&scope));
        scope.set("TRUE", Value::Bool(true));
        scope.set("FALSE", Value::Bool(false));
        scope.new_child()
    }

    /// Lexes, parses and evaluates `src` in a fresh test scope.
    fn run(src: &str) -> Value {
        let scope = root_scope();
        let mut parser = Parser::new(Lexer::new(src));
        let ast = parser.parse();
        eval(&ast, &scope)
    }

    /// `time.now()`: returns a number.
    #[test]
    fn test_now() {
        assert!(matches!(run("time.now()"), Value::Numb(_)));
    }

    /// `time.to_date`: timestamp to date namespace.
    #[test]
    fn test_to_date() {
        // Number
        assert_eq!(
            run("d = time.to_date(1780000000), d.(year, month, day, hour, minute, second)"),
            Value::Tuple(Rc::new(vec![
                Value::Numb(2026.0), // year
                Value::Numb(5.0),    // month
                Value::Numb(28.0),   // day
                Value::Numb(20.0),   // hour (UTC)
                Value::Numb(26.0),   // minute
                Value::Numb(40.0),   // second
            ]))
        );

        // Nothing
        assert_eq!(run("time.to_date()"), Value::Nothing);

        // Tuple
        assert_eq!(
            run("(d1,d2) = time.to_date(1780000000, 1780000009), d1.[minute, second], d2.[minute, second]"),
            Value::Tuple(Rc::new(vec![
                Value::List(Rc::new(vec![Value::Numb(26.0), Value::Numb(40.0)])),
                Value::List(Rc::new(vec![Value::Numb(26.0), Value::Numb(49.0)])),
            ]))
        );

        // Others
        assert!(matches!(run("time.to_date []"), Value::Undefined(_)));
    }

    /// `time.from_date`: date namespace to timestamp.
    #[test]
    fn test_from_date() {
        // Number
        assert_eq!(
            run("time.from_date{year:2026, month:5, day:28, hour:20, minute:26, second:40}"),
            Value::Numb(1780000000.0)
        );

        // Missing fields default to 1970-01-01T00:00:00
        assert_eq!(run("time.from_date{year:1970}"), Value::Numb(0.0));

        // Nothing
        assert_eq!(run("time.from_date()"), Value::Nothing);

        // Tuple
        let d1 = "{year:2026, month:5, day:28, hour:20, minute:26, second:40}";
        let d2 = "{year:2026, month:5, day:28, hour:20, minute:26, second:49}";
        assert_eq!(
            run(&format!("time.from_date({}, {})", d1, d2)),
            Value::Tuple(Rc::new(vec![
                Value::Numb(1780000000.0),
                Value::Numb(1780000009.0),
            ]))
        );

        // Others
        assert!(matches!(run("time.from_date []"), Value::Undefined(_)));
    }

    /// `time.week_day`: returns day of week.
    #[test]
    fn test_week_day() {
        // Number (Sunday = 0 ... Saturday = 6)
        assert_eq!(run("time.week_day(1780000000)"), Value::Numb(4.0)); // Thursday
        assert_eq!(run("time.week_day(1780259200)"), Value::Numb(0.0)); // Sunday
        assert_eq!(run("time.week_day(1780345600)"), Value::Numb(1.0)); // Monday
        assert_eq!(run("time.week_day(0)"), Value::Numb(4.0));          // 1970-01-01, Thursday

        // Nothing
        assert_eq!(run("time.week_day()"), Value::Nothing);

        // Tuple
        assert_eq!(
            run("time.week_day(1780000000, 1780259200, 1780345600)"),
            Value::Tuple(Rc::new(vec![
                Value::Numb(4.0),
                Value::Numb(0.0),
                Value::Numb(1.0),
            ]))
        );

        // Others
        assert!(matches!(run("time.week_day([])"), Value::Undefined(_)));
    }

    /// `time.week_number`: returns ISO week number.
    #[test]
    fn test_week_number() {
        // Number (ISO-8601 week numbers)
        assert_eq!(run("time.week_number(1780000000)"), Value::Numb(22.0)); // Thursday
        assert_eq!(run("time.week_number(1780259200)"), Value::Numb(22.0)); // Sunday, same week
        assert_eq!(run("time.week_number(1780345600)"), Value::Numb(23.0)); // Monday, next week
        assert_eq!(run("time.week_number(0)"), Value::Numb(1.0));          // 1970-01-01 is in ISO week 1

        // Nothing
        assert_eq!(run("time.week_number()"), Value::Nothing);

        // Tuple
        assert_eq!(
            run("time.week_number(1780000000, 1780345600)"),
            Value::Tuple(Rc::new(vec![
                Value::Numb(22.0),
                Value::Numb(23.0),
            ]))
        );

        // Others
        assert!(matches!(run("time.week_number([])"), Value::Undefined(_)));
    }

    /// `time.to_string`: formats timestamp to ISO-8601 string.
    #[test]
    fn test_to_string() {
        // Number
        assert_eq!(
            run("time.to_string(1780000000)"),
            Value::Text("2026-05-28T20:26:40.000Z".into())
        );

        // Number with fractional seconds
        assert_eq!(
            run("time.to_string(1780000000.5)"),
            Value::Text("2026-05-28T20:26:40.500Z".into())
        );
        assert_eq!(run("time.to_string(0)"), Value::Text("1970-01-01T00:00:00.000Z".into()));

        // Nothing
        assert_eq!(run("time.to_string()"), Value::Nothing);

        // Tuple
        assert_eq!(
            run("time.to_string(1780000000, 1780000009)"),
            Value::Tuple(Rc::new(vec![
                Value::Text("2026-05-28T20:26:40.000Z".into()),
                Value::Text("2026-05-28T20:26:49.000Z".into()),
            ]))
        );

        // Others
        assert!(matches!(run("time.to_string([])"), Value::Undefined(_)));
        assert!(matches!(run(r#"time.to_string("abc")"#), Value::Undefined(_)));
    }

    /// `time.from_string`: parses ISO-8601 string to timestamp.
    #[test]
    fn test_from_string() {
        // Text (full date-time)
        assert_eq!(
            run(r#"time.from_string("2026-05-28T20:26:40.000Z")"#),
            Value::Numb(1780000000.0)
        );

        // Text (date only -> midnight UTC)
        assert_eq!(
            run(r#"time.from_string("2026-05-28")"#),
            Value::Numb(1779926400.0)
        );

        // Space as date/time separator, no trailing Z
        assert_eq!(
            run(r#"time.from_string("2026-05-28 20:26:40")"#),
            Value::Numb(1780000000.0)
        );

        // Nothing
        assert_eq!(run("time.from_string()"), Value::Nothing);

        // Tuple
        assert_eq!(
            run(r#"time.from_string("2026-05-28T20:26:40Z", "2026-05-28T20:26:49Z")"#),
            Value::Tuple(Rc::new(vec![
                Value::Numb(1780000000.0),
                Value::Numb(1780000009.0),
            ]))
        );

        // Round trip
        assert_eq!(
            run("time.from_string(time.to_string(1780000000))"),
            Value::Numb(1780000000.0)
        );

        // Others
        assert!(matches!(run("time.from_string([])"), Value::Undefined(_)));
        assert!(matches!(run(r#"time.from_string("not a date")"#), Value::Undefined(_)));
        assert!(matches!(run(r#"time.from_string("2026-13-01")"#), Value::Undefined(_)));
        assert!(matches!(run(r#"time.from_string("2026-05-00")"#), Value::Undefined(_)));
    }
}
