
// =============================================================================
// parser.rs — Pratt (Top-Down Operator Precedence) parser for Swan
// =============================================================================
//
// What is a Pratt parser?
// -----------------------
// A Pratt parser is a recursive-descent parser where every token has two
// optional "parsing functions" associated with it:
//
//   • prefix_fn  — called when the token appears at the START of an
//                  expression (e.g. a number, an identifier, a `(`, a
//                  unary `-`).
//
//   • infix_fn   — called when the token appears BETWEEN two expressions
//                  (e.g. `+`, `*`, `.`).  The left-hand expression is
//                  passed in as an argument.
//
// The core loop (`parse_expr`) works like this:
//
//   1. Call `parse_prefix` to parse the left-hand side (lhs).
//   2. Peek at the next token.  If its BINDING POWER (precedence) is
//      greater than `min_bp`, consume the token and call `parse_infix`
//      to combine the lhs with the rhs into a new node.
//   3. Repeat step 2 until the next token's binding power is ≤ `min_bp`.
//
// Binding power controls associativity:
//   • Left-associative  (+, -, *, …) : pass the SAME power as the infix op
//                                      when recursing for the rhs.
//   • Right-associative (^, ->)      : pass ONE LESS than the infix op's
//                                      power so that the rhs grabs more.
//
// References
// ----------
// • Vaughan Pratt, "Top Down Operator Precedence" (1973)
// • https://matklad.github.io/2020/04/13/simple-but-powerful-pratt-parsing.html

use crate::ast::{BinaryOp, Leaf, Node, UnaryOp};
use crate::lexer::Lexer;
use crate::token::Token;





// =============================================================================
// Binding-power table
// =============================================================================
//
// Each infix operator has a LEFT and a RIGHT binding power (lbp / rbp).
// The parser compares the NEXT token's lbp against the current minimum; it
// consumes the token only when  lbp > min_bp.
//
// For a left-associative operator the rbp equals the lbp (so the same level
// cannot re-associate on the right).  For a right-associative operator the
// rbp is lbp - 1 (one less), allowing the same operator to appear again on
// the right and bind first.
//
// The numbers below match the precedence values documented in token.rs.

/// Returns the LEFT binding power of an infix/postfix operator token.
///
/// Returns `0` for tokens that are NOT infix operators (terminators, closing
/// brackets, unknown tokens). This causes the main loop to stop.
fn lbp(token: &Token) -> u32 {
    match token {
        // Lowest-precedence infix operators come first.
        Token::PairingOp             => 10,   // ,

        Token::AssignmentOp          => 12,   // =
        Token::LabellingOp           => 12,   // :

        Token::MappingOp             => 13,   // =>

        Token::ReverseCompositionOp  => 14,   // >>
        Token::CompositionOp         => 14,   // <<

        Token::FunctionDefinitionOp  => 15,   // ->

        Token::AlternativeOp         => 21,   // ;
        Token::ConditionalOp         => 22,   // ?

        Token::OrOp                  => 23,   // |
        Token::AndOp                 => 23,   // &

        Token::EqualToOp             => 24,   // ==
        Token::NotEqualToOp          => 24,   // !=
        Token::LessThanOp            => 24,   // <
        Token::LessThanOrEqualToOp   => 24,   // <=
        Token::GreaterThanOp         => 24,   // >
        Token::GreaterThanOrEqualToOp => 24,  // >=

        Token::PlusOp                => 25,   // +
        Token::MinusOp               => 25,   // -

        Token::ProductOp             => 26,   // *
        Token::DivisionOp            => 26,   // /
        Token::ModuloOp              => 26,   // %

        Token::PowerOp               => 27,   // ^  (right-assoc, see rbp)

        Token::SubcontextingOp       => 30,   // .

        // Everything else is not an infix operator → lbp = 0 stops the loop.
        _ => 0,
    }
}

