use super::*;
use crate::expression::{self, Expression};




// ----------------------------------------------------------------------------
//  Undefined
// ----------------------------------------------------------------------------

/// Why a value is undefined: the payload of [`Value::Undefined`].
///
/// Positions (`pos`) are byte offsets in the text of `expression`, the code
/// that made the value (see [`crate::expression`]). Built-in functions do not
/// know where they are called from: they make values with no position
/// (`pos: None`, no expression), and the evaluator gives them the position of
/// the call when the built-in returns them ([`Undef::place`]). Two `Undef`s
/// are equal when they have the same variant and contents, regardless of
/// position and expression.
///
/// [`Undef::describe`] explains the value in plain words, and
/// [`Undef::members`] lists its details; the Swan built-in `undef.inspect`
/// exposes both to Swan code.
#[derive(Debug, Clone)]
pub enum Undef {

    /// The result of a unary operation that is not defined for the operand type.
    UnaryOperation {
        /// Operation name (always a `&'static str` for built-in ops).
        /// If dunder-method dispatch (`__neg__`, etc.) is added later and
        /// user-defined error labels are needed, change this to `Rc<str>`.
        op : &'static str,
        /// The operand.
        val: Box<Value>,
        /// Source position of the operation, `None` if unknown.
        pos: Option<usize>,
        /// The expression `pos` refers to.
        expression: Option<Rc<Expression>>,
    },

    /// The result of a binary operation that is not defined for the operand types.
    BinaryOperation {
        /// Operation name, such as `"sum"` or `"application"`.
        op : &'static str,
        /// The left-hand operand.
        lhv: Box<Value>,
        /// The right-hand operand.
        rhv: Box<Value>,
        /// Source position of the operation, `None` if unknown.
        pos: Option<usize>,
        /// The expression `pos` refers to.
        expression: Option<Rc<Expression>>,
    },

    /// Syntax error: a group `(...)`, `[...]` or `{...}` without its closing
    /// bracket.
    Termination {
        /// The missing closing bracket.
        expected: char,
        /// Source position where the bracket was expected.
        pos: Option<usize>,
        /// The expression `pos` refers to.
        expression: Option<Rc<Expression>>,
    },

    /// Syntax error: a token that is not valid where it appears.
    Token {
        /// The token as written in the source.
        token: String,
        /// Source position of the token.
        pos: Option<usize>,
        /// The expression `pos` refers to.
        expression: Option<Rc<Expression>>,
    },

    /// A name was expected (left of `=`, `:` or `->`, or after `$`), but
    /// another expression was found.
    Name {
        /// Source position of the offending expression, `None` if unknown.
        pos: Option<usize>,
        /// The expression `pos` refers to.
        expression: Option<Rc<Expression>>,
    },

    /// An undefined value created explicitly, for example by the Swan
    /// built-in `undef.from`. It wraps an arbitrary value.
    Value(Box<Value>),
}

impl PartialEq for Undef {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Undef::UnaryOperation{op:op1, val:val1, ..},
             Undef::UnaryOperation{op:op2, val:val2, ..}) => {
                op1 == op2 && val1 == val2
            }
            (Undef::BinaryOperation{op:op1, lhv:lhv1, rhv:rhv1, ..},
             Undef::BinaryOperation{op:op2, lhv:lhv2, rhv:rhv2, ..}) => {
                op1 == op2 && lhv1 == lhv2 && rhv1 == rhv2
            }
            (Undef::Termination{expected:exp1, ..},
             Undef::Termination{expected:exp2, ..}) => {
                exp1 == exp2
            }
            (Undef::Token{token:token1, ..},
             Undef::Token{token:token2, ..}) => {
                token1 == token2
            }
            (Undef::Name{..},
             Undef::Name{..}) => {
                true
            }
            (Undef::Value(val1),
             Undef::Value(val2)) => {
                val1 == val2
            }
            _ => false,
        }
    }
}


impl Undef {

