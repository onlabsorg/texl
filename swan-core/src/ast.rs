//! The abstract syntax tree produced by the [`Parser`](crate::parser::Parser).
//!
//! A parsed expression is a tree of [`Node`]s. Every node records `pos`, the
//! byte offset in the source of the token that produced it (for a binary
//! operation, the operator; for function application, which has no operator,
//! the start of the argument). Evaluation errors report this position.
//!
//! Syntax errors do not stop parsing. They become [`Node::Unclosed`] or
//! [`Node::UnexpectedToken`] leaves, which evaluate to `Undefined` values.


use std::rc::Rc;

/// A leaf node: a literal value and its source position.
#[derive(Debug, PartialEq, Clone)]
pub struct Leaf<T> {
    /// Byte offset of the token in the source.
    pub pos: usize,
    /// The literal value.
    pub val: T,
}


/// A node with one operand: a prefix operator or a bracketed group.
#[derive(Debug, PartialEq, Clone)]
pub struct UnaryOp {
    /// Byte offset of the operator or opening bracket in the source.
    pub pos: usize,
    /// The operand.
    pub opd: Box<Node>,
}


/// A node with two operands: an infix operator or function application.
#[derive(Debug, PartialEq, Clone)]
pub struct BinaryOp {
    /// Byte offset of the operator in the source. For function application,
    /// which has no operator, it is the byte offset of the argument.
    pub pos: usize,
    /// The left-hand operand.
    pub lho: Box<Node>,
    /// The right-hand operand. It is shared, not owned: for `X -> Y` it is
    /// the body of the function, which every value of that function holds,
    /// so that defining, copying or calling a function does not copy its
    /// syntax tree.
    pub rho: Rc<Node>,
}




/// A node of the Swan syntax tree.
///
/// The comment on each variant shows the syntax that produces it.
#[derive(Debug, PartialEq, Clone)]
pub enum Node {

    // Literals / Leaves

    /// A name, such as `x` or `my_var`.
    Identifier(Leaf<String>),
    /// A number literal, such as `42` or `1.5e-3`.
    Numb(Leaf<f64>),
    /// A text literal, such as `"hello"`. The value excludes the quotes.
    Text(Leaf<String>),
    /// `()`, or an empty expression.
    Nothing(Leaf<()>),
    /// Syntax error: a group that is not closed. The value is the missing
    /// closing bracket (`)`, `]` or `}`).
    Unclosed(Leaf<char>),
    /// Syntax error: a token that cannot appear where it was found. The value
    /// is a debug description of the token.
    UnexpectedToken(Leaf<String>),
    /// A syntax error: the expression is nested deeper than the parser goes
    /// (see [`MAX_NESTING`](crate::parser::MAX_NESTING)). `pos` is where the
    /// parser stopped.
    TooDeep(Leaf<()>),


    // Grouping

    /// `[X]`: a list literal.
    List(UnaryOp),
    /// `{X}`: a namespace literal.
    Namespace(UnaryOp),


    // Unary Operations

    /// `+X`
    AffirmOp(UnaryOp),
    /// `-X`
    NegateOp(UnaryOp),
    /// `!X`
    LogicalNegationOp(UnaryOp),
    /// `$X`
    LiftingOp(UnaryOp),


    // Binary Operations :: Core

    /// `X -> Y`
    FunctionDefinitionOp(BinaryOp),
    /// `X Y`: function application, written by juxtaposition.
    ApplicationOp(BinaryOp),
    /// `X , Y`
    PairingOp(BinaryOp),
    /// `X => Y`
    MappingOp(BinaryOp),
    /// `X = Y`
    AssignmentOp(BinaryOp),
    /// `X : Y`
    LabellingOp(BinaryOp),
    /// `X . Y`
    SubcontextingOp(BinaryOp),
    /// `X ~> Y`
    ThenOp(BinaryOp),


    // Binary Operations :: Arithmetic

    /// `X ^ Y`
    PowerOp(BinaryOp),
    /// `X * Y`
    ProductOp(BinaryOp),
    /// `X / Y`
    DivisionOp(BinaryOp),
    /// `X % Y`
    ModuloOp(BinaryOp),
    /// `X + Y`
    SumOp(BinaryOp),
    /// `X - Y`
    SubtractionOp(BinaryOp),


    // Binary Operations :: Comparison

    /// `X == Y`
    EqualToOp(BinaryOp),
    /// `X != Y`
    NotEqualToOp(BinaryOp),
    /// `X < Y`
    LessThanOp(BinaryOp),
    /// `X <= Y`
    LessThanOrEqualToOp(BinaryOp),
    /// `X > Y`
    GreaterThanOp(BinaryOp),
    /// `X >= Y`
    GreaterThanOrEqualToOp(BinaryOp),


    // Binary Operations :: Logic

    /// `X & Y`
    AndOp(BinaryOp),
    /// `X | Y`
    OrOp(BinaryOp),


    // Binary Operations :: Conditional

    /// `X ? Y`
    ConditionalOp(BinaryOp),
    /// `X ; Y`
    AlternativeOp(BinaryOp),


    // Binary Operations :: Function Composition

    /// `X << Y`
    CompositionOp(BinaryOp),
    /// `X >> Y`
    ReverseCompositionOp(BinaryOp),
}


impl Node {

