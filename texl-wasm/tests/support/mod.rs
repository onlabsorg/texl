//! Shared by the browser test and the `serve` example: builds the web package and
//! serves the test page.
//!
//! URL paths map to files of the texl-wasm crate (the test page is
//! `/tests/web/index.html`), except `/pkg/...`, which maps to the built package.

#![allow(dead_code)]

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::process::Command;

/// Path of the test page on the server.
pub const PAGE: &str = "/tests/web/index.html";

/// The texl-wasm crate folder.
pub fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Finds `program` in the `PATH`.
pub fn find_in_path(program: &str) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|dir| dir.join(program))
        .find(|path| path.is_file())
}

/// Builds the web package into `out_dir` with wasm-pack. `target_dir`, if given,
/// keeps the build apart from the workspace target folder.
pub fn build_pkg(out_dir: &Path, dev: bool, target_dir: Option<&Path>) -> Result<(), String> {
    let mut cmd = Command::new("wasm-pack");
    cmd.arg("build").arg(crate_dir()).args(["--target", "web"]);
    cmd.arg("--out-dir").arg(out_dir);
    if dev {
        cmd.arg("--dev");
    }
    if let Some(target_dir) = target_dir {
        cmd.env("CARGO_TARGET_DIR", target_dir);
    }
    let out = cmd.output().map_err(|e| format!("wasm-pack: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!("wasm-pack build failed:\n{}", String::from_utf8_lossy(&out.stderr)))
    }
}

/// Serves the crate folder, with `/pkg/` mapped to `pkg_dir`, on a background thread.
/// Returns the server origin, e.g. `http://127.0.0.1:41234`.
pub fn serve(addr: &str, pkg_dir: PathBuf) -> std::io::Result<String> {
    let listener = TcpListener::bind(addr)?;
    let origin = format!("http://{}", listener.local_addr()?);
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let pkg_dir = pkg_dir.clone();
            std::thread::spawn(move || {
                let _ = respond(stream, &pkg_dir);
            });
        }
    });
    Ok(origin)
}

/// Answers one GET request, then closes the connection.
fn respond(mut stream: TcpStream, pkg_dir: &Path) -> std::io::Result<()> {
    let mut request_line = String::new();
    BufReader::new(&stream).read_line(&mut request_line)?;
    let target = request_line.split_whitespace().nth(1).unwrap_or("/");
    let url_path = target.split(['?', '#']).next().unwrap_or("/");

    let (status, content_type, body) = match file_for(url_path, pkg_dir).and_then(|p| {
        std::fs::read(&p).ok().map(|body| (content_type(&p), body))
    }) {
        Some((content_type, body)) => ("200 OK", content_type, body),
        None => ("404 Not Found", "text/plain", b"not found".to_vec()),
    };
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\
         Cache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(&body)
}

/// Maps a URL path to a file. Paths leaving the served folders give `None`.
fn file_for(url_path: &str, pkg_dir: &Path) -> Option<PathBuf> {
    let (root, rel) = match url_path.strip_prefix("/pkg/") {
        Some(rel) => (pkg_dir.to_path_buf(), rel),
        None => (crate_dir(), url_path.trim_start_matches('/')),
    };
    let rel = Path::new(rel);
    if !rel.components().all(|c| matches!(c, Component::Normal(_))) {
        return None;
    }
    let path = root.join(rel);
    Some(if path.is_dir() { path.join("index.html") } else { path })
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("json") => "application/json",
        _ => "text/plain; charset=utf-8",
    }
}
