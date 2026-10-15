//! End-to-end tests of the `texl` binary.

use std::path::PathBuf;
use std::process::Command;

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn texl() -> Command {
    Command::new(env!("CARGO_BIN_EXE_texl"))
}

fn expected_sample() -> String {
    std::fs::read_to_string(fixtures().join("sample.expected.txt")).unwrap()
}

#[test]
fn test_renders_sample_to_stdout() {
    let out = texl().arg(fixtures().join("sample.texl")).output().unwrap();

    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(String::from_utf8(out.stdout).unwrap(), expected_sample());
}

#[test]
fn test_resolves_imports_against_the_document_not_the_working_directory() {
    // Run from an unrelated directory, passing a path relative to it
    let out = texl()
        .current_dir(fixtures().join("partials"))
        .arg("../sample.texl")
        .output()
        .unwrap();

    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(String::from_utf8(out.stdout).unwrap(), expected_sample());
}

#[test]
fn test_writes_output_file() {
    let out_path = std::env::temp_dir().join(format!("texl-cli-test-{}.txt", std::process::id()));

    let out = texl()
        .arg(fixtures().join("sample.texl"))
        .arg("-o")
        .arg(&out_path)
        .output()
        .unwrap();
    let written = std::fs::read_to_string(&out_path);
    let _ = std::fs::remove_file(&out_path);

    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(out.stdout.is_empty());
    assert_eq!(written.unwrap(), expected_sample());
}

#[test]
fn test_requires_an_input_file() {
    let out = texl().output().unwrap();

    assert!(!out.status.success());
}

#[test]
fn test_reports_missing_input_file() {
    let out = texl().arg(fixtures().join("does-not-exist.texl")).output().unwrap();

    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("does-not-exist.texl"));
}
