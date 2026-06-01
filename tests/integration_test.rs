use std::path::Path;
use std::process::Command;

fn mapx_binary() -> &'static str {
    if cfg!(debug_assertions) {
        "target/debug/mapx"
    } else {
        "target/release/mapx"
    }
}

fn fixtures_dir() -> &'static Path {
    Path::new("tests/fixtures")
}

#[test]
fn test_mapx_help() {
    let output = Command::new(mapx_binary())
        .arg("--help")
        .output()
        .expect("failed to run mapx --help");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("mapx"));
    assert!(stdout.contains("--query"));
    assert!(stdout.contains("--root"));
}

#[test]
fn test_mapx_grep_mode_js() {
    let root = fixtures_dir();
    let output = Command::new(mapx_binary())
        .arg("--root")
        .arg(root)
        .arg("--query")
        .arg("buildMiniGraph")
        .arg("--mode")
        .arg("grep")
        .output()
        .expect("failed to run mapx");
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should find buildMiniGraph in sample.js
    assert!(stdout.contains("buildMiniGraph"), "stdout:\n{stdout}");
}

#[test]
fn test_mapx_grep_mode_python() {
    let root = fixtures_dir();
    let output = Command::new(mapx_binary())
        .arg("--root")
        .arg(root)
        .arg("--query")
        .arg("build_mini_graph")
        .arg("--mode")
        .arg("grep")
        .output()
        .expect("failed to run mapx");
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("build_mini_graph"), "stdout:\n{stdout}");
}

#[test]
fn test_mapx_grep_mode_php() {
    let root = fixtures_dir();
    let output = Command::new(mapx_binary())
        .arg("--root")
        .arg(root)
        .arg("--query")
        .arg("GraphWalker")
        .arg("--mode")
        .arg("grep")
        .output()
        .expect("failed to run mapx");
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("GraphWalker"), "stdout:\n{stdout}");
}

#[test]
fn test_mapx_lines_format() {
    let root = fixtures_dir();
    let output = Command::new(mapx_binary())
        .arg("--root")
        .arg(root)
        .arg("--query")
        .arg("lazyBuildContext")
        .arg("--format")
        .arg("lines")
        .output()
        .expect("failed to run mapx");
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("lazyBuildContext"));
    // Should be lines, not JSON
    assert!(!stdout.trim().starts_with('['));
}

#[test]
fn test_mapx_empty_query() {
    let root = fixtures_dir();
    let output = Command::new(mapx_binary())
        .arg("--root")
        .arg(root)
        .arg("--query")
        .arg("")
        .output()
        .expect("failed to run mapx");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let val: serde_json::Value = serde_json::from_str(stdout.trim()).expect("invalid json");
    assert_eq!(val["tags"], serde_json::json!([]));
}

#[test]
fn test_mapx_no_match() {
    let root = fixtures_dir();
    let output = Command::new(mapx_binary())
        .arg("--root")
        .arg(root)
        .arg("--query")
        .arg("xyzzy_nonexistent_symbol_42")
        .output()
        .expect("failed to run mapx");
    // Should succeed with empty results
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let val: serde_json::Value = serde_json::from_str(stdout.trim()).expect("invalid json");
    assert_eq!(val["tags"], serde_json::json!([]));
}
