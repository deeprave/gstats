//! Async Performance Validation Tests
//!
//! Tests for validating performance characteristics using command execution

use std::process::Command;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use git2::Repository;

/// Create a test repository for performance testing
fn create_performance_test_repository(files: usize, commits: usize) -> TempDir {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo_path = temp_dir.path();
    
    let repo = Repository::init(repo_path).expect("Failed to init repository");
    
    // Configure git user
    let mut config = repo.config().expect("Failed to get config");
    config.set_str("user.name", "Performance Test").expect("Failed to set user.name");
    config.set_str("user.email", "performance@test.com").expect("Failed to set user.email");
    
    // Create files and commits
    for commit_i in 0..commits {
        for file_i in 0..files {
            let file_path = repo_path.join(format!("perf_file_{}_{}.rs", commit_i, file_i));
            let content = format!(
                "// Performance test file {} in commit {}\n\
                 pub fn performance_function_{}() {{\n\
                     // Implementation with some complexity\n\
                     let mut result = 0;\n\
                     for i in 0..{} {{\n\
                         result += i * {};\n\
                     }}\n\
                     result\n\
                 }}\n",
                file_i, commit_i, file_i, file_i * 10, file_i
            );
            std::fs::write(&file_path, content).expect("Failed to write file");
        }
        
        // Add all files for this commit
        let mut index = repo.index().expect("Failed to get index");
        for file_i in 0..files {
            let relative_path = format!("perf_file_{}_{}.rs", commit_i, file_i);
            index.add_path(&std::path::Path::new(&relative_path))
                .expect("Failed to add file");
        }
        index.write().expect("Failed to write index");
        
        // Create commit
        let tree_id = index.write_tree().expect("Failed to write tree");
        let tree = repo.find_tree(tree_id).expect("Failed to find tree");
        let signature = git2::Signature::now("Performance Test", "performance@test.com")
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
            &format!("Performance test commit {}", commit_i),
            &tree,
            &parent_commits.iter().collect::<Vec<_>>(),
        ).expect("Failed to create commit");
    }
    
    temp_dir
}

#[test]
fn test_command_performance_responsiveness() {
    let temp_repo = create_performance_test_repository(50, 10);
    let repo_path = temp_repo.path().to_str().unwrap();
    
    // Test that scanning operations complete within reasonable time
    let start = Instant::now();
    
    let output = Command::new("./target/debug/gstats")
        .arg("--repository")
        .arg(repo_path)
        .arg("--no-color")
        .arg("commits")
        .output()
        .expect("Failed to execute gstats");
    
    let duration = start.elapsed();
    
    assert!(output.status.success(), 
        "Performance test should succeed. stderr: {}", 
        String::from_utf8_lossy(&output.stderr));
    
    // Should complete within reasonable time for medium-sized repository
    assert!(duration < Duration::from_secs(10), 
        "Command should complete within 10 seconds, took {:?}", duration);
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.len() > 0, "Should produce output");
}

#[test]
fn test_sequential_command_performance() {
    let temp_repo = create_performance_test_repository(30, 8);
    let repo_path = temp_repo.path().to_str().unwrap();
    
    // Test multiple sequential commands
    let commands = vec!["commits", "authors", "metrics"];
    let mut total_duration = Duration::from_secs(0);
    
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
        total_duration += duration;
        
        assert!(output.status.success(), 
            "Command {} should succeed. stderr: {}", 
            command,
            String::from_utf8_lossy(&output.stderr));
        
        assert!(duration < Duration::from_secs(15), 
            "Command {} should complete within 15 seconds, took {:?}", command, duration);
    }
    
    // Sequential execution should be reasonable
    assert!(total_duration < Duration::from_secs(30), 
        "Sequential commands should complete within 30 seconds, took {:?}", total_duration);
}

#[test]
fn test_large_repository_performance() {
    let temp_repo = create_performance_test_repository(40, 12);
    let repo_path = temp_repo.path().to_str().unwrap();
    
    // Test performance with larger repository
    let start = Instant::now();
    
    let output = Command::new("./target/debug/gstats")
        .arg("--repository")
        .arg(repo_path)
        .arg("--no-color")
        .arg("metrics") // Most intensive command
        .output()
        .expect("Failed to execute gstats");
    
    let duration = start.elapsed();
    
    assert!(output.status.success(), 
        "Large repository test should succeed. stderr: {}", 
        String::from_utf8_lossy(&output.stderr));
    
    // Should handle larger repository within reasonable time
    assert!(duration < Duration::from_secs(25), 
        "Large repository should complete within 25 seconds, took {:?}", duration);
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("=== Metrics Report ==="), "Should contain metrics output");
}

#[test]
fn test_color_performance_impact() {
    let temp_repo = create_performance_test_repository(25, 6);
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
    
    // Color overhead should be minimal
    let overhead = if duration_color > duration_no_color {
        duration_color - duration_no_color
    } else {
        Duration::from_millis(0)
    };
    
    assert!(overhead < Duration::from_millis(200), 
        "Color overhead should be minimal (was {:?})", overhead);
    
    println!("Performance impact: no-color={:?}, color={:?}, overhead={:?}", 
        duration_no_color, duration_color, overhead);
}

#[test]
fn test_configuration_export_performance() {
    let temp_repo = create_performance_test_repository(20, 4);
    let repo_path = temp_repo.path().to_str().unwrap();
    let export_file = temp_repo.path().join("performance_export.toml");
    
    // Test performance of configuration export
    let start = Instant::now();
    
    let output = Command::new("./target/debug/gstats")
        .arg("--repository")
        .arg(repo_path)
        .arg("--export-config")
        .arg(&export_file)
        .arg("commits")
        .output()
        .expect("Failed to execute gstats");
    
    let duration = start.elapsed();
    
    assert!(output.status.success(), 
        "Export performance test should succeed. stderr: {}", 
        String::from_utf8_lossy(&output.stderr));
    
    // Export should not significantly impact performance
    assert!(duration < Duration::from_secs(8), 
        "Export should complete quickly, took {:?}", duration);
    
    // Verify export file was created
    assert!(export_file.exists(), "Export file should be created");
    
    let config_size = std::fs::metadata(&export_file)
        .expect("Should get file metadata")
        .len();
    assert!(config_size > 100, "Export file should contain substantial content");
}

#[test]
fn test_memory_efficiency() {
    let temp_repo = create_performance_test_repository(60, 15);
    let repo_path = temp_repo.path().to_str().unwrap();
    
    // Test with various commands to ensure no memory leaks in quick succession
    let commands = vec!["commits", "authors", "metrics", "commits", "authors"];
    
    let overall_start = Instant::now();
    
    for (i, command) in commands.iter().enumerate() {
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
            "Memory efficiency test iteration {} ({}) should succeed. stderr: {}", 
            i, command, String::from_utf8_lossy(&output.stderr));
        
        // Each command should complete within reasonable time (no memory accumulation)
        assert!(duration < Duration::from_secs(20), 
            "Command {} (iteration {}) should complete efficiently, took {:?}", 
            command, i, duration);
    }
    
    let total_duration = overall_start.elapsed();
    
    // Total time should be reasonable for repeated operations
    assert!(total_duration < Duration::from_secs(60), 
        "Memory efficiency test should complete within 60 seconds, took {:?}", total_duration);
}