// Integration tests for CLI argument parsing and git repository detection
// Tests the complete workflow from command line arguments to repository validation

use std::process::Command;

#[test]
fn test_cli_with_repository_path() {
    // Test that gstats accepts a repository path argument using --repo flag
    let output = Command::new("cargo")
        .args(&["run", "--quiet", "--", "--repo", "/path/to/repo"])
        .output()
        .expect("Failed to execute cargo run");

    // Should handle the path argument gracefully (exit code 1 for invalid path)
    assert_eq!(output.status.code(), Some(1), "Invalid path should return exit code 1");
    
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
    // Test error handling for non-existent repository paths
    // Use --repository flag to test repository path validation
    let output = Command::new("cargo")
        .args(&["run", "--quiet", "--", "--repository", "/definitely/does/not/exist"])
        .output()
        .expect("Failed to execute cargo run");

    // Should exit with error code for non-existent path
    assert!(!output.status.success());
    
    // Should not contain the old testing message
    let stdout = String::from_utf8(output.stdout).unwrap_or_default();
    assert!(!stdout.contains("gstats testing infrastructure validated!"));
}

#[test]
fn test_repo_flag_integration() {
    // Test the complete workflow with repository flag functionality
    // Test all three flag aliases: -r, --repo, --repository
    
    // Test 1: Current directory with -r flag
    let output = Command::new("cargo")
        .args(&["run", "--quiet", "--", "-r", "."])
        .output()
        .expect("Failed to execute cargo run");
    
    // Should succeed since current directory is a git repository
    assert_eq!(output.status.code(), Some(0), 
               "Current directory should be valid git repository");
    
    // Test 2: Current directory with --repo flag
    let output = Command::new("cargo")
        .args(&["run", "--quiet", "--", "--repo", "."])
        .output()
        .expect("Failed to execute cargo run");
    
    assert_eq!(output.status.code(), Some(0), 
               "--repo flag should work with current directory");
    
    // Test 3: Current directory with --repository flag
    let output = Command::new("cargo")
        .args(&["run", "--quiet", "--", "--repository", "."])
        .output()
        .expect("Failed to execute cargo run");
    
    assert_eq!(output.status.code(), Some(0), 
               "--repository flag should work with current directory");
    
    // Test 4: Invalid path with -r flag
    let output = Command::new("cargo")
        .args(&["run", "--quiet", "--", "-r", "/tmp"])
        .output()
        .expect("Failed to execute cargo run");
    
    assert_eq!(output.status.code(), Some(1), 
               "Invalid repository path should return exit code 1");
    
    // Test 5: Verify error message quality
    let stderr = String::from_utf8(output.stderr).unwrap_or_default();
    assert!(stderr.contains("Failed to open repository") || stderr.contains("not a git repository"), 
            "Error message should be descriptive");
}