    /// The kind of undefined value: the variant name, such as
    /// `"BinaryOperation"`. It is the `type` member of `undef.inspect`.
    pub fn kind(&self) -> &'static str {
        match self {
            Undef::UnaryOperation { .. } => "UnaryOperation",
            Undef::BinaryOperation { .. } => "BinaryOperation",
            Undef::Termination { .. } => "Termination",
            Undef::Token { .. } => "Token",
            Undef::Name { .. } => "Name",
            Undef::Value(_) => "Value",
        }
    }

    /// The byte offset in the text of [`expression`](Self::expression) where
    /// the value was made. `None` for [`Undef::Value`], and for values made by
    /// built-in functions that have not been given the position of their call
    /// (see [`Undef::place`]).
    pub fn pos(&self) -> Option<usize> {
        match self {
            Undef::UnaryOperation { pos, .. }
            | Undef::BinaryOperation { pos, .. }
            | Undef::Termination { pos, .. }
            | Undef::Token { pos, .. }
            | Undef::Name { pos, .. } => *pos,
            Undef::Value(_) => None,
        }
    }

    /// Gives a value with no position the position `pos` in `expression`;
    /// a value that has a position, and [`Undef::Value`], are left as they
    /// are. The evaluator calls it on what a built-in function returns, with
    /// the position of the call: a built-in cannot know where it is called
    /// from.
    pub fn place(&mut self, at: usize, in_expression: Option<Rc<Expression>>) {
        match self {
            Undef::UnaryOperation { pos, expression, .. }
            | Undef::BinaryOperation { pos, expression, .. }
            | Undef::Termination { pos, expression, .. }
            | Undef::Token { pos, expression, .. }
            | Undef::Name { pos, expression } => {
                if pos.is_none() {
                    *pos = Some(at);
                    *expression = in_expression;
                }
            }
            Undef::Value(_) => {}
        }
    }

    /// The expression that [`pos`](Self::pos) refers to, if known.
    pub fn expression(&self) -> Option<&Rc<Expression>> {
        match self {
            Undef::UnaryOperation { expression, .. }
            | Undef::BinaryOperation { expression, .. }
            | Undef::Termination { expression, .. }
            | Undef::Token { expression, .. }
            | Undef::Name { expression, .. } => expression.as_ref(),
            Undef::Value(_) => None,
        }
    }

    /// The operands of the value, by member name: `val` for a unary
    /// operation and [`Undef::Value`], `lhv` and `rhv` for a binary one.
    pub fn operands(&self) -> Vec<(&'static str, &Value)> {
        match self {
            Undef::UnaryOperation { val, .. } | Undef::Value(val) => vec![("val", val)],
            Undef::BinaryOperation { lhv, rhv, .. } => vec![("lhv", lhv), ("rhv", rhv)],
            _ => vec![],
        }
    }

    /// The cause of this value: the first operand that is itself undefined
    /// (or the first undefined item of a tuple operand). Following `cause`
    /// leads to the root cause of an error. [`Undef::Value`] has no cause:
    /// it was made on purpose.
    pub fn cause(&self) -> Option<&Undef> {
        if let Undef::Value(_) = self { return None; }
        self.operands().into_iter().find_map(|(_, val)| match val {
            Value::Undefined(u) => Some(u),
            Value::Tuple(items) => items.iter().find_map(|v| match v {
                Value::Undefined(u) => Some(u),
                _ => None,
            }),
            _ => None,
        })
    }

    /// The details of the value, by member name, as `undef.inspect` shows
    /// them: `type`, `description`, the variant's fields (`op`, `val`,
    /// `lhv`, `rhv`, `expected`, `token`), `pos` (a byte offset in the
    /// expression, absent when there is no position) and `origin` (the
    /// expression's origin as text, when it has one). Operands are returned
    /// as they are, not inspected.
    pub fn members(&self) -> Vec<(&'static str, Value)> {
        let mut members = vec![
            ("type", Value::Text(self.kind().into())),
            ("description", Value::Text(self.describe().into())),
        ];
        match self {
            Undef::UnaryOperation { op, .. } | Undef::BinaryOperation { op, .. } => {
                members.push(("op", Value::Text((*op).into())));
            }
            Undef::Termination { expected, .. } => {
                members.push(("expected", Value::Text(expected.to_string().into())));
            }
            Undef::Token { token, .. } => {
                members.push(("token", Value::Text(token.as_str().into())));
            }
            _ => {}
        }
        for (name, val) in self.operands() {
            members.push((name, val.clone()));
        }
        if let Some(pos) = self.pos() {
            members.push(("pos", Value::Numb(pos as f64)));
        }
        if let Some(origin) = self.expression().and_then(|e| e.origin.as_ref()) {
            members.push(("origin", Value::Text(origin.to_string().into())));
        }
        members
    }

    /// Explains this value in plain words, such as "cannot add the text
    /// `"a"` and the number `1`".
    ///
    /// The description covers this level only. An operand that is itself
    /// undefined is called "an undefined value"; [`Undef::cause`] leads to
    /// its own description.
    pub fn describe(&self) -> String {
        match self {
            Undef::BinaryOperation { op, lhv, rhv, .. } => describe_binary(op, lhv, rhv),
            Undef::UnaryOperation { op, val, .. } => describe_unary(op, val),
            Undef::Termination { expected, .. } => format!("missing closing `{expected}`"),
            Undef::Token { token, .. } if token == "end of expression" => {
                "unexpected end of expression".to_string()
            }
            Undef::Token { token, .. } => format!("unexpected `{token}`"),
            Undef::Name { .. } => "expected a name".to_string(),
            Undef::Value(val) => format!("undefined on purpose, from {}", a_value(val)),
        }
    }
}


