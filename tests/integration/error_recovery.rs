//! Error Recovery & Resilience Tests
//!
//! Tests system behavior under error conditions using command execution

use std::process::Command;
use tempfile::TempDir;
use git2::Repository;

/// Create a basic test repository for error testing
fn create_test_repository() -> TempDir {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo_path = temp_dir.path();
    
    let repo = Repository::init(repo_path).expect("Failed to init repository");
    
    // Configure git user
    let mut config = repo.config().expect("Failed to get config");
    config.set_str("user.name", "Error Test").expect("Failed to set user.name");
    config.set_str("user.email", "error@test.com").expect("Failed to set user.email");
    
    // Create a basic file and commit
    let file_path = repo_path.join("test.rs");
    std::fs::write(&file_path, "fn test() {}").expect("Failed to write file");
    
    // Add and commit the file
    let mut index = repo.index().expect("Failed to get index");
    index.add_path(std::path::Path::new("test.rs")).expect("Failed to add file");
    index.write().expect("Failed to write index");
    
    let tree_id = index.write_tree().expect("Failed to write tree");
    let tree = repo.find_tree(tree_id).expect("Failed to find tree");
    
    let signature = git2::Signature::now("Error Test", "error@test.com")
        .expect("Failed to create signature");
    
    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "Initial commit",
        &tree,
        &[],
    ).expect("Failed to create commit");
    
    temp_dir
}

#[test]
fn test_invalid_repository_path_error() {
    let output = Command::new("./target/debug/gstats")
        .arg("--repository")
        .arg("/completely/non/existent/path/to/repository")
        .arg("commits")
        .output()
        .expect("Failed to execute gstats");

    assert!(!output.status.success(), "Should fail with invalid repository path");
    
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.len() > 0, "Should have error message");
}

#[test]  
fn test_permission_denied_error() {
    let output = Command::new("./target/debug/gstats")
        .arg("--repository")
        .arg("/usr/bin") // System directory, not a git repo
        .arg("commits")
        .output()
        .expect("Failed to execute gstats");

    assert!(!output.status.success(), "Should fail with permission/format error");
    
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.len() > 0, "Should have error message");
}

#[test]
fn test_command_resilience() {
    let temp_repo = create_test_repository();
    let repo_path = temp_repo.path().to_str().unwrap();
    
    // Test basic command works
    let output = Command::new("./target/debug/gstats")
        .arg("--repository")
        .arg(repo_path)
        .arg("--no-color")
        .arg("commits")
        .output()
        .expect("Failed to execute gstats");
    
    assert!(output.status.success(), "Basic command should succeed");
    
    // Test with invalid flags - should handle gracefully
    let output = Command::new("./target/debug/gstats")
        .arg("--repository")
        .arg(repo_path)
        .arg("--invalid-flag")
        .arg("commits")
        .output()
        .expect("Failed to execute gstats");
    
    // Should either succeed (ignoring invalid flag) or fail gracefully
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.len() > 0, "Should have error message for invalid flag");
    }
}

#[test]
fn test_configuration_error_handling() {
    let temp_repo = create_test_repository();
    let repo_path = temp_repo.path().to_str().unwrap();
    
    // Test with invalid configuration file
    let invalid_config = temp_repo.path().join("invalid.toml");
    std::fs::write(&invalid_config, "invalid toml content [[[").expect("Failed to write invalid config");
    
    let output = Command::new("./target/debug/gstats")
        .arg("--repository")
        .arg(repo_path)
        .arg("--config-file")
        .arg(invalid_config.to_str().unwrap())
        .arg("commits")
        .output()
        .expect("Failed to execute gstats");
    
    // Should handle invalid config gracefully
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.len() > 0, "Should have error message for invalid config");
    }
}

#[test]
fn test_multiple_command_modes() {
    let temp_repo = create_test_repository();
    let repo_path = temp_repo.path().to_str().unwrap();
    
    // Test different command modes work
    let commands = vec!["commits", "authors", "metrics"];
    
    for command in commands {
        let output = Command::new("./target/debug/gstats")
            .arg("--repository")
            .arg(repo_path)
            .arg("--no-color")
            .arg(command)
            .output()
            .expect("Failed to execute gstats");
        
        assert!(output.status.success(), 
            "Command {} should succeed. stderr: {}", 
            command,
            String::from_utf8_lossy(&output.stderr));
    }
}