/// Returns the RIGHT binding power of an infix operator token.
///
/// For left-associative operators this equals `lbp(token)` (the rhs
/// recurses at the same level, preventing re-association at this level).
/// For right-associative operators this is `lbp(token) - 1` (the rhs
/// recurses one level lower, so two operators at the same level bind
/// right-to-left).
fn rbp(token: &Token) -> u32 {
    match token {
        // Right-associative operators.
        Token::PowerOp              => lbp(token) - 1,   // ^
        Token::FunctionDefinitionOp => lbp(token) - 1,   // ->
        Token::MappingOp            => lbp(token) - 1,   // =>

        // All other infix operators are left-associative.
        _ => lbp(token),
    }
}





// =============================================================================
// Application (empty) operator
// =============================================================================
//
// The application operator has no token.  It is triggered when the look-ahead
// can begin an expression (i.e. it is in "prefix position") while the parser
// is in the middle of the infix loop.
//
// Binding power: 28 — tighter than exponentiation (27) so `f x ^ 2` parses
// as `(f x) ^ 2`, but looser than sub-contexting (30) so `f x.y` parses
// as `f (x.y)`.  Application is left-associative (lbp == rbp).

/// The left *and* right binding power of the implicit application operator.
const APPLICATION_BP: u32 = 28;

/// Returns `true` when `token` can start a new expression (prefix position).
///
/// These are exactly the tokens for which `parse_prefix` has a real (non-`_`)
/// branch.  When one of them appears in the look-ahead after a complete lhs,
/// we synthesise an `ApplicationOp` without consuming any infix token.
fn is_application_starter(token: &Token) -> bool {
    matches!(
        token,
        Token::Identifier(_)
            | Token::Number(_)
            | Token::Text(_)
            | Token::LeftRoundBracket
            | Token::LeftSquareBracket
            | Token::LeftCurlyBracket
    )
}





// =============================================================================
// Parser struct
// =============================================================================
//
// The parser keeps a ONE-TOKEN look-ahead.
//
// Why own Token instead of borrowing it?
// ---------------------------------------
// The lexer returns `(usize, Token<'a>)` by VALUE each time `next_token` is
// called. Storing those values as `&'a Token<'a>` would be storing a reference
// to a LOCAL variable that is about to be dropped — a classic Rust lifetime
// error. Owning the token (no `&`) is the correct fix.

pub struct Parser<'a> {
    /// The lexer feeds us tokens on demand.
    lexer: Lexer<'a>,

    /// The CURRENT token (just consumed) and its byte position in the source.
    /// Initialised to `(0, Token::EndOfExpression)` before the first real
    /// token is fetched.
    current_token: Token<'a>,
    current_pos:   usize,

    /// The NEXT token (one step ahead) and its position.
    /// Pre-filled by `new` so every call to peek is O(1).
    peek_token: Token<'a>,
    peek_pos:   usize,
}

impl<'a> Parser<'a> {

    // -------------------------------------------------------------------------
    // Construction
    // -------------------------------------------------------------------------

    /// Creates a new parser wrapping `lexer` and pre-loads the look-ahead.
    pub fn new(mut lexer: Lexer<'a>) -> Self {
        // Prime the look-ahead with the very first real token from the lexer.
        // `current_token` starts as a dummy; it will be replaced on the first
        // call to `advance`.
        let (peek_pos, peek_token) = lexer.next_token();
        Self {
            lexer,
            current_token: Token::EndOfExpression,
            current_pos:   0,
            peek_token,
            peek_pos,
        }
    }

    // -------------------------------------------------------------------------
    // Token management helpers
    // -------------------------------------------------------------------------

    /// Advances the parser by one token.
    ///
    /// Moves the look-ahead into `current_*` and fetches a new look-ahead from
    /// the lexer.
    ///
    /// `std::mem::replace` lets us MOVE `self.peek_token` out of the struct
    /// (which we can't do with a plain assignment because the struct must
    /// always remain fully initialised) while simultaneously writing the new
    /// look-ahead value into its place.
    fn advance(&mut self) {
        let (new_peek_pos, new_peek_token) = self.lexer.next_token();

        self.current_token = std::mem::replace(&mut self.peek_token, new_peek_token);
        self.current_pos   = self.peek_pos;
        self.peek_pos      = new_peek_pos;
    }

