//! The Swan lexer: turns source text into [`Token`]s.


use std::iter::Peekable;
use std::str::Chars;
use crate::token::Token;


/// A zero-copy lexer for Swan source text.
///
/// `Lexer` converts a source `&str` into a sequence of [`Token`]s.  Each
/// token that carries a string payload (`Identifier`, `Text`, `Comment`,
/// `Undefined`) borrows a slice directly from the original source, so no heap
/// allocations are needed during tokenisation.
///
/// # Usage
///
/// Call [`next_token`](Lexer::next_token) in a loop to pull tokens one at a
/// time, or use the [`Iterator`] implementation to drive the lexer with
/// standard iterator adapters.  The iterator stops (returns `None`) at
/// [`Token::EndOfExpression`]; `next_token` returns that sentinel instead.
///
/// ```
/// use swan_core::lexer::Lexer;
/// use swan_core::token::Token;
///
/// let tokens: Vec<_> = Lexer::new("x = 42").collect();
/// assert_eq!(tokens[0], (0, Token::Identifier("x")));
/// assert_eq!(tokens[1], (2, Token::AssignmentOp));
/// assert_eq!(tokens[2], (4, Token::Number(42.0)));
/// ```
///
/// # Position tracking
///
/// Every call to `next_token` returns a `(usize, Token)` pair where the
/// `usize` is the **byte offset** of the token's first character inside the
/// source string.  Positions are byte-based (not char-based) and are safe to
/// use as indices into the original `&str`.

pub struct Lexer<'a> {
    /// The full source string — never modified; sliced into by token variants.
    source: &'a str,
    /// Character iterator with one-character lookahead.
    chars: Peekable<Chars<'a>>,
    /// Byte offset of the *next* character to be consumed by `next_char`.
    /// Starts at 0.  Incremented by `ch.len_utf8()` with every consumed char,
    /// keeping it valid as a UTF-8 byte index into `source`.
    position: usize,
}

impl<'a> Lexer<'a> {

