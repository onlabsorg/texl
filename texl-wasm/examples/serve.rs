//! Builds the web package into `texl-wasm/pkg` and serves the browser test page,
//! so the rendered sample document and the test results can be inspected.
//!
//!     cargo run -p texl-wasm --example serve [ADDRESS]     (default 127.0.0.1:8080)

#[path = "../tests/support/mod.rs"]
mod support;

fn main() {
    let addr = std::env::args().nth(1).unwrap_or_else(|| "127.0.0.1:8080".into());
    let pkg_dir = support::crate_dir().join("pkg");

    eprintln!("building {} ...", pkg_dir.display());
    if let Err(err) = support::build_pkg(&pkg_dir, false, None) {
        eprintln!("{err}");
        std::process::exit(1);
    }

    let origin = support::serve(&addr, pkg_dir).unwrap_or_else(|err| {
        eprintln!("{addr}: {err}");
        std::process::exit(1);
    });
    eprintln!("open {origin}{}   (Ctrl+C to stop)", support::PAGE);
    loop {
        std::thread::park();
    }
}