    /// Skips any `Comment` tokens from the look-ahead.
    ///
    /// The lexer yields comments as real tokens; the parser ignores them.
    /// Call this whenever you are about to inspect the look-ahead.
    fn skip_comments(&mut self) {
        // Keep advancing while the upcoming token is a comment.
        while matches!(self.peek_token, Token::Comment(_)) {
            self.advance();
        }
    }

    // -------------------------------------------------------------------------
    // Core Pratt parsing engine
    // -------------------------------------------------------------------------

    /// Parses a single complete expression at the given minimum binding power.
    ///
    /// `min_bp` is the binding power of the operator that called us.  We only
    /// consume infix operators whose left-binding-power is GREATER than
    /// `min_bp`; everything else is left for the caller to handle.
    ///
    /// Call with `min_bp = 0` to parse a full expression.
    ///
    /// # Algorithm (step by step)
    ///
    /// 1. Parse a PREFIX production → gives us the left-hand node (lhs).
    /// 2. Loop:
    ///    a. Skip comments and peek at the next token.
    ///    b. If `lbp(peek) <= min_bp` → STOP; return lhs.
    ///    c. Consume the infix token (advance).
    ///    d. Recursively parse the rhs with `min_bp = rbp(infix_token)`.
    ///    e. Combine lhs + rhs into a new node; that becomes the new lhs.
    pub fn parse_expr(&mut self, min_bp: u32) -> Node {

        // --- Step 1: prefix production ----------------------------------------
        self.skip_comments();
        if min_bp == 0 && matches!(self.peek_token, Token::EndOfExpression) {
            return Node::Nothing(Leaf { pos: self.peek_pos, val: () });
        }
        let mut lhs = self.parse_prefix();

        // --- Step 2: infix loop -----------------------------------------------
        loop {
            self.skip_comments();

            // --- Case A: implicit application operator ------------------------
            //
            // When the look-ahead can start an expression AND the application
            // binding power beats the current minimum, we treat the two adjacent
            // expressions as `lhs rhs` (function application).
            //
            // We do NOT consume any token here — `parse_prefix` inside the
            // recursive call to `parse_expr` will advance past the rhs on its
            // own.  The position we record for the node is the byte offset of
            // the rhs (i.e. `self.peek_pos`), since there is no operator token.
            if APPLICATION_BP > min_bp && is_application_starter(&self.peek_token) {
                let pos = self.peek_pos;
                // Right binding power equals left (left-associative), so we
                // recurse at APPLICATION_BP to prevent right-nesting at the
                // same level.
                let rhs = self.parse_expr(APPLICATION_BP);
                lhs = Node::ApplicationOp(BinaryOp {
                    pos,
                    lho: Box::new(lhs),
                    rho: Box::new(rhs),
                });
                continue;
            }

            // --- Case B: explicit infix operator ------------------------------

            // Peek: if the next token isn't an infix operator (or has too low
            // a binding power), we're done with this sub-expression.
            let next_lbp = lbp(&self.peek_token);
            if next_lbp <= min_bp {
                break;
            }

            // Consume the infix operator so `current_token` holds it.
            self.advance();

            // IMPORTANT: snapshot the infix operator NOW, before we recurse.
            // The recursive call to parse_expr will call advance() many times,
            // overwriting self.current_token with whatever the recursion
            // consumes last. If we don't save it here, parse_infix will see
            // the wrong token and panic.
            //
            // `Token` derives `Clone`, so this is a cheap copy for unit
            // variants (operators have no heap data) and a heap allocation only
            // for string-carrying variants like Identifier — which never appear
            // in the infix position anyway.
            let infix_token = self.current_token.clone();
            let infix_pos   = self.current_pos;

            // Parse the right-hand side at the right binding power of this op.
            // Using rbp instead of lbp here is what makes right-associative
            // operators work: rbp < lbp means the recursive call can "steal"
            // another operator at the same level.
            let right_bp = rbp(&infix_token);
            let rhs = self.parse_expr(right_bp);

            // Combine lhs and rhs using the saved infix token.
            lhs = self.parse_infix(lhs, rhs, infix_token, infix_pos);
        }

        lhs
    }

