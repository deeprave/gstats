use std::process::Command;
use tempfile::TempDir;
use std::fs;

#[test]
fn test_no_duplicate_log_after_plugin_output() {
    // Create a temporary directory for testing
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    
    // Initialize a git repo in the temp directory
    Command::new("git")
        .current_dir(temp_dir.path())
        .args(&["init"])
        .output()
        .expect("Failed to init git repo");
        
    // Configure git user
    Command::new("git")
        .current_dir(temp_dir.path())
        .args(&["config", "user.email", "test@example.com"])
        .output()
        .expect("Failed to set git email");
        
    Command::new("git")
        .current_dir(temp_dir.path())
        .args(&["config", "user.name", "Test User"])
        .output()
        .expect("Failed to set git name");
    
    // Create a test file and commit
    fs::write(temp_dir.path().join("test.txt"), "Initial content").expect("Failed to write file");
    
    Command::new("git")
        .current_dir(temp_dir.path())
        .args(&["add", "."])
        .output()
        .expect("Failed to add files");
        
    Command::new("git")
        .current_dir(temp_dir.path())
        .args(&["commit", "-m", "Initial commit"])
        .output()
        .expect("Failed to commit");
    
    // Run the authors command and capture output
    let output = Command::new("cargo")
        .args(&["run", "--quiet", "--bin", "gstats", "--", "authors", "--no-color", "--repo", temp_dir.path().to_str().unwrap()])
        .env("RUST_LOG", "info")
        .output()
        .expect("Failed to execute command");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined_output = format!("{}\n{}", stderr, stdout);  // stderr first since logs appear first
    
    // Debug output
    println!("STDOUT:\n{}", stdout);
    println!("STDERR:\n{}", stderr);
    
    // With INFO log level, Analysis Summary SHOULD appear since it's now INFO level
    // But it should appear BEFORE the plugin output (not duplicate after it)
    assert!(
        combined_output.contains("Analysis Summary:"),
        "Analysis Summary should appear in output with RUST_LOG=info"
    );
    
    // Verify the Analysis Summary appears BEFORE the Author Analysis Report
    let summary_pos = combined_output.find("Analysis Summary:").expect("Analysis Summary not found");
    let report_pos = combined_output.find("=== Author Analysis Report ===").expect("Author report not found");
    assert!(
        summary_pos < report_pos,
        "Analysis Summary should appear before the Author Analysis Report (no duplicate after)"
    );
    
    // The Author Analysis Report should still appear
    assert!(
        combined_output.contains("=== Author Analysis Report ==="),
        "Author Analysis Report not found in output"
    );
}

#[test]
fn test_log_level_for_analysis_summary() {
    // Create a temporary directory for testing
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    
    // Initialize a git repo in the temp directory
    Command::new("git")
        .current_dir(temp_dir.path())
        .args(&["init"])
        .output()
        .expect("Failed to init git repo");
        
    // Configure git user
    Command::new("git")
        .current_dir(temp_dir.path())
        .args(&["config", "user.email", "test@example.com"])
        .output()
        .expect("Failed to set git email");
        
    Command::new("git")
        .current_dir(temp_dir.path())
        .args(&["config", "user.name", "Test User"])
        .output()
        .expect("Failed to set git name");
    
    // Create a test file and commit
    fs::write(temp_dir.path().join("test.txt"), "Initial content").expect("Failed to write file");
    
    Command::new("git")
        .current_dir(temp_dir.path())
        .args(&["add", "."])
        .output()
        .expect("Failed to add files");
        
    Command::new("git")
        .current_dir(temp_dir.path())
        .args(&["commit", "-m", "Initial commit"])
        .output()
        .expect("Failed to commit");
    
    // Run with --quiet flag (should not show INFO messages)
    let output = Command::new("cargo")
        .args(&["run", "--quiet", "--bin", "gstats", "--", "authors", "--no-color", "--quiet", "--repo", temp_dir.path().to_str().unwrap()])
        .output()
        .expect("Failed to execute command");
    
    let stderr = String::from_utf8_lossy(&output.stderr);
    
    // Analysis Summary should not appear when using --quiet flag (ERROR level only)
    assert!(
        !stderr.contains("Analysis Summary:"),
        "Analysis Summary should not appear with --quiet flag"
    );
}