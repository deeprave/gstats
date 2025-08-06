//! Load Testing & Performance Tests
//!
//! Tests system behavior under various load conditions using command execution

use std::process::Command;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use git2::Repository;

/// Create a test repository with specified size characteristics
fn create_load_test_repository(name: &str, files_per_commit: usize, commits: usize) -> TempDir {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo_path = temp_dir.path();
    
    let repo = Repository::init(repo_path).expect("Failed to init repository");
    
    // Configure git user
    let mut config = repo.config().expect("Failed to get config");
    config.set_str("user.name", &format!("{} Load Test", name)).expect("Failed to set user.name");
    config.set_str("user.email", &format!("{}@loadtest.com", name.to_lowercase())).expect("Failed to set user.email");
    
    // Create files and commits for load testing
    for commit_i in 0..commits {
        for file_i in 0..files_per_commit {
            let file_path = repo_path.join(format!("load_file_{}_{}.rs", commit_i, file_i));
            let content = format!(
                "// Load test file {} in commit {}\n\
                 pub struct LoadTestData {{\n\
                     id: u64,\n\
                     data: String,\n\
                 }}\n\
                 \n\
                 impl LoadTestData {{\n\
                     pub fn new(id: u64) -> Self {{\n\
                         Self {{\n\
                             id,\n\
                             data: \"load_value_{}\".to_string(),\n\
                         }}\n\
                     }}\n\
                 }}\n",
                file_i, commit_i, file_i
            );
            std::fs::write(&file_path, content).expect("Failed to write file");
        }
        
        // Add files to git index
        let mut index = repo.index().expect("Failed to get index");
        for file_i in 0..files_per_commit {
            let relative_path = format!("load_file_{}_{}.rs", commit_i, file_i);
            index.add_path(&std::path::Path::new(&relative_path))
                .expect("Failed to add file");
        }
        index.write().expect("Failed to write index");
        
        // Create commit
        let tree_id = index.write_tree().expect("Failed to write tree");
        let tree = repo.find_tree(tree_id).expect("Failed to find tree");
        let signature = git2::Signature::now(&format!("{} Load Test", name), &format!("{}@loadtest.com", name.to_lowercase()))
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
            &format!("Load test commit {} - Add {} files", commit_i, files_per_commit),
            &tree,
            &parent_commits.iter().collect::<Vec<_>>(),
        ).expect("Failed to create commit");
    }
    
    temp_dir
}

#[test]
fn test_small_repository_load() {
    // Small repository: 5 files per commit, 3 commits = 15 files
    let temp_repo = create_load_test_repository("SmallLoad", 5, 3);
    let repo_path = temp_repo.path().to_str().unwrap();
    
    let start_time = Instant::now();
    
    // Test basic commands on small repository
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
            "Command {} should succeed on small repository. stderr: {}", 
            command,
            String::from_utf8_lossy(&output.stderr));
    }
    
    let duration = start_time.elapsed();
    assert!(duration < Duration::from_secs(5), "Small repository should scan quickly");
    
    println!("Small repository load test completed in {:?}", duration);
}

#[test]
fn test_medium_repository_load() {
    // Medium repository: 15 files per commit, 5 commits = 75 files
    let temp_repo = create_load_test_repository("MediumLoad", 15, 5);
    let repo_path = temp_repo.path().to_str().unwrap();
    
    let start_time = Instant::now();
    
    // Test commits command on medium repository
    let output = Command::new("./target/debug/gstats")
        .arg("--repository")
        .arg(repo_path)
        .arg("--no-color")
        .arg("commits")
        .output()
        .expect("Failed to execute gstats");
    
    assert!(output.status.success(), 
        "Commits command should succeed on medium repository. stderr: {}", 
        String::from_utf8_lossy(&output.stderr));
    
    let duration = start_time.elapsed();
    assert!(duration < Duration::from_secs(15), "Medium repository should scan reasonably quickly");
    
    println!("Medium repository load test completed in {:?}", duration);
}

