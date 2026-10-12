


#[derive(Debug, PartialEq, Clone)]
pub struct Leaf<T> {
    pub pos: usize,
    pub val: T,
}


#[derive(Debug, PartialEq, Clone)]
pub struct UnaryOp {
    pub pos: usize,
    pub opd: Box<Node>,
}


#[derive(Debug, PartialEq, Clone)]
pub struct BinaryOp {
    pub pos: usize,
    pub lho: Box<Node>,
    pub rho: Box<Node>,
}




#[derive(Debug, PartialEq, Clone)]
pub enum Node {

    // Literals / Leaves
    Identifier(Leaf<String>),
    Numb(Leaf<f64>),
    Text(Leaf<String>),
    Nothing(Leaf<()>),
    Unclosed(Leaf<char>),
    UnexpectedToken(Leaf<String>),


    // Grouping 
    List(UnaryOp),          // [ ... ]
    Namespace(UnaryOp),     // { ... }


    // Unary Operations
    AffirmOp(UnaryOp),          // +
    NegateOp(UnaryOp),          // -
    LogicalNegationOp(UnaryOp), // !
    LiftingOp(UnaryOp),         // $


    // Binary Operations :: Core
    FunctionDefinitionOp(BinaryOp), // -> 
    ApplicationOp(BinaryOp),        // no operator
    PairingOp(BinaryOp),            // ,
    MappingOp(BinaryOp),            // =>
    AssignmentOp(BinaryOp),         // =
    LabellingOp(BinaryOp),          // :
    SubcontextingOp(BinaryOp),      // .
    ThenOp(BinaryOp),               // ~>


    // Binary Operations :: Arithmetic
    PowerOp(BinaryOp),          // ^
    ProductOp(BinaryOp),        // *
    DivisionOp(BinaryOp),       // /
    ModuloOp(BinaryOp),         // %
    SumOp(BinaryOp),            // +
    SubtractionOp(BinaryOp),    // -


    // Binary Operations :: Comparison
    EqualToOp(BinaryOp),                // ==
    NotEqualToOp(BinaryOp),             // !=
    LessThanOp(BinaryOp),               // <
    LessThanOrEqualToOp(BinaryOp),      // <=
    GreaterThanOp(BinaryOp),            // >
    GreaterThanOrEqualToOp(BinaryOp),   // >=


    // Binary Operations :: Logic
    AndOp(BinaryOp),            // &
    OrOp(BinaryOp),             // |


    // Binary Operations :: Conditional
    ConditionalOp(BinaryOp),    // ?
    AlternativeOp(BinaryOp),    // ;


    // Binary Operations :: Function Composition
    CompositionOp(BinaryOp),         // <<
    ReverseCompositionOp(BinaryOp),  // >>
}


impl Node {
    
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



