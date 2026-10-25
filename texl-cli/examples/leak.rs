//! Measures the memory a rendering leaves behind after the document is dropped.
//!
//! ```text
//! cargo run --release -p texl-cli --example leak -- [FILE] [N]
//! ```
//!
//! Renders FILE (default: the CLI sample) N times (default: 1000) with one
//! engine, dropping each document, and prints the live scopes
//! ([`Scope::live_count`]) and live heap bytes (a counting allocator) per
//! rendering.

#[path = "../src/loader.rs"]
mod loader;

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicIsize, Ordering};
use swan_core::value::{Scope, Value};
use texl_core::helpers::spawner_fn;
use texl_core::{Engine, loader_fn};
use tokio::task::LocalSet;
use url::Url;

/// The system allocator, counting the bytes currently allocated.
struct Counting;

static LIVE_BYTES: AtomicIsize = AtomicIsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        LIVE_BYTES.fetch_add(layout.size() as isize, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE_BYTES.fetch_sub(layout.size() as isize, Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        LIVE_BYTES.fetch_add(new_size as isize - layout.size() as isize, Ordering::Relaxed);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn live_bytes() -> isize {
    LIVE_BYTES.load(Ordering::Relaxed)
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let mut args = std::env::args().skip(1);
    let file = args
        .next()
        .unwrap_or_else(|| concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/sample.texl").into());
    let n: usize = args.next().map_or(1000, |s| s.parse().expect("N must be a number"));
    let path = std::fs::canonicalize(&file).expect("cannot read FILE");
    let url = Url::from_file_path(&path).unwrap().to_string();

    LocalSet::new().run_until(measure(&url, n)).await;
}

async fn measure(url: &str, n: usize) {
    let engine = Engine::new(Some(spawner_fn(|task| {
        tokio::task::spawn_local(task);
    })));
    engine.add_loader("file", loader_fn(loader::load_file));

    // A first rendering builds the thread's built-ins scope, which is never freed
    render_once(&engine, url).await;
    let (scopes0, bytes0) = (Scope::live_count(), live_bytes());

    for _ in 0..n {
        render_once(&engine, url).await;
    }

    let scopes = Scope::live_count() - scopes0;
    let bytes = live_bytes() - bytes0;
    println!("{url}");
    println!("renderings:            {n}");
    println!("leaked scopes:         {scopes} ({:.1} per rendering)", scopes as f64 / n as f64);
    println!("leaked heap bytes:     {bytes} ({:.1} KB per rendering)", bytes as f64 / n as f64 / 1024.0);
}

/// Loads the document, checks that it rendered, and drops it.
async fn render_once(engine: &Engine, url: &str) {
    let doc = engine.load(url).await;
    let Value::Namespace(ns) = &doc else { panic!("{url}: could not be loaded") };
    assert!(matches!(ns.get("rendering"), Value::Text(_)), "{url}: rendering failed");
    // Let the spawned import tasks finish and release what they hold
    tokio::task::yield_now().await;
}