    // -------------------------------------------------------------------------
    // Prefix productions
    // -------------------------------------------------------------------------

    /// Called at the start of each expression.
    ///
    /// Advances to the next token, then dispatches on its kind to build the
    /// appropriate AST node.  Each branch handles one "prefix" form:
    ///   • Literal values (number, text, identifier)
    ///   • Unary operators (prefix `+`, prefix `-`)
    ///   • Grouping (`(`, `[`, `{`)
    fn parse_prefix(&mut self) -> Node {
        self.advance();                     // consume the first token of this expression
        let pos = self.current_pos;

        match &self.current_token {

            // -----------------------------------------------------------------
            // Literals
            // -----------------------------------------------------------------

            // A numeric literal becomes a `Node::Numb` leaf.
            Token::Number(val) => {
                let val = *val;             // copy the f64 out before the borrow ends
                Node::Numb(Leaf { pos, val })
            }

            // A string literal becomes a `Node::Text` leaf.
            Token::Text(val) => {
                let val = val.to_string();
                Node::Text(Leaf { pos, val })
            }

            // An identifier becomes a `Node::Identifier` leaf.
            Token::Identifier(name) => {
                let val = name.to_string();
                Node::Identifier(Leaf { pos, val })
            }

            // -----------------------------------------------------------------
            // Grouped expressions
            // -----------------------------------------------------------------

            // A `(` starts a parenthesised sub-expression.
            // Empty parentheses `()` represent Nothing.
            // Otherwise, parse the inner expression at precedence 0 (so ALL
            // operators inside bind), then expect a closing `)`.
            Token::LeftRoundBracket => {
                self.skip_comments();
                if matches!(self.peek_token, Token::RightRoundBracket) {
                    self.advance();
                    Node::Nothing(Leaf { pos, val: () })
                } else {
                    let inner = self.parse_expr(0);
                    // Consume the matching `)`.
                    self.advance();
                    if matches!(self.current_token, Token::RightRoundBracket) {
                        inner
                    } else {
                        Node::Unclosed(Leaf {pos:self.current_pos, val:')'})
                    }
                }
            }

            // A `[` starts a list literal.
            // An empty bracket pair `[]` is a list containing Nothing (which yields an empty list).
            // Parse the inner expression at precedence 0, then expect `]`.
            Token::LeftSquareBracket => {
                self.skip_comments();
                let inner = if matches!(self.peek_token, Token::RightSquareBracket) {
                    let void_pos = self.peek_pos;
                    self.advance();
                    Node::Nothing(Leaf { pos: void_pos, val: () })
                } else {
                    let inner = self.parse_expr(0);
                    self.advance();
                    if matches!(self.current_token, Token::RightSquareBracket) {
                        inner
                    } else {
                        Node::Unclosed(Leaf {pos:self.current_pos, val:']'})
                    }
                };
                Node::List(UnaryOp { pos, opd: Box::new(inner) })
            }

            // A `{` starts a namespace literal.
            // Parse the inner expression at precedence 0, then expect `}`.
            Token::LeftCurlyBracket => {
                self.skip_comments();
                let inner = if matches!(self.peek_token, Token::RightCurlyBracket) {
                    let void_pos = self.peek_pos;
                    self.advance();
                    Node::Nothing(Leaf { pos: void_pos, val: () })
                } else {
                    let inner = self.parse_expr(0);
                    self.advance();
                    if matches!(self.current_token, Token::RightCurlyBracket) {
                        inner
                    } else {
                        Node::Unclosed(Leaf {pos:self.current_pos, val:'}'})
                    }
                };
                Node::Namespace(UnaryOp { pos, opd: Box::new(inner) })
            }

            // -----------------------------------------------------------------
            // Unary (prefix) operators
            // -----------------------------------------------------------------

            // Prefix `+` — unary affirmation (no real effect on numbers, but
            // it is syntactically valid: `+x`).
            // We parse the operand at a HIGH binding power (50) so that the
            // unary op grabs its argument tightly.
            Token::PlusOp => {
                let opd = Box::new(self.parse_expr(50));
                Node::AffirmOp(UnaryOp { pos, opd })
            }

            // Prefix `-` — unary negation.
            Token::MinusOp => {
                let opd = Box::new(self.parse_expr(50));
                Node::NegateOp(UnaryOp { pos, opd })
            }

            // Prefix `!` — unary lofical negation.
            Token::NotOp => {
                let opd = Box::new(self.parse_expr(50));
                Node::LogicalNegationOp(UnaryOp { pos, opd })
            }

            // Prefix `$` — unary lifting operation.
            Token::LiftingOp => {
                let opd = Box::new(self.parse_expr(50));
                Node::LiftingOp(UnaryOp { pos, opd })
            }

            // -----------------------------------------------------------------
            // Anything else is unexpected at the start of an expression.
            // -----------------------------------------------------------------
            _ => { 
                let val = format!("{:?}", self.current_token);
                Node::UnexpectedToken( Leaf {pos, val})
            }
        }
    }

