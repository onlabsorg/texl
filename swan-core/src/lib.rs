//! Swan is a small, dynamically typed, expression-oriented language designed to
//! be embedded in template engines and other host applications. This crate is
//! its reference implementation: a lexer, a parser, an evaluator, the runtime
//! values and the built-in library. It has no dependencies.
//!
//! The language itself is described in the
//! [Swan language manual](https://onlabsorg.github.io/texl/book/swan/index.html).
//! This documentation covers the Rust API.
//!
//! # Quick start
//!
//! Source text goes through three stages: the [`Lexer`](lexer::Lexer) turns it
//! into tokens, the [`Parser`](parser::Parser) turns the tokens into an
//! [`ast::Node`], and [`evaluator::eval`] turns the node into a
//! [`Value`](value::Value), using a [`Scope`](value::Scope) to look up names.
//! [`builtins::root_scope`] creates a scope that already contains the built-in
//! library.
//!
//! ```
//! use swan_core::builtins::root_scope;
//! use swan_core::evaluator::eval;
//! use swan_core::lexer::Lexer;
//! use swan_core::parser::Parser;
//! use swan_core::value::Value;
//!
//! let scope = root_scope();
//! let ast = Parser::new(Lexer::new("double = x -> x * 2, double 21")).parse();
//! assert_eq!(eval(&ast, &scope), Value::Numb(42.0));
//!
//! // Assignments are stored in the scope, so a later evaluation can use them.
//! let ast = Parser::new(Lexer::new("double 5")).parse();
//! assert_eq!(eval(&ast, &scope), Value::Numb(10.0));
//! ```
//!
//! # Design rules
//!
//! The whole crate follows a few rules. Knowing them makes the rest of the API
//! easier to read.
//!
//! - **Evaluation never fails.** [`evaluator::eval`] never panics and never
//!   returns an error. Syntax errors, type mismatches and invalid operations
//!   all produce a [`Value::Undefined`](value::Value::Undefined) value, which
//!   flows through the rest of the expression like any other value. Built-in
//!   functions follow the same rule.
//! - **Every value is a tuple.** `Nothing` is the empty tuple, a single value
//!   is a one-item tuple, and [`Value::Tuple`](value::Value::Tuple) holds two
//!   or more items. Tuples are always flat and never contain `Nothing`.
//!   Operators apply item by item to tuples.
//! - **Single-threaded and shared.** Values use `Rc`, so cloning is cheap and
//!   nothing is `Send`. [`Scope`](value::Scope), [`Promise`](value::Promise)
//!   and [`BuiltinFn`](value::BuiltinFn) are handles: clones share the same
//!   state.
//!
//! # Extending the language
//!
//! A host adds its own functions by binding [`BuiltinFn`](value::BuiltinFn)
//! values in a scope:
//!
//! ```
//! use swan_core::builtins::root_scope;
//! use swan_core::evaluator::eval;
//! use swan_core::lexer::Lexer;
//! use swan_core::parser::Parser;
//! use swan_core::value::{BuiltinFn, Value};
//!
//! let scope = root_scope();
//! scope.set("shout", Value::Builtin(BuiltinFn::new("shout", |arg| match arg {
//!     Value::Text(t) => Value::Text(t.to_uppercase().into()),
//!     other => Value::undef_unary("shout", other, 0),
//! })));
//!
//! let ast = Parser::new(Lexer::new(r#"shout "hello""#)).parse();
//! assert_eq!(eval(&ast, &scope), Value::Text("HELLO".into()));
//! ```

#![warn(missing_docs)]

pub mod token;
pub mod lexer;

pub mod ast;
pub mod parser;

pub mod value;
pub mod evaluator;

pub mod builtins;
