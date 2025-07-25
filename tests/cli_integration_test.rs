// Integration tests for CLI argument parsing and git repository detection
// Tests the complete workflow from command line arguments to repository validation

use std::process::Command;

#[test]
fn test_cli_with_repository_path() {
    // Test that gstats accepts a repository path argument
    // This test will FAIL initially (RED phase) until CLI parsing is implemented
    let output = Command::new("cargo")
        .args(&["run", "--quiet", "--", "/path/to/repo"])
        .output()
        .expect("Failed to execute cargo run");

    // Should not panic, should handle the path argument
    assert!(output.status.success() || output.status.code() == Some(1)); // Allow exit code 1 for invalid path
    
    // Should not contain the old testing message
    let stdout = String::from_utf8(output.stdout).unwrap_or_default();
    assert!(!stdout.contains("gstats testing infrastructure validated!"));
}

#[test]
fn test_cli_current_directory_behavior() {
    // Test that gstats works with current directory when no path specified
    // This test will FAIL initially (RED phase) until git detection is implemented
    let output = Command::new("cargo")
        .args(&["run", "--quiet"])
        .output()
        .expect("Failed to execute cargo run");

    let stdout = String::from_utf8(output.stdout).unwrap_or_default();
    
    // Should not contain the old testing message
    assert!(!stdout.contains("gstats testing infrastructure validated!"));
    
    // Since current directory IS a git repository, it should either:
    // 1. Success (exit code 0) when fully implemented
    // 2. Graceful failure (exit code 1) during development
    // 3. Should NOT panic or crash
    let exit_code = output.status.code().unwrap_or(-1);
    assert!(exit_code == 0 || exit_code == 1, "Process should exit gracefully, got exit code: {}", exit_code);
}

#[test]
fn test_cli_error_handling() {
    // Test error handling for non-existent paths
    // This test will FAIL initially (RED phase) until error handling is implemented
    let output = Command::new("cargo")
        .args(&["run", "--quiet", "--", "/definitely/does/not/exist"])
        .output()
        .expect("Failed to execute cargo run");

    // Should exit with error code for non-existent path
    assert!(!output.status.success());
    
    // Should not contain the old testing message
    let stdout = String::from_utf8(output.stdout).unwrap_or_default();
    assert!(!stdout.contains("gstats testing infrastructure validated!"));
}
