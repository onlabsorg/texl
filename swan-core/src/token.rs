
//! Tokens produced by the [`Lexer`](crate::lexer::Lexer).
//!
//! The precedence numbers in the operator docs are the binding powers used by
//! the [`Parser`](crate::parser::Parser): a higher number binds tighter.
//! Function application, which has no token, binds at 28.

/// All token kinds produced by the [`Lexer`](crate::lexer::Lexer).
///
/// The lifetime `'a` is tied to the source string: `Identifier`, `Text`,
/// `Comment`, and `Undefined` variants borrow slices directly from it, so
/// no heap allocation is needed for those payloads.
#[derive(Debug, PartialEq, Clone)]
pub enum Token<'a> {

    // -------------------------------------------------------------------------
    // Literals and identifiers
    // -------------------------------------------------------------------------

    /// A name — starts with a letter or `_`, continues with letters, digits,
    /// or `_`.  Examples: `foo`, `_bar`, `myVar1`.
    Identifier(&'a str),

    /// A floating-point numeric literal.  Integer, decimal, and scientific
    /// notation are all parsed into an `f64`.  Examples: `42`, `3.14`,
    /// `1.5e-3`.
    Number(f64),

    /// A double-quoted string literal.  The slice contains only the content
    /// between the quotes — the delimiters are not included.
    /// Example: `"hello"` → `Text("hello")`.
    ///
    /// There are no escape sequences: a backslash is an ordinary character,
    /// and a text literal cannot contain `"`. A literal may span several lines.
    ///
    /// > **Note**: if the closing `"` is absent the slice extends to the end
    /// > of the source string (the lexer does not error on unterminated strings).
    Text(&'a str),

    // -------------------------------------------------------------------------
    //  Grouping
    // -------------------------------------------------------------------------

    /// `(`
    LeftRoundBracket,
    /// `)`
    RightRoundBracket,

    /// `[`  — opens a list literal.
    LeftSquareBracket,
    /// `]`  — closes a list literal.
    RightSquareBracket,

    /// `{`  — opens a namespace literal.
    LeftCurlyBracket,
    /// `}`  — closes a namespace literal.
    RightCurlyBracket,

    // -------------------------------------------------------------------------
    //  Binary and unary operators (listed low → high precedence)
    // -------------------------------------------------------------------------

    /// `,`  (precedence 10) — pairing / tuple construction.
    PairingOp,

    /// `=`  (precedence 12) — assignment.
    AssignmentOp,
    /// `:`  (precedence 12) — labelling / named argument.
    LabellingOp,

    /// `=>` (precedence 13, right-associative) — mapping: applies the
    /// right-hand function(s) to each item of the left-hand tuple.
    MappingOp,

    /// `~>` (precedence 13, left-associative) — then: chains a function to
    /// a promise.
    ThenOp,

    /// `>>` (precedence 14) — reverse composition / pipe (left-to-right).
    ReverseCompositionOp,
    /// `<<` (precedence 14) — composition (right-to-left).
    CompositionOp,

    /// `->` (precedence 15) — function definition (right-associative).
    FunctionDefinitionOp,

    /// `;`  (precedence 21) — alternative: evaluates the right-hand side only
    /// when the left-hand side is `Nothing`.
    AlternativeOp,

    /// `?`  (precedence 22) — conditional: evaluates the right-hand side only
    /// when the left-hand side is truthy.
    ConditionalOp,

    /// `|`  (precedence 23) — logical OR.
    OrOp,
    /// `&`  (precedence 23) — logical AND.
    AndOp,

    /// `==` (precedence 24) — equality comparison.
    EqualToOp,
    /// `!=` (precedence 24) — inequality comparison.
    NotEqualToOp,
    /// `<`  (precedence 24) — less-than comparison.
    LessThanOp,
    /// `<=` (precedence 24) — less-than-or-equal comparison.
    LessThanOrEqualToOp,
    /// `>`  (precedence 24) — greater-than comparison.
    GreaterThanOp,
    /// `>=` (precedence 24) — greater-than-or-equal comparison.
    GreaterThanOrEqualToOp,

    /// `+`  (precedence 25) — addition.
    PlusOp,
    /// `-`  (precedence 25) — subtraction.
    MinusOp,

    /// `*`  (precedence 26) — multiplication.
    ProductOp,
    /// `/`  (precedence 26) — division.
    DivisionOp,
    /// `%`  (precedence 26) — modulo.
    ModuloOp,

    /// `^`  (precedence 27) — exponentiation.
    PowerOp,

    /// `.`  (precedence 30) — sub-contexting: looks up the right-hand name
    /// inside the namespace produced by the left-hand expression.
    /// Example: `math.sin` retrieves `sin` from the `math` namespace.
    SubcontextingOp,

    /// `!`  (prefix) — logical negation.
    NotOp,

    /// `$`  (prefix) — lifting: looks a name up starting from the outermost
    /// scope.
    LiftingOp,

    // -------------------------------------------------------------------------
    // Special tokens
    // -------------------------------------------------------------------------

    /// A character sequence the lexer could not classify as any valid token.
    /// The slice covers exactly the unrecognised source characters.
    ///
    /// Examples: `'`, `@`, a malformed number like `1e+` or `12abc`.
    Undefined(&'a str),

    /// A `#`-prefixed line comment.  The slice contains the comment text
    /// *after* the `#` and *before* the terminating `\n` (or end of source).
    ///
    /// The lexer yields comments as tokens so that a tool such as a formatter
    /// can use them. The [`Parser`](crate::parser::Parser) skips them.
    Comment(&'a str),

    /// Signals that the source string has been fully consumed.
    ///
    /// `next_token` returns this sentinel instead of `Option::None` so that
    /// call-sites do not need to handle two distinct "no more input" states.
    /// The `Iterator` impl converts it to `None`.
    EndOfExpression,
}