    // -------------------------------------------------------------------------
    // Infix productions
    // -------------------------------------------------------------------------

    /// Combines a parsed `lhs` and `rhs` into the correct binary `Node`.
    ///
    /// # Why take the token as a parameter?
    ///
    /// After `parse_expr(right_bp)` returns, `self.current_token` has been
    /// overwritten many times by the recursive calls to `advance()` inside it.
    /// It no longer holds the infix operator — it holds whatever the deepest
    /// recursion consumed last.  We therefore snapshot the infix token in
    /// `parse_expr` (with `.clone()`) before recursing and pass it in here.
    ///
    /// `pos`   — byte position of the operator in the source.
    /// `token` — the infix operator token (already consumed, passed by value).
    fn parse_infix(&mut self, lhs: Node, rhs: Node, token: Token<'a>, pos: usize) -> Node {

        // Helper closure — packages lhs + rhs into a `BinaryOp`.
        let bin = |lho: Node, rho: Node| BinaryOp {
            pos,
            lho: Box::new(lho),
            rho: Box::new(rho),
        };

        match token {

            // Arithmetic
            Token::PlusOp       => Node::SumOp(bin(lhs, rhs)),
            Token::MinusOp      => Node::SubtractionOp(bin(lhs, rhs)),
            Token::ProductOp    => Node::ProductOp(bin(lhs, rhs)),
            Token::DivisionOp   => Node::DivisionOp(bin(lhs, rhs)),
            Token::ModuloOp     => Node::ModuloOp(bin(lhs, rhs)),
            Token::PowerOp      => Node::PowerOp(bin(lhs, rhs)),

            // Comparison
            Token::EqualToOp              => Node::EqualToOp(bin(lhs, rhs)),
            Token::NotEqualToOp           => Node::NotEqualToOp(bin(lhs, rhs)),
            Token::LessThanOp             => Node::LessThanOp(bin(lhs, rhs)),
            Token::LessThanOrEqualToOp    => Node::LessThanOrEqualToOp(bin(lhs, rhs)),
            Token::GreaterThanOp          => Node::GreaterThanOp(bin(lhs, rhs)),
            Token::GreaterThanOrEqualToOp => Node::GreaterThanOrEqualToOp(bin(lhs, rhs)),

            // Logic
            Token::AndOp => Node::AndOp(bin(lhs, rhs)),
            Token::OrOp  => Node::OrOp(bin(lhs, rhs)),

            // Control flow / short-circuit
            Token::ConditionalOp  => Node::ConditionalOp(bin(lhs, rhs)),
            Token::AlternativeOp  => Node::AlternativeOp(bin(lhs, rhs)),

            // Function-related
            Token::FunctionDefinitionOp => Node::FunctionDefinitionOp(bin(lhs, rhs)),
            Token::MappingOp            => Node::MappingOp(bin(lhs, rhs)),

            // Composition
            Token::CompositionOp        => Node::CompositionOp(bin(lhs, rhs)),
            Token::ReverseCompositionOp => Node::ReverseCompositionOp(bin(lhs, rhs)),

            // Namespace access: `foo.bar`
            Token::SubcontextingOp => Node::SubcontextingOp(bin(lhs, rhs)),

            // Assignment and labelling
            Token::AssignmentOp => Node::AssignmentOp(bin(lhs, rhs)),
            Token::LabellingOp  => Node::LabellingOp(bin(lhs, rhs)),

            // Pairing (tuple / comma list): `a, b, c`
            Token::PairingOp => Node::PairingOp(bin(lhs, rhs)),

            // Anything that reaches here is a parser bug.
            _ => {
                let val = format!("{:?}", token);
                Node::UnexpectedToken( Leaf {pos, val})
            }
        }
    }

