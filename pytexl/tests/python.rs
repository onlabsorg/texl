//! Runs the Python unit tests of the `pytexl` package (`tests/test_*.py`).
//!
//! The test builds the native module with cargo, stages it next to the Python
//! sources of the package, and runs `python -m unittest discover` on the `tests`
//! folder with that package on the `PYTHONPATH`. It is skipped, with a message,
//! when no Python 3.10 or later is found. Set `PYTEXL_PYTHON` to the path of an
//! interpreter to pick one.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The pytexl crate folder.
fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Finds a Python interpreter of version 3.10 or later.
fn find_python() -> Option<PathBuf> {
    let candidates: Vec<PathBuf> = match std::env::var_os("PYTEXL_PYTHON") {
        Some(path) => vec![PathBuf::from(path)],
        None => vec!["python3".into(), "python".into()],
    };
    candidates.into_iter().find(|python| {
        Command::new(python)
            .args(["-c", "import sys; sys.exit(sys.version_info < (3, 10))"])
            .status()
            .is_ok_and(|status| status.success())
    })
}

/// Builds the native module for `python` and returns the path of the library.
fn build_module(python: &Path, target_dir: &Path) -> Result<PathBuf, String> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let out = Command::new(cargo)
        .args(["build", "--lib", "--manifest-path"])
        .arg(crate_dir().join("Cargo.toml"))
        .arg("--target-dir")
        .arg(target_dir)
        .env("PYO3_PYTHON", python)
        // An extension module does not link libpython: it uses the interpreter that loads it
        .env("PYO3_BUILD_EXTENSION_MODULE", "1")
        .output()
        .map_err(|e| format!("cargo: {e}"))?;
    if !out.status.success() {
        return Err(format!("cargo build failed:\n{}", String::from_utf8_lossy(&out.stderr)));
    }
    let name = if cfg!(windows) {
        "_pytexl.dll"
    } else if cfg!(target_os = "macos") {
        "lib_pytexl.dylib"
    } else {
        "lib_pytexl.so"
    };
    Ok(target_dir.join("debug").join(name))
}

/// Copies the Python sources of the package and the native module into `site/pytexl`.
fn stage_package(library: &Path, site: &Path) -> std::io::Result<()> {
    let package = site.join("pytexl");
    if package.exists() {
        fs::remove_dir_all(&package)?;
    }
    fs::create_dir_all(&package)?;
    // Only the sources: `maturin develop` leaves a build of the module in the folder
    for entry in fs::read_dir(crate_dir().join("python").join("pytexl"))? {
        let path = entry?.path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if path.is_file() && (name.ends_with(".py") || name.ends_with(".pyi") || name == "py.typed") {
            fs::copy(&path, package.join(name))?;
        }
    }
    let module = if cfg!(windows) { "_pytexl.pyd" } else { "_pytexl.so" };
    fs::copy(library, package.join(module))?;
    Ok(())
}

#[test]
fn test_python_package_passes_its_unit_tests() {
    let Some(python) = find_python() else {
        eprintln!("skipped: Python 3.10 or later not found (set PYTEXL_PYTHON)");
        return;
    };

    // A separate target folder avoids contention with the cargo process running this test
    let work_dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("pytexl-python");
    let library = build_module(&python, &work_dir.join("target")).unwrap();
    let site = work_dir.join("site");
    stage_package(&library, &site).unwrap();

    let out = Command::new(&python)
        .args(["-m", "unittest", "discover", "-v", "-s", "tests"])
        .current_dir(crate_dir())
        .env("PYTHONPATH", &site)
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "the Python tests failed\n--- stdout ---\n{}\n--- stderr ---\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}
