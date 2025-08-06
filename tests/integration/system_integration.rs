//! System Integration Tests
//!
//! Tests integration between system components using command execution

use std::process::Command;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use git2::Repository;

/// Create a test repository for system integration testing
fn create_system_test_repository(name: &str, files: usize, commits: usize) -> TempDir {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo_path = temp_dir.path();
    
    let repo = Repository::init(repo_path).expect("Failed to init repository");
    
    // Configure git user
    let mut config = repo.config().expect("Failed to get config");
    config.set_str("user.name", &format!("{} System Test", name)).expect("Failed to set user.name");
    config.set_str("user.email", &format!("{}@systemtest.com", name.to_lowercase())).expect("Failed to set user.email");
    
    // Create files and commits for testing
    for commit_i in 0..commits {
        for file_i in 0..files {
            let file_path = repo_path.join(format!("system_file_{}_{}.rs", commit_i, file_i));
            let content = format!(
                "// System integration test file {} in commit {}\n\
                 pub fn system_function_{}() {{\n\
                     println!(\"System integration test function {}\");\n\
                 }}\n",
                file_i, commit_i, file_i, file_i
            );
            std::fs::write(&file_path, content).expect("Failed to write file");
        }
        
        // Add files to git index
        let mut index = repo.index().expect("Failed to get index");
        for file_i in 0..files {
            let relative_path = format!("system_file_{}_{}.rs", commit_i, file_i);
            index.add_path(&std::path::Path::new(&relative_path))
                .expect("Failed to add file");
        }
        index.write().expect("Failed to write index");
        
        // Create commit
        let tree_id = index.write_tree().expect("Failed to write tree");
        let tree = repo.find_tree(tree_id).expect("Failed to find tree");
        let signature = git2::Signature::now(&format!("{} System Test", name), &format!("{}@systemtest.com", name.to_lowercase()))
            .expect("Failed to create signature");
        
        let parent_commits = if commit_i == 0 {
            vec![]
        } else {
            vec![repo.head().unwrap().peel_to_commit().unwrap()]
        };
        
        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            &format!("System integration commit {} - Add {} files", commit_i, files),
            &tree,
            &parent_commits.iter().collect::<Vec<_>>(),
        ).expect("Failed to create commit");
    }
    
    temp_dir
}

#[test]
fn test_basic_system_integration() {
    let temp_repo = create_system_test_repository("BasicIntegration", 10, 5);
    let repo_path = temp_repo.path().to_str().unwrap();
    
    // Test all basic commands work together
    let commands = vec!["commits", "authors", "metrics"];
    
    for command in commands {
        let start = Instant::now();
        let output = Command::new("./target/debug/gstats")
            .arg("--repository")
            .arg(repo_path)
            .arg("--no-color")
            .arg(command)
            .output()
            .expect("Failed to execute gstats");
        let duration = start.elapsed();
        
        assert!(output.status.success(), 
            "Command {} should succeed in system integration. stderr: {}", 
            command,
            String::from_utf8_lossy(&output.stderr));
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.len() > 0, "Command {} should produce output", command);
        
        assert!(duration < Duration::from_secs(10), "Command {} should complete quickly", command);
    }
}

#[test]
fn test_configuration_system_integration() {
    let temp_repo = create_system_test_repository("ConfigTest", 12, 6);
    let repo_path = temp_repo.path().to_str().unwrap();
    let config_file = temp_repo.path().join("test_config.toml");
    
    // Create a test configuration file
    let config_content = r#"
        color = false
        quiet = false
        
        [scanner]
        max_memory = "32MB"
    "#;
    std::fs::write(&config_file, config_content).expect("Failed to write config file");
    
    // Test system with custom configuration
    let output = Command::new("./target/debug/gstats")
        .arg("--repository")
        .arg(repo_path)
        .arg("--config-file")
        .arg(config_file.to_str().unwrap())
        .arg("commits")
        .output()
        .expect("Failed to execute gstats");
    
    assert!(output.status.success(), 
        "Configuration integration should succeed. stderr: {}", 
        String::from_utf8_lossy(&output.stderr));
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should not contain ANSI colors when color = false
    assert!(!stdout.contains("\u{001b}["), "Should not contain ANSI colors with color = false");
}