    // -------------------------------------------------------------------------
    // Public entry point
    // -------------------------------------------------------------------------

    /// Parse the entire input as a single expression and return the root node.
    ///
    /// ```
    /// use swan_core::lexer::Lexer;
    /// use swan_core::parser::Parser;
    ///
    /// let lexer = Lexer::new("1 + 2 * 3");
    /// let mut parser = Parser::new(lexer);
    /// let ast = parser.parse();   // PlusOp(1, ProductOp(2, 3))
    /// ```
    pub fn parse(&mut self) -> Node {
        // Start with the lowest possible binding power so nothing is "held
        // back" — the entire input becomes one big expression.
        self.parse_expr(0)
    }
}





// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{BinaryOp, Leaf, Node, UnaryOp};
    use crate::lexer::Lexer;

    /// Convenience: parse `src` and return the root node.
    fn parse(src: &str) -> Node {
        Parser::new(Lexer::new(src)).parse()
    }

    // -------------------------------------------------------------------------
    // Literals
    // -------------------------------------------------------------------------

    #[test]
    fn test_number_literal() {
        assert_eq!(parse("42"), Node::Numb(Leaf { pos: 0, val: 42.0 }));
    }

    #[test]
    fn test_empty_expression() {
        assert_eq!(parse(""), Node::Nothing(Leaf { pos: 0, val: () }));
    }

    #[test]
    fn test_whitespace_empty_expression() {
        assert_eq!(parse("   "), Node::Nothing(Leaf { pos: 3, val: () }));
    }

    #[test]
    fn test_comment_only_expression() {
        assert_eq!(parse("# comment\n"), Node::Nothing(Leaf { pos: 10, val: () }));
    }

    #[test]
    fn test_nothing_literal() {
        assert_eq!(parse("()"), Node::Nothing(Leaf { pos: 0, val: () }));
    }

    #[test]
    fn test_nothing_literal_with_whitespace_and_comment() {
        assert_eq!(parse("(\n  # comment\n)"), Node::Nothing(Leaf { pos: 0, val: () }));
    }

    #[test]
    fn test_nested_nothing_literal() {
        assert_eq!(parse("(())"), Node::Nothing(Leaf { pos: 1, val: () }));
    }

    #[test]
    fn test_empty_list_literal() {
        assert_eq!(
            parse("[]"),
            Node::List(UnaryOp {
                pos: 0,
                opd: Box::new(Node::Nothing(Leaf { pos: 1, val: () })),
            })
        );
    }

    #[test]
    fn test_empty_namespace_literal() {
        assert_eq!(
            parse("{}"),
            Node::Namespace(UnaryOp {
                pos: 0,
                opd: Box::new(Node::Nothing(Leaf { pos: 1, val: () })),
            })
        );
    }

    #[test]
    fn test_application_with_nothing_argument() {
        assert_eq!(
            parse("f ()"),
            Node::ApplicationOp(BinaryOp {
                pos: 2,
                lho: Box::new(Node::Identifier(Leaf { pos: 0, val: "f".to_string() })),
                rho: Box::new(Node::Nothing(Leaf { pos: 2, val: () })),
            })
        );
    }

    #[test]
    fn test_text_literal() {
        assert_eq!(
            parse("\"hello\""),
            Node::Text(Leaf { pos: 0, val: "hello".to_string() })
        );
    }

    #[test]
    fn test_identifier_literal() {
        assert_eq!(
            parse("foo"),
            Node::Identifier(Leaf { pos: 0, val: "foo".to_string() })
        );
    }


    // -------------------------------------------------------------------------
    // Arithmetic — precedence and associativity
    // -------------------------------------------------------------------------

    #[test]
    fn test_addition() {
        // `1 + 2` → PlusOp(1, 2)
        assert_eq!(
            parse("1 + 2"),
            Node::SumOp(BinaryOp {
                pos: 2,
                lho: Box::new(Node::Numb(Leaf { pos: 0, val: 1.0 })),
                rho: Box::new(Node::Numb(Leaf { pos: 4, val: 2.0 })),
            })
        );
    }

    #[test]
    fn test_precedence_mul_over_add() {
        // `1 + 2 * 3` must parse as `1 + (2 * 3)` because `*` binds tighter.
        // The expected tree: PlusOp(1, ProductOp(2, 3))
        let expected = Node::SumOp(BinaryOp {
            pos: 2,
            lho: Box::new(Node::Numb(Leaf { pos: 0, val: 1.0 })),
            rho: Box::new(Node::ProductOp(BinaryOp {
                pos: 6,
                lho: Box::new(Node::Numb(Leaf { pos: 4, val: 2.0 })),
                rho: Box::new(Node::Numb(Leaf { pos: 8, val: 3.0 })),
            })),
        });
        assert_eq!(parse("1 + 2 * 3"), expected);
    }

    #[test]
    fn test_left_associativity_of_addition() {
        // `1 + 2 + 3` must parse as `(1 + 2) + 3` (left-associative).
        // The expected tree: PlusOp(PlusOp(1, 2), 3)
        let expected = Node::SumOp(BinaryOp {
            pos: 6,
            lho: Box::new(Node::SumOp(BinaryOp {
                pos: 2,
                lho: Box::new(Node::Numb(Leaf { pos: 0, val: 1.0 })),
                rho: Box::new(Node::Numb(Leaf { pos: 4, val: 2.0 })),
            })),
            rho: Box::new(Node::Numb(Leaf { pos: 8, val: 3.0 })),
        });
        assert_eq!(parse("1 + 2 + 3"), expected);
    }

    #[test]
    fn test_right_associativity_of_power() {
        // `2 ^ 3 ^ 4` must parse as `2 ^ (3 ^ 4)` (right-associative).
        let expected = Node::PowerOp(BinaryOp {
            pos: 2,
            lho: Box::new(Node::Numb(Leaf { pos: 0, val: 2.0 })),
            rho: Box::new(Node::PowerOp(BinaryOp {
                pos: 6,
                lho: Box::new(Node::Numb(Leaf { pos: 4, val: 3.0 })),
                rho: Box::new(Node::Numb(Leaf { pos: 8, val: 4.0 })),
            })),
        });
        assert_eq!(parse("2 ^ 3 ^ 4"), expected);
    }


    #[test]
    fn test_unary_minus() {
        // `-3` → NegateOp(3)
        assert_eq!(
            parse("-3"),
            Node::NegateOp(UnaryOp {
                pos: 0,
                opd: Box::new(Node::Numb(Leaf { pos: 1, val: 3.0 })),
            })
        );
    }

    #[test]
    fn test_parentheses_grouping() {
        // `(1 + 2)` → PlusOp(1, 2)  — parentheses are transparent in the AST.
        assert_eq!(
            parse("(1 + 2)"),
            Node::SumOp(BinaryOp {
                pos: 3,
                lho: Box::new(Node::Numb(Leaf { pos: 1, val: 1.0 })),
                rho: Box::new(Node::Numb(Leaf { pos: 5, val: 2.0 })),
            })
        );
    }

    #[test]
    fn test_list_literal() {
        // `[42]` → List(42)
        assert_eq!(
            parse("[42]"),
            Node::List(UnaryOp {
                pos: 0,
                opd: Box::new(Node::Numb(Leaf { pos: 1, val: 42.0 })),
            })
        );
    }

    #[test]
    fn test_namespace_literal() {
        // `{42}` → Namespace(42)
        assert_eq!(
            parse("{42}"),
            Node::Namespace(UnaryOp {
                pos: 0,
                opd: Box::new(Node::Numb(Leaf { pos: 1, val: 42.0 })),
            })
        );
    }


    // -------------------------------------------------------------------------
    // Application operator (empty / juxtaposition)
    // -------------------------------------------------------------------------

    #[test]
    fn test_application_simple() {
        // `1 2` → ApplicationOp(1, 2)
        // pos recorded is the position of the rhs token (2), which is at byte 2.
        assert_eq!(
            parse("1 2"),
            Node::ApplicationOp(BinaryOp {
                pos: 2,
                lho: Box::new(Node::Numb(Leaf { pos: 0, val: 1.0 })),
                rho: Box::new(Node::Numb(Leaf { pos: 2, val: 2.0 })),
            })
        );
    }

    #[test]
    fn test_application_left_associative() {
        // `1 2 3` must parse as `(1 2) 3` (left-associative).
        // Expected tree: ApplicationOp(ApplicationOp(1, 2), 3)
        let expected = Node::ApplicationOp(BinaryOp {
            pos: 4,
            lho: Box::new(Node::ApplicationOp(BinaryOp {
                pos: 2,
                lho: Box::new(Node::Numb(Leaf { pos: 0, val: 1.0 })),
                rho: Box::new(Node::Numb(Leaf { pos: 2, val: 2.0 })),
            })),
            rho: Box::new(Node::Numb(Leaf { pos: 4, val: 3.0 })),
        });
        assert_eq!(parse("1 2 3"), expected);
    }

    #[test]
    fn test_application_binds_tighter_than_arithmetic() {
        // `1 2 + 3` must parse as `(1 2) + 3`
        // because application (bp=28) > addition (bp=25).
        let expected = Node::SumOp(BinaryOp {
            pos: 4,
            lho: Box::new(Node::ApplicationOp(BinaryOp {
                pos: 2,
                lho: Box::new(Node::Numb(Leaf { pos: 0, val: 1.0 })),
                rho: Box::new(Node::Numb(Leaf { pos: 2, val: 2.0 })),
            })),
            rho: Box::new(Node::Numb(Leaf { pos: 6, val: 3.0 })),
        });
        assert_eq!(parse("1 2 + 3"), expected);
    }

    #[test]
    fn test_application_parenthesised_argument() {
        // `1(2)` — the rhs is a parenthesised sub-expression.
        // parse("1(2)") → ApplicationOp(1, 2)
        // The inner `(2)` just passes through the grouping rule.
        assert_eq!(
            parse("1(2)"),
            Node::ApplicationOp(BinaryOp {
                pos: 1,
                lho: Box::new(Node::Numb(Leaf { pos: 0, val: 1.0 })),
                rho: Box::new(Node::Numb(Leaf { pos: 2, val: 2.0 })),
            })
        );
    }
}

