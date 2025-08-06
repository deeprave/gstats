//! Simple Error Recovery Tests
//! 
//! Basic error handling validation using command execution

use std::process::Command;

#[test]
fn test_invalid_repository_path() {
    let output = Command::new("./target/debug/gstats")
        .arg("--repository")
        .arg("/completely/non/existent/path")
        .arg("commits")
        .output()
        .expect("Failed to execute gstats");

    // Should fail with non-zero exit code
    assert!(!output.status.success(), "Should fail with invalid repository path");
    
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.len() > 0, "Should have error message");
}

#[test]
fn test_non_git_directory() {
    let output = Command::new("./target/debug/gstats")
        .arg("--repository")
        .arg("/tmp") // Not a git repository
        .arg("commits")
        .output()
        .expect("Failed to execute gstats");

    // Should fail with non-zero exit code
    assert!(!output.status.success(), "Should fail with non-git directory");
    
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.len() > 0, "Should have error message");
}

#[test]
fn test_invalid_command_error() {
    let output = Command::new("./target/debug/gstats")
        .arg("nonexistent_command")
        .output()
        .expect("Failed to execute gstats");

    // Should fail with non-zero exit code
    assert!(!output.status.success(), "Should fail with invalid command");
    
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Unknown command") || stderr.contains("Invalid"), "Should contain error about unknown command");
}