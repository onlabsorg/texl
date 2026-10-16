//! The abstract syntax tree produced by the [`Parser`](crate::parser::Parser).
//!
//! A parsed expression is a tree of [`Node`]s. Every node records `pos`, the
//! byte offset in the source of the token that produced it (for a binary
//! operation, the operator; for function application, which has no operator,
//! the start of the argument). Evaluation errors report this position.
//!
//! Syntax errors do not stop parsing. They become [`Node::Unclosed`] or
//! [`Node::UnexpectedToken`] leaves, which evaluate to `Undefined` values.


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
    /// The right-hand operand.
    pub rho: Box<Node>,
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

}
