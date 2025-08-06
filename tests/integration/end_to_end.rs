//! End-to-End Integration Tests
//!
//! Tests complete workflows using the current plugin-based architecture.
//! Covers basic functionality with real Git repositories.

use std::process::Command;
use tempfile::TempDir;
use git2::Repository;

/// Create a simple test repository for end-to-end testing
fn create_test_repository() -> TempDir {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo_path = temp_dir.path();
    
    let repo = Repository::init(repo_path).expect("Failed to init repository");
    
    // Configure git user
    let mut config = repo.config().expect("Failed to get config");
    config.set_str("user.name", "Test User").expect("Failed to set user.name");
    config.set_str("user.email", "test@example.com").expect("Failed to set user.email");
    
    // Create a simple file and commit
    let file_path = repo_path.join("README.md");
    std::fs::write(&file_path, "# Test Project\n\nThis is a test.").expect("Failed to write file");
    
    // Add and commit the file
    let mut index = repo.index().expect("Failed to get index");
    index.add_path(std::path::Path::new("README.md")).expect("Failed to add file");
    index.write().expect("Failed to write index");
    
    let tree_id = index.write_tree().expect("Failed to write tree");
    let tree = repo.find_tree(tree_id).expect("Failed to find tree");
    
    let signature = git2::Signature::now("Test User", "test@example.com")
        .expect("Failed to create signature");
    
    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "Initial commit",
        &tree,
        &[],
    ).expect("Failed to create commit");
    
    // Add another file and commit
    let file2_path = repo_path.join("src").join("main.rs");
    std::fs::create_dir_all(repo_path.join("src")).expect("Failed to create src dir");
    std::fs::write(&file2_path, "fn main() {\n    println!(\"Hello, world!\");\n}").expect("Failed to write file");
    
    index.add_path(std::path::Path::new("src/main.rs")).expect("Failed to add file");
    index.write().expect("Failed to write index");
    
    let tree_id = index.write_tree().expect("Failed to write tree");
    let tree = repo.find_tree(tree_id).expect("Failed to find tree");
    let parent_commit = repo.head().unwrap().peel_to_commit().unwrap();
    
    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "Add main.rs",
        &tree,
        &[&parent_commit],
    ).expect("Failed to create second commit");
    
    temp_dir
}

#[test]
fn test_basic_commits_command() {
    let temp_repo = create_test_repository();
    let repo_path = temp_repo.path().to_str().unwrap();
    
    // Test commits command using the built binary
    let output = Command::new("./target/debug/gstats")
        .arg("--repository")
        .arg(repo_path)
        .arg("--no-color")
        .arg("commits")
        .output()
        .expect("Failed to execute gstats");
    
    assert!(output.status.success(), 
        "Command should succeed. stderr: {}", 
        String::from_utf8_lossy(&output.stderr));
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    
    // Check that basic commit analysis output is present
    assert!(stdout.contains("Commit Analysis Report"), "Should contain report header");
    assert!(stdout.contains("Total Commits"), "Should show commit count");
    assert!(stdout.contains("Unique Authors"), "Should show author count");
}

#[test]
fn test_authors_command() {
    let temp_repo = create_test_repository();
    let repo_path = temp_repo.path().to_str().unwrap();
    
    let output = Command::new("./target/debug/gstats")
        .arg("--repository")
        .arg(repo_path)
        .arg("--no-color")
        .arg("authors")
        .output()
        .expect("Failed to execute gstats");
    
    assert!(output.status.success(), 
        "Command should succeed. stderr: {}", 
        String::from_utf8_lossy(&output.stderr));
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Author Analysis Report"), "Should contain author report");
    assert!(stdout.contains("Test User"), "Should show the test user");
}

#[test]
fn test_metrics_command() {
    let temp_repo = create_test_repository();
    let repo_path = temp_repo.path().to_str().unwrap();
    
    let output = Command::new("./target/debug/gstats")
        .arg("--repository")
        .arg(repo_path)
        .arg("--no-color")
        .arg("metrics")
        .output()
        .expect("Failed to execute gstats");
    
    assert!(output.status.success(), 
        "Command should succeed. stderr: {}", 
        String::from_utf8_lossy(&output.stderr));
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("=== Metrics Report ==="), "Should contain metrics report");
}

#[test]
fn test_color_flags() {
    let temp_repo = create_test_repository();
    let repo_path = temp_repo.path().to_str().unwrap();
    
    // Test --no-color flag
    let output = Command::new("./target/debug/gstats")
        .arg("--repository")
        .arg(repo_path)
        .arg("--no-color")
        .arg("commits")
        .output()
        .expect("Failed to execute gstats");
    
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    
    // Should not contain ANSI escape sequences when --no-color is used
    assert!(!stdout.contains("\u{001b}["), "Should not contain ANSI colors with --no-color");
}

#[test]
fn test_configuration_export() {
    let temp_repo = create_test_repository();
    let repo_path = temp_repo.path().to_str().unwrap();
    let export_file = temp_repo.path().join("exported_config.toml");
    
    let output = Command::new("./target/debug/gstats")
        .arg("--repository")
        .arg(repo_path)
        .arg("--export-config")
        .arg(&export_file)
        .arg("commits")
        .output()
        .expect("Failed to execute gstats");
    
    assert!(output.status.success(), 
        "Export config command should succeed. stderr: {}", 
        String::from_utf8_lossy(&output.stderr));
    
    // Check that config file was created
    assert!(export_file.exists(), "Config file should be created");
    
    // Check that config file contains expected content
    let config_content = std::fs::read_to_string(&export_file).expect("Failed to read config");
    assert!(config_content.contains("# Complete gstats configuration file"), "Should have header");
    assert!(config_content.contains("[scanner]"), "Should have scanner section");
}

#[test]
fn test_invalid_repository_error() {
    let output = Command::new("./target/debug/gstats")
        .arg("--repository")
        .arg("/nonexistent/path")
        .arg("commits")
        .output()
        .expect("Failed to execute gstats");
    
    // Should fail with invalid repository
    assert!(!output.status.success(), "Should fail with invalid repository");
    
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.len() > 0, "Should have error message");
}

#[test]
fn test_help_output() {
    let output = Command::new("./target/debug/gstats")
        .arg("--help")
        .output()
        .expect("Failed to execute gstats");
    
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    
    assert!(stdout.contains("Fast, local-first git analytics tool"), "Should contain app description");
    assert!(stdout.contains("--color"), "Should mention color option");
    assert!(stdout.contains("--config-file"), "Should mention config file option");
}