    /// Creates a new `Lexer` positioned at the start of `source`.
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            chars: source.chars().peekable(),
            position: 0,
        }
    }

    // -------------------------------------------------------------------------
    // Low-level character helpers
    // -------------------------------------------------------------------------

    /// Consumes and returns the next character, advancing `position` by the
    /// character's UTF-8 byte length.  Returns `None` at end-of-input.
    fn next_char(&mut self) -> Option<char> {
        let ch = self.chars.next()?;
        self.position += ch.len_utf8();
        Some(ch)
    }

    /// Returns the next character without consuming it.
    fn peek_char(&mut self) -> Option<char> {
        self.chars.peek().copied()
    }

    /// Returns the second upcoming character without consuming any characters.
    fn peek_second_char(&self) -> Option<char> {
        let mut it = self.chars.clone();
        it.next(); // Skip the first peeked character
        it.next()   // Return the second character
    }

    /// Returns `true` if the next (unconsumed) character equals `ch`.
    fn next_char_is(&mut self, ch: char) -> bool {
        self.peek_char() == Some(ch)
    }

    /// Skips all leading whitespace characters (space, tab, newline, …).
    fn skip_whitespaces(&mut self) {
        while let Some(ch) = self.peek_char() {
            if ch.is_whitespace() {
                self.next_char();
            } else {
                break;
            }
        }
    }

    // -------------------------------------------------------------------------
    // Token-specific scanners
    // -------------------------------------------------------------------------

    /// Scans the rest of an identifier, given that its first character has
    /// already been consumed and `start_pos` is its byte offset.
    ///
    /// Identifier continuation characters are letters, digits, and `_`.
    /// The first character (already consumed by the caller) is either a letter
    /// or `_`, so this method only extends the span rightward.
    ///
    /// Returns a [`Token::Identifier`] borrowing the scanned slice from
    /// `source`.
    fn next_identifier(&mut self, start_pos: usize) -> Token<'a> {
        while let Some(ch) = self.peek_char() {
            if ch == '_' || ch.is_alphanumeric() {
                self.next_char();
            } else {
                break;
            }
        }
        let literal = &self.source[start_pos..self.position];
        Token::Identifier(literal)
    }

    /// Consumes a run of ASCII decimal digits without returning anything.
    /// Used as a building block inside [`next_numeric_literal`](Self::next_numeric_literal).
    fn consume_digits(&mut self) {
        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_digit() {
                self.next_char();
            } else {
                break;
            }
        }
    }

    /// Scans a numeric literal, given that its first digit has already been
    /// consumed and `start_pos` is its byte offset.
    ///
    /// Accepts the following forms (matching the JS reference grammar):
    ///
    /// ```text
    /// integer:    123
    /// decimal:    12.3
    /// scientific: 12.3e2  12.3E+2  12.3e-2
    /// ```
    ///
    /// Returns [`Token::Number`] on success, or [`Token::Undefined`] when the
    /// literal is malformed: an exponent marker without digits (`1e+`), or a
    /// number immediately followed by identifier characters (`123abc`).
    ///
    /// A dot that is not followed by a digit is not part of the number, so
    /// `12.` lexes as `Number(12)` followed by `SubcontextingOp`.
    fn next_numeric_literal(&mut self, start_pos: usize) -> Token<'a> {

        // Consume the integer part (the first digit was already consumed by the
        // caller, but consume_digits will handle any remaining leading digits).
        self.consume_digits();

        // Optional fractional part: `.` followed by one or more digits.
        if self.peek_char() == Some('.') && matches!(self.peek_second_char(), Some('0'..='9')) {
            self.next_char(); // consume '.'
            self.consume_digits();
        }

        // Optional exponent part: `e` or `E`, optional sign, one or more digits.
        if matches!(self.peek_char(), Some('e') | Some('E')) {
            self.next_char(); // consume 'e'/'E'
            if matches!(self.peek_char(), Some('+') | Some('-')) {
                self.next_char(); // consume optional sign
            }
            if matches!(self.peek_char(), Some('0'..='9')) {
                self.consume_digits();
            } else {
                // Exponent marker not followed by digits → malformed literal.
                return Token::Undefined(&self.source[start_pos..self.position]);
            }
        }

        // Trailing word-boundary check: numbers cannot be directly adjacent to identifier chars
        if matches!(self.peek_char(), Some('_') | Some('a'..='z' | 'A'..='Z')) {
            while let Some(ch) = self.peek_char() {
                if ch == '_' || ch.is_alphanumeric() {
                    self.next_char();
                } else {
                    break;
                }
            }
            return Token::Undefined(&self.source[start_pos..self.position]);
        }

        // The accumulated substring is a well-formed f64 string; Rust's
        // standard parser handles all three forms above.
        let number_str = &self.source[start_pos..self.position];
        let value: f64 = number_str
            .parse()
            .expect("BUG: lexer produced a number string that f64::parse() rejected");
        Token::Number(value)
    }

    /// Scans a double-quoted string literal, given that the opening `"` has
    /// already been consumed and `start_pos` is the byte offset of the first
    /// content character (i.e. `self.position` right after consuming `"`).
    ///
    /// The returned [`Token::Text`] slice contains only the content between
    /// the quotes; the delimiters are not included.
    ///
    /// If the input ends before a closing `"` is found, the slice extends to
    /// the end of the source rather than returning an error.
    fn next_string_literal(&mut self, start_pos: usize) -> Token<'a> {
        let end_pos: usize = loop {
            let ch = self.peek_char();
            if ch == Some('"') {
                // Found the closing quote.  Record the end position *before*
                // consuming it so the quote is excluded from the slice.
                let end_pos = self.position;
                self.next_char(); // consume closing '"'
                break end_pos;
            } else if ch.is_none() {
                // End of input without a closing quote — emit the content so
                // far rather than erroring (lenient, matching the JS behaviour).
                break self.position;
            } else {
                self.next_char();
            }
        };
        Token::Text(&self.source[start_pos..end_pos])
    }

    /// Scans a `#`-prefixed line comment, given that the `#` has already been
    /// consumed and `start_pos` is the byte offset of the first content
    /// character (i.e. `self.position` right after consuming `#`).
    ///
    /// The comment extends to (but does not include) the next `\n`, or to the
    /// end of the source if there is no newline.  The `\n` itself is consumed
    /// and discarded.
    ///
    /// The returned [`Token::Comment`] slice contains only the text after `#`
    /// and before the terminating newline.
    fn next_comment(&mut self, start_pos: usize) -> Token<'a> {
        let end_pos: usize = loop {
            let ch = self.peek_char();
            if ch == Some('\n') {
                // Newline terminates the comment; exclude it from the slice.
                let end_pos = self.position;
                self.next_char(); // consume '\n'
                break end_pos;
            } else if ch.is_none() {
                // End of input — comment runs to the end of the source.
                break self.position;
            } else {
                self.next_char();
            }
        };
        Token::Comment(&self.source[start_pos..end_pos])
    }

    // -------------------------------------------------------------------------
    // Main entry point
    // -------------------------------------------------------------------------

    /// Returns the next token and its byte position in the source.
    ///
    /// Leading whitespace is skipped before each token.  When the source is
    /// exhausted this method returns `(pos, Token::EndOfExpression)` and
    /// continues to do so on every subsequent call — the position will be
    /// pinned at the end of the source.
    ///
    /// Two-character operators (`==`, `!=`, `<=`, `>=`, `->`, `<<`, `>>`,
    /// `=>`, `~>`) are recognised greedily: the first character is consumed by the
    /// main dispatch, then if the following character completes the operator it
    /// is consumed too before the token is returned.
    pub fn next_token(&mut self) -> (usize, Token<'a>) {
        self.skip_whitespaces();

        // Snapshot the byte offset of the token we are about to produce.
        let ch_pos = self.position;

        // Consume the first character; return EndOfExpression at end-of-input.
        let ch = match self.next_char() {
            Some(ch) => ch,
            None => return (ch_pos, Token::EndOfExpression),
        };

        // --- Identifiers -------------------------------------------------------
        // Start character: letter or `_`.  The rest of the identifier is
        // consumed by next_identifier.
        if ch == '_' || ch.is_alphabetic() {
            (ch_pos, self.next_identifier(ch_pos))
        }

        // --- Numeric literals --------------------------------------------------
        // Start character: ASCII digit.
        else if ch.is_ascii_digit() {
            (ch_pos, self.next_numeric_literal(ch_pos))
        }

        // --- String literals ---------------------------------------------------
        // Opening `"`.  self.position now points past the quote.
        else if ch == '"' {
            (ch_pos, self.next_string_literal(self.position))
        }

        // --- Line comments -----------------------------------------------------
        // Opening `#`.  self.position now points past the `#`.
        else if ch == '#' {
            (ch_pos, self.next_comment(self.position))
        }

        // --- Two-character operators -------------------------------------------
        // Each branch peeks at the next character before committing.

        // `==`
        else if ch == '=' && self.next_char_is('=') {
            self.next_char(); (ch_pos, Token::EqualToOp)
        }
        // `!=`
        else if ch == '!' && self.next_char_is('=') {
            self.next_char(); (ch_pos, Token::NotEqualToOp)
        }
        // `<=`
        else if ch == '<' && self.next_char_is('=') {
            self.next_char(); (ch_pos, Token::LessThanOrEqualToOp)
        }
        // `>=`
        else if ch == '>' && self.next_char_is('=') {
            self.next_char(); (ch_pos, Token::GreaterThanOrEqualToOp)
        }
        // `->`
        else if ch == '-' && self.next_char_is('>') {
            self.next_char(); (ch_pos, Token::FunctionDefinitionOp)
        }
        // `<<`
        else if ch == '<' && self.next_char_is('<') {
            self.next_char(); (ch_pos, Token::CompositionOp)
        }
        // `>>`
        else if ch == '>' && self.next_char_is('>') {
            self.next_char(); (ch_pos, Token::ReverseCompositionOp)
        }
        // `=>`
        else if ch == '=' && self.next_char_is('>') {
            self.next_char(); (ch_pos, Token::MappingOp)
        }
        else if ch == '~' && self.next_char_is('>') {
            self.next_char();
            (ch_pos, Token::ThenOp)
        }

        // --- Single-character tokens -------------------------------------------
        else {
            match ch {
                '(' => (ch_pos, Token::LeftRoundBracket),
                ')' => (ch_pos, Token::RightRoundBracket),

                '[' => (ch_pos, Token::LeftSquareBracket),
                ']' => (ch_pos, Token::RightSquareBracket),

                '{' => (ch_pos, Token::LeftCurlyBracket),
                '}' => (ch_pos, Token::RightCurlyBracket),

                '.' => (ch_pos, Token::SubcontextingOp),

                '^' => (ch_pos, Token::PowerOp),
                '*' => (ch_pos, Token::ProductOp),
                '/' => (ch_pos, Token::DivisionOp),
                '%' => (ch_pos, Token::ModuloOp),
                '+' => (ch_pos, Token::PlusOp),
                '-' => (ch_pos, Token::MinusOp),

                '<' => (ch_pos, Token::LessThanOp),
                '>' => (ch_pos, Token::GreaterThanOp),

                '&' => (ch_pos, Token::AndOp),
                '|' => (ch_pos, Token::OrOp),

                '?' => (ch_pos, Token::ConditionalOp),
                ';' => (ch_pos, Token::AlternativeOp),

                '=' => (ch_pos, Token::AssignmentOp),
                ':' => (ch_pos, Token::LabellingOp),

                ',' => (ch_pos, Token::PairingOp),

                '!' => (ch_pos, Token::NotOp),
                '$' => (ch_pos, Token::LiftingOp),

                // Anything else is returned as an Undefined token containing
                // exactly the one unrecognised character.
                _ => (ch_pos, Token::Undefined(&self.source[ch_pos..self.position])),
            }
        }
    }
}