/// The sign of a binding operation, for messages.
fn binding_sign(op: &str) -> &'static str {
    match op {
        "assignment" => "=",
        "labelling" => ":",
        _ => "->",
    }
}

/// Describes a binary operation. All the wording of operations is here and in
/// [`describe_unary`].
fn describe_binary(op: &str, lhv: &Value, rhv: &Value) -> String {
    let (l, r) = (a_value(lhv), a_value(rhv));
    match op {
        "sum"         => format!("cannot add {l} and {r}"),
        "subtraction" => format!("cannot subtract {r} from {l}"),
        "product"     => format!("cannot multiply {l} by {r}"),
        "division"    => format!("cannot divide {l} by {r}"),
        "modulo"      => format!("cannot take {l} modulo {r}"),
        "power"       => format!("cannot raise {l} to the power of {r}"),
        "order"       => format!("cannot compare {l} with {r}"),
        "application" => format!("cannot apply {l} to {r}"),
        "subcontexting" => format!("cannot look up names in {l}: `.` needs a namespace"),
        "assignment" | "labelling" | "function_definition" => match lhv {
            Value::Namespace(_) => format!(
                "cannot bind names with `{}` here: the scope is read-only",
                binding_sign(op),
            ),
            _ => format!("the left side of `{}` must be a name or a tuple of names", binding_sign(op)),
        },
        _ => format!("`{op}` is not defined for {l} and {r}"),
    }
}

/// Describes a unary operation (an operator or a built-in function).
fn describe_unary(op: &str, val: &Value) -> String {
    match (op, val) {
        ("negation", _) => format!("cannot negate {}", a_value(val)),
        ("lifting", _) => "`$` must be followed by a name".to_string(),
        ("import.cycle", Value::Text(url)) => format!("import cycle: `{url}` is already being imported"),
        ("import.invalid_uri", Value::Text(err)) => format!("invalid URI: {err}"),
        _ => format!("`{op}` is not defined for {}", a_value(val)),
    }
}


/// Longest text or tuple quoted in a description.
const MAX_QUOTE: usize = 24;

/// Names a value for a description: by its literal when it is short (the
/// number `1`, the text `"abc"`), otherwise by its type (a list, a text).
fn a_value(val: &Value) -> String {
    match val {
        Value::Nothing => "nothing".to_string(),
        Value::Bool(_) | Value::Numb(_) | Value::Text(_) | Value::Name(_) | Value::Tuple(_) => {
            match quote(val) {
                Some(q) => format!("{} `{q}`", the_type(val)),
                None => a_type(val),
            }
        }
        other => a_type(other),
    }
}

/// A short Swan literal for `val`, or `None` if there is none or it is long.
fn quote(val: &Value) -> Option<String> {
    let q = match val {
        Value::Bool(b) => if *b { "TRUE".to_string() } else { "FALSE".to_string() },
        Value::Numb(n) => n.to_string(),
        Value::Text(t) if !t.contains('\n') => format!("{:?}", t.as_ref()),
        Value::Name(n) => n.to_string(),
        Value::Tuple(items) => {
            let items: Option<Vec<String>> = items.iter().map(quote).collect();
            format!("({})", items?.join(", "))
        }
        _ => return None,
    };
    (q.chars().count() <= MAX_QUOTE).then_some(q)
}

/// "the number", "the text"… used before a quoted literal.
fn the_type(val: &Value) -> String {
    match val {
        Value::Bool(_) => "the boolean",
        Value::Numb(_) => "the number",
        Value::Text(_) => "the text",
        Value::Name(_) => "the name",
        _ => "the tuple",
    }.to_string()
}

