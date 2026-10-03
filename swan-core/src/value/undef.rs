use super::*;




// ----------------------------------------------------------------------------
//  Undefined
// ----------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum Undef {

    /// The result of a unary operation that is not defined for the operand type.
    UnaryOperation {
        /// Operation name (always a `&'static str` for built-in ops).
        /// If dunder-method dispatch (`__neg__`, etc.) is added later and
        /// user-defined error labels are needed, change this to `Rc<str>`.
        op : &'static str,
        val: Box<Value>,
        pos: usize,
    },

    /// The result of a binary operation that is not defined for the operand types.
    BinaryOperation {
        op : &'static str,
        lhv: Box<Value>,
        rhv: Box<Value>,
        pos: usize,
    },

    /// The result of a grounp (...), [...] or {...}, wihtout closing parenthesig
    Termination {
        expected: char,
        pos: usize,
    },

    /// The result of an undefined token
    Token {
        token: String,
        pos: usize,
    },

    /// Used when a name is expected but another value is found
    Name {
        pos: usize
    },

    /// A generic undefined value used mostly for user-defined undefined values
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