// -----------------------------------------------------------------------------
// Iterator implementation
// -----------------------------------------------------------------------------

/// Drives the lexer as a standard Rust iterator.
///
/// Each call to `next` delegates to [`next_token`](Lexer::next_token) and maps
/// `Token::EndOfExpression` to `None`, stopping iteration.  All other tokens
/// (including [`Token::Comment`]) are yielded.
///
/// > **Note**: Comment tokens are *not* silently discarded here.  The parser
/// > is responsible for skipping them during expression parsing.
impl<'a> Iterator for Lexer<'a> {
    type Item = (usize, Token<'a>);

    fn next(&mut self) -> Option<Self::Item> {
        let (pos, token) = self.next_token();
        if token == Token::EndOfExpression {
            None
        } else {
            Some((pos, token))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lexer_next_token() {

        macro_rules! assert_token {
            ($input:expr, $token:expr, $pos:expr) => {
                assert_eq!(
                    Lexer::new($input).next_token(),
                    ($pos, $token))
            }
        }

        // Identifier
        assert_token!("Abc1", Token::Identifier("Abc1"), 0);
        assert_token!("abc1", Token::Identifier("abc1"), 0);
        assert_token!("_ab1", Token::Identifier("_ab1"), 0);

        // Numeric Literal
        assert_token!("123", Token::Number(123.0), 0);
        assert_token!("12.3", Token::Number(12.3), 0);
        assert_token!("12.3e2", Token::Number(1230.0), 0);
        assert_token!("12.3E+2", Token::Number(1230.0), 0);
        assert_token!("12.3e-2", Token::Number(0.123), 0);
        assert_token!("12.", Token::Number(12.0), 0);
        assert_token!("12.3e", Token::Undefined("12.3e"), 0);
        assert_token!("12.3e+", Token::Undefined("12.3e+"), 0);
        assert_token!("12.3E-", Token::Undefined("12.3E-"), 0);
        assert_token!("12.3abc", Token::Undefined("12.3abc"), 0);

        // Text Literal
        assert_token!("\"abc\"", Token::Text("abc"), 0);
        assert_token!("\"abc", Token::Text("abc"), 0);

        // Brackets
        assert_token!("(", Token::LeftRoundBracket, 0);
        assert_token!(")", Token::RightRoundBracket, 0);
        assert_token!("[", Token::LeftSquareBracket, 0);
        assert_token!("]", Token::RightSquareBracket, 0);
        assert_token!("{", Token::LeftCurlyBracket, 0);
        assert_token!("}", Token::RightCurlyBracket, 0);

        // Binary operators
        assert_token!(".", Token::SubcontextingOp, 0);

        assert_token!("^", Token::PowerOp, 0);
        assert_token!("*", Token::ProductOp, 0);
        assert_token!("/", Token::DivisionOp, 0);
        assert_token!("%", Token::ModuloOp, 0);
        assert_token!("+", Token::PlusOp, 0);
        assert_token!("-", Token::MinusOp, 0);

        assert_token!("==", Token::EqualToOp, 0);
        assert_token!("!=", Token::NotEqualToOp, 0);
        assert_token!("<", Token::LessThanOp, 0);
        assert_token!("<=", Token::LessThanOrEqualToOp, 0);
        assert_token!(">", Token::GreaterThanOp, 0);
        assert_token!(">=", Token::GreaterThanOrEqualToOp, 0);
        
        assert_token!("&", Token::AndOp, 0);
        assert_token!("|", Token::OrOp, 0);

        assert_token!("?", Token::ConditionalOp, 0);
        assert_token!(";", Token::AlternativeOp, 0);

        assert_token!("->", Token::FunctionDefinitionOp, 0);

        assert_token!("<<", Token::CompositionOp, 0);
        assert_token!(">>", Token::ReverseCompositionOp, 0);

        assert_token!("=>", Token::MappingOp, 0);

        assert_token!("=", Token::AssignmentOp, 0);
        assert_token!(":", Token::LabellingOp, 0);

        assert_token!(",", Token::PairingOp, 0);

        // Special tokens
        assert_token!("@", Token::Undefined("@"), 0);

        assert_token!("# this is a comment \n\n", Token::Comment(" this is a comment "), 0);
        assert_token!("# this is a comment ", Token::Comment(" this is a comment "), 0);

        assert_token!("", Token::EndOfExpression, 0);

        // Spaces skipping
        assert_token!("  *", Token::ProductOp, 2);
    }

    #[test]
    fn test_lexer_iterator() {
        let input = "msg = \"hello\"";
        let lexer = Lexer::new(input);
        
        let tokens: Vec<(usize, Token)> = lexer.collect();
        assert_eq!(
            tokens,
            vec![
                (0, Token::Identifier("msg")),
                (4, Token::AssignmentOp),
                (6, Token::Text("hello")),
            ]
        );
    }
}


