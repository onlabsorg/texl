//! Runs the browser test page (`tests/web/index.html`) in headless Chrome.
//!
//! The test builds the web package with wasm-pack, serves the page, and reads the
//! page's DOM once its scripts are done: the page sets `data-status` on its body to
//! `pass` or `fail`. It is skipped, with a message, when wasm-pack or Chrome is missing.
//! Set `TEXL_WASM_BROWSER` to the path of a Chrome or Chromium binary to pick one.

mod support;

use std::path::PathBuf;
use std::process::Command;

fn find_browser() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("TEXL_WASM_BROWSER") {
        return Some(PathBuf::from(path));
    }
    ["google-chrome", "google-chrome-stable", "chromium", "chromium-browser", "chrome"]
        .iter()
        .find_map(|name| support::find_in_path(name))
}

#[test]
fn test_web_page_passes_in_headless_chrome() {
    let Some(browser) = find_browser() else {
        eprintln!("skipped: Chrome or Chromium not found (set TEXL_WASM_BROWSER)");
        return;
    };
    if support::find_in_path("wasm-pack").is_none() {
        eprintln!("skipped: wasm-pack not found");
        return;
    }

    // A separate target folder avoids contention with the cargo process running this test
    let work_dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("texl-wasm-browser");
    let pkg_dir = work_dir.join("pkg");
    support::build_pkg(&pkg_dir, true, Some(&work_dir.join("target"))).unwrap();

    let origin = support::serve("127.0.0.1:0", pkg_dir).unwrap();
    let profile_dir = work_dir.join("chrome-profile");
    let out = Command::new(&browser)
        .args(["--headless=new", "--disable-gpu", "--no-first-run", "--no-default-browser-check"])
        .arg(format!("--user-data-dir={}", profile_dir.display()))
        // Virtual time runs until the page is idle, so the DOM is dumped after the checks end
        .arg("--virtual-time-budget=30000")
        .arg("--dump-dom")
        .arg(format!("{origin}{}", support::PAGE))
        .output()
        .unwrap();
    let dom = String::from_utf8_lossy(&out.stdout);

    assert!(
        dom.contains(r#"data-status="pass""#),
        "the test page did not pass (open it with `cargo run -p texl-wasm --example serve`)\n\
         --- DOM ---\n{dom}\n--- stderr ---\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