    /// Returns the source byte offset recorded in this node.
    pub fn position (&self) -> usize {
        match self {

            Self::Identifier(leaf)      => leaf.pos,
            Self::Numb(leaf)            => leaf.pos,
            Self::Text(leaf)            => leaf.pos,
            Self::Nothing(leaf)         => leaf.pos,
            Self::Unclosed(leaf)        => leaf.pos,
            Self::UnexpectedToken(leaf) => leaf.pos,
            Self::TooDeep(leaf)         => leaf.pos,

            Self::List(unary)      => unary.pos,
            Self::Namespace(unary) => unary.pos,

            Self::AffirmOp(unary)          => unary.pos,
            Self::NegateOp(unary)          => unary.pos,
            Self::LogicalNegationOp(unary) => unary.pos,
            Self::LiftingOp(unary)         => unary.pos,

            Self::FunctionDefinitionOp(binary) => binary.pos,
            Self::ApplicationOp(binary)        => binary.pos,
            Self::PairingOp(binary)            => binary.pos,
            Self::MappingOp(binary)            => binary.pos,
            Self::AssignmentOp(binary)         => binary.pos,
            Self::LabellingOp(binary)          => binary.pos,
            Self::SubcontextingOp(binary)      => binary.pos,
            Self::ThenOp(binary)               => binary.pos,

            Self::PowerOp(binary)       => binary.pos,
            Self::ProductOp(binary)     => binary.pos,
            Self::DivisionOp(binary)    => binary.pos,
            Self::ModuloOp(binary)      => binary.pos,
            Self::SumOp(binary)         => binary.pos,
            Self::SubtractionOp(binary) => binary.pos,

            Self::EqualToOp(binary)              => binary.pos,
            Self::NotEqualToOp(binary)           => binary.pos,
            Self::LessThanOp(binary)             => binary.pos,
            Self::LessThanOrEqualToOp(binary)    => binary.pos,
            Self::GreaterThanOp(binary)          => binary.pos,
            Self::GreaterThanOrEqualToOp(binary) => binary.pos,

            Self::AndOp(binary) => binary.pos,
            Self::OrOp(binary)  => binary.pos,

            Self::ConditionalOp(binary) => binary.pos,
            Self::AlternativeOp(binary) => binary.pos,

            Self::CompositionOp(binary)        => binary.pos,
            Self::ReverseCompositionOp(binary) => binary.pos,
        }
    }

    /// Whether this node has no operands.
    fn is_leaf(&self) -> bool {
        matches!(
            self,
            Self::Identifier(_) | Self::Numb(_) | Self::Text(_) | Self::Nothing(_)
                | Self::Unclosed(_) | Self::UnexpectedToken(_) | Self::TooDeep(_)
        )
    }

    /// Moves the operands of this node that have operands of their own into
    /// `out`, leaving `Nothing` leaves in their place.
    fn take_operands(&mut self, out: &mut Vec<Node>) {
        let mut take = |operand: &mut Box<Node>| {
            if !operand.is_leaf() {
                out.push(std::mem::replace(&mut **operand, Node::Nothing(Leaf { pos: 0, val: () })));
            }
        };
        match self {
            Self::Identifier(_) | Self::Numb(_) | Self::Text(_) | Self::Nothing(_)
            | Self::Unclosed(_) | Self::UnexpectedToken(_) | Self::TooDeep(_) => {}

            Self::List(unary) | Self::Namespace(unary) | Self::AffirmOp(unary)
            | Self::NegateOp(unary) | Self::LogicalNegationOp(unary) | Self::LiftingOp(unary) => {
                take(&mut unary.opd);
            }

            Self::FunctionDefinitionOp(binary) | Self::ApplicationOp(binary) | Self::PairingOp(binary)
            | Self::MappingOp(binary) | Self::AssignmentOp(binary) | Self::LabellingOp(binary)
            | Self::SubcontextingOp(binary) | Self::ThenOp(binary) | Self::PowerOp(binary)
            | Self::ProductOp(binary) | Self::DivisionOp(binary) | Self::ModuloOp(binary)
            | Self::SumOp(binary) | Self::SubtractionOp(binary) | Self::EqualToOp(binary)
            | Self::NotEqualToOp(binary) | Self::LessThanOp(binary) | Self::LessThanOrEqualToOp(binary)
            | Self::GreaterThanOp(binary) | Self::GreaterThanOrEqualToOp(binary) | Self::AndOp(binary)
            | Self::OrOp(binary) | Self::ConditionalOp(binary) | Self::AlternativeOp(binary)
            | Self::CompositionOp(binary) | Self::ReverseCompositionOp(binary) => {
                take(&mut binary.lho);
                // A right operand that a function still holds as its body is
                // left alone: the last to hold it frees it, in the same way
                if let Some(rho) = Rc::get_mut(&mut binary.rho) {
                    if !rho.is_leaf() {
                        out.push(std::mem::replace(rho, Node::Nothing(Leaf { pos: 0, val: () })));
                    }
                }
            }
        }
    }

}

/// Frees a tree without recursion.
///
/// The derived drop would free the operands of a node from inside the drop of
/// the node, one level of stack for each level of the tree. A chain of
/// operations is a tree as deep as it is long (`1, 2, 3, …` nests on the
/// left), so a long list of items written in a document overflowed the stack
/// when its tree was freed. Here the nodes that have operands are moved to a
/// list and freed from there, each with leaves in place of its operands.
impl Drop for Node {
    fn drop(&mut self) {
        let mut pending = Vec::new();
        self.take_operands(&mut pending);
        while let Some(mut node) = pending.pop() {
            node.take_operands(&mut pending);
        }
    }
}