#[test]
fn test_export_system_integration() {
    let temp_repo = create_system_test_repository("ExportTest", 8, 4);
    let repo_path = temp_repo.path().to_str().unwrap();
    let export_file = temp_repo.path().join("system_export.toml");
    
    // Test configuration export functionality
    let output = Command::new("./target/debug/gstats")
        .arg("--repository")
        .arg(repo_path)
        .arg("--export-config")
        .arg(&export_file)
        .arg("commits")
        .output()
        .expect("Failed to execute gstats");
    
    assert!(output.status.success(), 
        "Export system integration should succeed. stderr: {}", 
        String::from_utf8_lossy(&output.stderr));
    
    // Verify export file was created and has expected content
    assert!(export_file.exists(), "Export file should be created");
    
    let config_content = std::fs::read_to_string(&export_file).expect("Failed to read export file");
    assert!(config_content.contains("# Complete gstats configuration file"), "Should have proper header");
    assert!(config_content.contains("[scanner]"), "Should have scanner section");
}

#[test]
fn test_error_handling_integration() {
    // Test error handling throughout the system
    let temp_repo = create_system_test_repository("ErrorTest", 5, 3);
    let repo_path = temp_repo.path().to_str().unwrap();
    
    // Test with invalid command - should handle gracefully
    let output = Command::new("./target/debug/gstats")
        .arg("--repository")
        .arg(repo_path)
        .arg("nonexistent_command")
        .output()
        .expect("Failed to execute gstats");
    
    assert!(!output.status.success(), "Invalid command should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.len() > 0, "Should have error message for invalid command");
    
    // Test with invalid repository - should handle gracefully  
    let output = Command::new("./target/debug/gstats")
        .arg("--repository")
        .arg("/nonexistent/path")
        .arg("commits")
        .output()
        .expect("Failed to execute gstats");
    
    assert!(!output.status.success(), "Invalid repository should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.len() > 0, "Should have error message for invalid repository");
}

#[test]
fn test_color_system_integration() {
    let temp_repo = create_system_test_repository("ColorTest", 6, 3);
    let repo_path = temp_repo.path().to_str().unwrap();
    
    // Test with colors enabled
    let output_color = Command::new("./target/debug/gstats")
        .arg("--repository")
        .arg(repo_path)
        .arg("--color")
        .arg("commits")
        .output()
        .expect("Failed to execute gstats");
    
    assert!(output_color.status.success(), "Color command should succeed");
    
    // Test with colors disabled
    let output_no_color = Command::new("./target/debug/gstats")
        .arg("--repository")
        .arg(repo_path)
        .arg("--no-color")
        .arg("commits")
        .output()
        .expect("Failed to execute gstats");
    
    assert!(output_no_color.status.success(), "No-color command should succeed");
    
    let stdout_no_color = String::from_utf8_lossy(&output_no_color.stdout);
    assert!(!stdout_no_color.contains("\u{001b}["), "Should not contain ANSI colors with --no-color");
}

#[test]
fn test_help_system_integration() {
    // Test help system integration
    let output = Command::new("./target/debug/gstats")
        .arg("--help")
        .output()
        .expect("Failed to execute gstats");
    
    assert!(output.status.success(), "Help command should succeed");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Fast, local-first git analytics tool"), "Should contain app description");
    assert!(stdout.contains("--color"), "Should mention color option");
    assert!(stdout.contains("--config-file"), "Should mention config file option");
    assert!(stdout.contains("commits"), "Should mention commits command");
    assert!(stdout.contains("authors"), "Should mention authors command");
    assert!(stdout.contains("metrics"), "Should mention metrics command");
}