/// "a number", "a list"… used when the value is not quoted.
fn a_type(val: &Value) -> String {
    match val {
        Value::Nothing => "nothing".to_string(),
        Value::Bool(_) => "a boolean".to_string(),
        Value::Numb(_) => "a number".to_string(),
        Value::Text(_) => "a text".to_string(),
        Value::Name(_) => "a name".to_string(),
        Value::List(_) => "a list".to_string(),
        Value::Namespace(_) => "a namespace".to_string(),
        Value::Tuple(items) => format!("a tuple of {} items", items.len()),
        Value::Func { .. } => "a function".to_string(),
        Value::Builtin(f) => format!("the function `{}`", f.name),
        Value::Promise(_) => "a promise".to_string(),
        Value::Undefined(_) => "an undefined value".to_string(),
    }
}


impl Value {

    /// Builds `Undefined` for operation `op` (usually a built-in function's
    /// name, such as `"math.sqrt"`) applied to `val`, with no position: the
    /// helper for built-in and host functions. The evaluator gives the value
    /// the position of the call when the function returns it.
    #[inline]
    pub fn undef_unary(op: &'static str, val: Value) -> Value {
        Value::Undefined(Undef::UnaryOperation {
            op,
            val: Box::new(val),
            pos: None,
            expression: None,
        })
    }

    /// Builds `Undefined` for operation `op` applied to `lhv` and `rhv`, with
    /// no position: the helper for built-in and host functions (see
    /// [`Value::undef_unary`]).
    #[inline]
    pub fn undef_binary(op: &'static str, lhv: Value, rhv: Value) -> Value {
        Value::Undefined(Undef::BinaryOperation {
            op,
            lhv: Box::new(lhv),
            rhv: Box::new(rhv),
            pos: None,
            expression: None,
        })
    }

    /// Builds `Undefined` for unary operation `op` applied to `val`, made at
    /// byte offset `pos` of the current expression (see
    /// [`crate::expression`]): the helper for the evaluator's operations.
    #[inline]
    pub fn undef_unary_at(op: &'static str, val: Value, pos: usize) -> Value {
        Value::Undefined(Undef::UnaryOperation {
            op,
            val: Box::new(val),
            pos: Some(pos),
            expression: expression::current(),
        })
    }

    /// Builds `Undefined` for binary operation `op` applied to `lhv` and
    /// `rhv`, made at byte offset `pos` of the current expression: the helper
    /// for the evaluator's operations.
    #[inline]
    pub fn undef_binary_at(op: &'static str, lhv: Value, rhv: Value, pos: usize) -> Value {
        Value::Undefined(Undef::BinaryOperation {
            op,
            lhv: Box::new(lhv),
            rhv: Box::new(rhv),
            pos: Some(pos),
            expression: expression::current(),
        })
    }

    /// Gives every `Undefined` in this value with no position (the value
    /// itself, or the items of a tuple) the position `pos` of the current
    /// expression. See [`Undef::place`].
    pub fn place_undefined(self, pos: usize) -> Value {
        match self {
            Value::Undefined(mut undef) => {
                undef.place(pos, expression::current());
                Value::Undefined(undef)
            }
            Value::Tuple(items) if items.iter().any(|v| matches!(v, Value::Undefined(u) if u.pos().is_none())) => {
                let expression = expression::current();
                let items = items.iter().cloned().map(|v| match v {
                    Value::Undefined(mut undef) => {
                        undef.place(pos, expression.clone());
                        Value::Undefined(undef)
                    }
                    other => other,
                }).collect();
                Value::Tuple(Rc::new(items))
            }
            other => other,
        }
    }
}


