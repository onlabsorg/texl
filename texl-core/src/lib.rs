
//! The texl template engine.
//!
//! A texl template is text with embedded `<% ... %>` tags. Each tag contains an
//! expression in [Swan](swan_core), and rendering replaces the tag with the
//! value of its expression:
//!
//! ```text
//! Hello <% name %>, you have <% list.size messages %> new messages.
//! ```
//!
//! The [Texl manual](https://onlabsorg.github.io/texl/book/) explains how to
//! write templates. This documentation covers the Rust API.
//!
//! # Overview
//!
//! - [`template::render`] renders a template in a given [`Scope`](swan_core::value::Scope).
//!   It is the low-level entry point.
//! - [`Engine`] adds what a full application needs: globals shared by every
//!   template, loaders that fetch documents by URL, and documents that import
//!   each other with `texl.import`, with relative URIs and an import cache per
//!   rendering.
//! - [`helpers`] connects the engine to the host's async runtime.
//!
//! # Async rendering
//!
//! Rendering is `async` because a tag may return a Swan promise, for example
//! the result of `texl.import`, and rendering waits for it. The futures are
//! single-threaded (`!Send`). With tokio, run them on a `LocalSet` and pass a
//! spawner that calls `tokio::task::spawn_local`:
//!
//! ```ignore
//! use texl_core::{Engine, helpers::spawner_fn};
//!
//! let engine = Engine::new(Some(spawner_fn(|task| { tokio::task::spawn_local(task); })));
//! let doc = tokio::task::LocalSet::new().run_until(engine.render("/main.texl", "<% 6 * 7 %>")).await;
//! ```
//!
//! Without a runtime, pass `None` and drive the futures yourself, calling
//! [`helpers::run_pending`] to run the background tasks.

#![warn(missing_docs)]

pub mod helpers;
pub mod template;
pub mod engine;

pub use engine::{Engine, LoaderFn, loader_fn};