#[test]
fn test_performance_with_no_color() {
    let temp_repo = create_load_test_repository("Performance", 10, 4);
    let repo_path = temp_repo.path().to_str().unwrap();
    
    // Test performance difference between color and no-color modes
    let start_no_color = Instant::now();
    let output_no_color = Command::new("./target/debug/gstats")
        .arg("--repository")
        .arg(repo_path)
        .arg("--no-color")
        .arg("commits")
        .output()
        .expect("Failed to execute gstats");
    let duration_no_color = start_no_color.elapsed();
    
    let start_color = Instant::now();
    let output_color = Command::new("./target/debug/gstats")
        .arg("--repository")
        .arg(repo_path)
        .arg("--color")
        .arg("commits")
        .output()
        .expect("Failed to execute gstats");
    let duration_color = start_color.elapsed();
    
    assert!(output_no_color.status.success(), "No-color command should succeed");
    assert!(output_color.status.success(), "Color command should succeed");
    
    println!("Performance test: no-color={:?}, color={:?}", duration_no_color, duration_color);
    
    // Color should not significantly impact performance
    let color_overhead = if duration_color > duration_no_color {
        duration_color - duration_no_color
    } else {
        Duration::from_millis(0)
    };
    
    assert!(color_overhead < Duration::from_millis(100), 
        "Color overhead should be minimal (was {:?})", color_overhead);
}

#[test]
fn test_concurrent_command_execution() {
    let temp_repo = create_load_test_repository("Concurrent", 8, 3);
    let repo_path = temp_repo.path().to_str().unwrap();
    
    let start_time = Instant::now();
    
    // Test that multiple commands can run (though not truly concurrent with Command)
    let commands = vec!["commits", "authors", "metrics"];
    let mut durations = Vec::new();
    
    for command in commands {
        let cmd_start = Instant::now();
        let output = Command::new("./target/debug/gstats")
            .arg("--repository")
            .arg(repo_path)
            .arg("--no-color")
            .arg(command)
            .output()
            .expect("Failed to execute gstats");
        let cmd_duration = cmd_start.elapsed();
        
        assert!(output.status.success(), 
            "Command {} should succeed. stderr: {}", 
            command,
            String::from_utf8_lossy(&output.stderr));
        
        durations.push((command, cmd_duration));
    }
    
    let total_duration = start_time.elapsed();
    
    for (command, duration) in durations {
        println!("Command {} completed in {:?}", command, duration);
        assert!(duration < Duration::from_secs(30), "Each command should complete efficiently");
    }
    
    println!("Sequential command execution completed in {:?}", total_duration);
    assert!(total_duration < Duration::from_secs(60), "Sequential operations should be efficient");
}

#[test]
fn test_export_functionality_load() {
    let temp_repo = create_load_test_repository("Export", 12, 4);
    let repo_path = temp_repo.path().to_str().unwrap();
    let export_file = temp_repo.path().join("config_export.toml");
    
    let start_time = Instant::now();
    
    // Test configuration export with load
    let output = Command::new("./target/debug/gstats")
        .arg("--repository")
        .arg(repo_path)
        .arg("--export-config")
        .arg(&export_file)
        .arg("commits")
        .output()
        .expect("Failed to execute gstats");
    
    let duration = start_time.elapsed();
    
    assert!(output.status.success(), 
        "Export command should succeed. stderr: {}", 
        String::from_utf8_lossy(&output.stderr));
    
    // Verify export file was created
    assert!(export_file.exists(), "Configuration export file should be created");
    
    let config_content = std::fs::read_to_string(&export_file).expect("Failed to read config");
    assert!(config_content.contains("# Complete gstats configuration file"), "Should have proper header");
    
    println!("Export functionality test completed in {:?}", duration);
    assert!(duration < Duration::from_secs(10), "Export should complete quickly");
}