// ----------------------------------------------------------------------------
//  UNIT TESTS
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtins::root_scope;
    use crate::evaluator::eval;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    /// An expression whose origin is the name `origin`.
    fn expr(text: &str, origin: &str) -> Rc<Expression> {
        Rc::new(Expression::new(text, Some(Rc::new(origin.to_string()))))
    }

    /// Evaluates `expr` in `scope`, with it as the current expression.
    fn run(expr: &Rc<Expression>, scope: &crate::value::Scope) -> Value {
        let ast = Parser::new(Lexer::new(&expr.text)).parse();
        expression::with_expression(Some(expr.clone()), || eval(&ast, scope))
    }

    /// The origin of `u`'s expression, as text.
    fn origin(u: &Undef) -> Option<String> {
        u.expression()?.origin.as_ref().map(|o| o.to_string())
    }

    /// Evaluates `text` as an expression from `doc` and returns its `Undef`.
    fn undef_of(text: &str) -> Undef {
        match run(&expr(text, "doc"), &root_scope()) {
            Value::Undefined(u) => u,
            other => panic!("not undefined: {other:?}"),
        }
    }

    fn describe(text: &str) -> String {
        undef_of(text).describe()
    }

    #[test]
    fn test_describe_binary_operations() {
        assert_eq!(describe(r#""a" + 1"#), r#"cannot add the text `"a"` and the number `1`"#);
        assert_eq!(describe(r#""a" - 1"#), r#"cannot subtract the number `1` from the text `"a"`"#);
        assert_eq!(describe("[1] * 2"), "cannot multiply a list by the number `2`");
        assert_eq!(describe("TRUE / 2"), "cannot divide the boolean `TRUE` by the number `2`");
        assert_eq!(describe("{} % 2"), "cannot take a namespace modulo the number `2`");
        assert_eq!(describe(r#"2 ^ "x""#), r#"cannot raise the number `2` to the power of the text `"x"`"#);
        assert_eq!(describe("TRUE < 1"), "cannot compare the boolean `TRUE` with the number `1`");
        assert_eq!(describe("10 20"), "cannot apply the number `10` to the number `20`");
        assert_eq!(describe("x = 10, x.y"), "cannot look up names in the number `10`: `.` needs a namespace");
        assert_eq!(describe("12 = 1"), "the left side of `=` must be a name or a tuple of names");
        assert_eq!(describe("12 : 1"), "the left side of `:` must be a name or a tuple of names");
        assert_eq!(describe("12 -> 1"), "the left side of `->` must be a name or a tuple of names");
        assert_eq!(describe("text.(size = 0)"), "cannot bind names with `=` here: the scope is read-only");
    }

    #[test]
    fn test_describe_unary_and_builtins() {
        assert_eq!(describe(r#"-"a""#), r#"cannot negate the text `"a"`"#);
        assert_eq!(describe("$12"), "`$` must be followed by a name");
        assert_eq!(describe("math.sqrt(-1)"), "`math.sqrt` is not defined for the number `-1`");
        assert_eq!(describe(r#"bind ("a", 5)"#), r#"`bind` is not defined for the tuple `("a", 5)`"#);
        assert_eq!(describe("text.size [1,2]"), "`text.size` is not defined for a list");
        let long = format!("bind({})", (0..20).map(|i| i.to_string()).collect::<Vec<_>>().join(","));
        assert_eq!(describe(&long), "`bind` is not defined for a tuple of 20 items");
        assert_eq!(describe("math.sqrt math.sqrt"), "`math.sqrt` is not defined for the function `math.sqrt`");
    }

    #[test]
    fn test_describe_syntax_errors_and_values() {
        assert_eq!(describe("(1 + 2"), "missing closing `)`");
        assert_eq!(describe("2 * {1"), "cannot multiply the number `2` by a namespace");
        assert_eq!(describe(")"), "unexpected `)`");
        assert_eq!(describe("1 + )"), "cannot add the number `1` and an undefined value");
        assert_eq!(undef_of("1 + )").cause().unwrap().describe(), "unexpected `)`");
        assert_eq!(describe("@"), "unexpected `@`");
        assert_eq!(undef_of("1 +").cause().unwrap().describe(), "unexpected end of expression");
        assert_eq!(describe("undef.from 5"), "undefined on purpose, from the number `5`");
        assert_eq!(describe("undef.from {}"), "undefined on purpose, from a namespace");
        let name = Undef::Name { pos: None, expression: None };
        assert_eq!(name.describe(), "expected a name");
    }

    #[test]
    fn test_unexpected_token_keeps_source_text() {
        // The token is described as written, not re-formatted
        let u = undef_of("2 + %");
        assert_eq!(u.cause().unwrap().describe(), "unexpected `%`");
        assert_eq!(undef_of("12abc").describe(), "unexpected `12abc`");
    }

    #[test]
    fn test_nested_causes() {
        let u = undef_of(r#"10 * ("a" - 1)"#);
        assert_eq!(u.describe(), "cannot multiply the number `10` by an undefined value");
        assert_eq!(u.pos(), Some(3));
        let cause = u.cause().unwrap();
        assert_eq!(cause.describe(), r#"cannot subtract the number `1` from the text `"a"`"#);
        assert_eq!(cause.pos(), Some(10));
        assert!(cause.cause().is_none());
        // An undefined item of a tuple operand is a cause too.
        let u = undef_of(r#"bind (1, "a" - 1)"#);
        assert_eq!(u.cause().unwrap().kind(), "BinaryOperation");
        // A value made on purpose has no cause.
        assert!(undef_of(r#"undef.from ("a" - 1)"#).cause().is_none());
    }

    #[test]
    fn test_positions_and_expressions() {
        let u = undef_of(r#""a" - 1"#);
        assert_eq!(u.pos(), Some(4));
        assert_eq!(&*u.expression().unwrap().text, r#""a" - 1"#);
        assert_eq!(origin(&u).as_deref(), Some("doc"));
        // A built-in's error has the position of the call (its argument).
        let u = undef_of("math.sqrt(-1)");
        assert_eq!(u.pos(), Some(9));
        assert_eq!(origin(&u).as_deref(), Some("doc"));
        // Made on purpose: no position.
        assert_eq!(undef_of("undef.from 1").pos(), None);
        // Made by a built-in, not yet returned to a call.
        let Value::Undefined(mut u) = Value::undef_unary("f", Value::Nothing) else { panic!() };
        assert_eq!(u.pos(), None);
        assert!(u.expression().is_none());
        u.place(3, None);
        assert_eq!(u.pos(), Some(3));
        u.place(5, None); // a position is never replaced
        assert_eq!(u.pos(), Some(3));
        // An expression with no origin.
        let ast = Parser::new(Lexer::new("1 - TRUE")).parse();
        let e = Some(Rc::new(Expression::from("1 - TRUE")));
        let Value::Undefined(u) = expression::with_expression(e, || eval(&ast, &root_scope())) else { panic!() };
        assert_eq!(u.pos(), Some(2));
        assert!(origin(&u).is_none());
    }

    #[test]
    fn test_values_keep_their_expression() {
        // A value made by expression A and read by expression B points into A.
        let scope = root_scope();
        run(&expr(r#"x = "a" - 1"#, "a"), &scope);
        let Value::Undefined(u) = run(&expr("x", "b"), &scope) else { panic!() };
        assert_eq!(origin(&u).as_deref(), Some("a"));
        assert_eq!(u.pos(), Some(8));
    }

    #[test]
    fn test_functions_keep_their_expression() {
        // `f` is defined in expression A and called from expression B: the
        // error points into A.
        let scope = root_scope();
        run(&expr(r#"f = x -> x - 1"#, "a"), &scope);
        let Value::Undefined(u) = run(&expr(r#"f "z""#, "b"), &scope) else { panic!() };
        assert_eq!(origin(&u).as_deref(), Some("a"));
        assert_eq!(u.pos(), Some(11));
        // The same through a built-in that calls back into Swan.
        scope.set("call_z", Value::Builtin(BuiltinFn::new("call_z", |f| {
            crate::evaluator::eval_apply(f, Value::Text("z".into()), 0)
        })));
        let Value::Undefined(u) = run(&expr("call_z f", "c"), &scope) else { panic!() };
        assert_eq!(origin(&u).as_deref(), Some("a"));
    }

    #[test]
    fn test_parsed_code_is_its_own_expression() {
        let u = undef_of(r#"(parse "1 + TRUE") {}"#);
        assert_eq!(u.pos(), Some(2));
        let e = u.expression().unwrap();
        assert!(e.origin.is_none());
        assert_eq!(&*e.text, "1 + TRUE");
    }

    #[test]
    fn test_members() {
        let u = undef_of(r#""a" - 1"#);
        let names: Vec<_> = u.members().into_iter().map(|(n, _)| n).collect();
        assert_eq!(names, ["type", "description", "op", "lhv", "rhv", "pos", "origin"]);
        let Value::Undefined(u) = Value::undef_unary("f", Value::Nothing) else { panic!() };
        let names: Vec<_> = u.members().into_iter().map(|(n, _)| n).collect();
        assert_eq!(names, ["type", "description", "op", "val"]);
        let u = undef_of("(1");
        let names: Vec<_> = u.members().into_iter().map(|(n, _)| n).collect();
        assert_eq!(names, ["type", "description", "expected", "pos", "origin"]);
    }
}
