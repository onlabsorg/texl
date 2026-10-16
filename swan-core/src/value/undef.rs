use super::*;




// ----------------------------------------------------------------------------
//  Undefined
// ----------------------------------------------------------------------------

/// Why a value is undefined: the payload of [`Value::Undefined`].
///
/// Positions (`pos`) are byte offsets in the evaluated source. Built-in
/// functions use position `0`. Two `Undef`s are equal when they have the same
/// variant and contents, regardless of position.
///
/// The Swan built-in `undef.inspect` exposes these fields to Swan code.
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
        /// Source position of the operation.
        pos: usize,
    },

    /// The result of a binary operation that is not defined for the operand types.
    BinaryOperation {
        /// Operation name, such as `"sum"` or `"application"`.
        op : &'static str,
        /// The left-hand operand.
        lhv: Box<Value>,
        /// The right-hand operand.
        rhv: Box<Value>,
        /// Source position of the operation.
        pos: usize,
    },

    /// Syntax error: a group `(...)`, `[...]` or `{...}` without its closing
    /// bracket.
    Termination {
        /// The missing closing bracket.
        expected: char,
        /// Source position where the bracket was expected.
        pos: usize,
    },

    /// Syntax error: a token that is not valid where it appears.
    Token {
        /// A description of the token.
        token: String,
        /// Source position of the token.
        pos: usize,
    },

    /// A name was expected (left of `=`, `:` or `->`, or after `$`), but
    /// another expression was found.
    Name {
        /// Source position of the offending expression.
        pos: usize
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


