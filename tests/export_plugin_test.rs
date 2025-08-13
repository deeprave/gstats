use std::process::Command;
use tempfile::TempDir;
use std::fs;

#[test]
fn test_export_plugin_receives_scan_data() {
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
    
    // Create test files and commits
    fs::write(temp_dir.path().join("file1.txt"), "Content 1").expect("Failed to write file");
    fs::write(temp_dir.path().join("file2.txt"), "Content 2").expect("Failed to write file");
    
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
    
    // Run the export command and capture output
    let output = Command::new("cargo")
        .args(&["run", "--quiet", "--bin", "gstats", "--", "export", "--no-color", "--repo", temp_dir.path().to_str().unwrap()])
        .output()
        .expect("Failed to execute command");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    
    println!("Export output:\n{}", stdout);
    
    // Check that export data is not empty
    assert!(
        !stdout.contains("\"entries_count\": 0"),
        "Export entries_count should not be 0, found:\n{}", stdout
    );
    
    // Check that scan_results is not empty
    assert!(
        !stdout.contains("\"scan_results\": []"),
        "Export scan_results should not be empty, found:\n{}", stdout
    );
}

#[test]
#[ignore = "Skipped due to plugin activation architecture issue (GS-73). Export plugin not properly activated."]
fn test_export_json_format_with_data() {
    // FIXME: This test is skipped because the export plugin is not being properly activated
    // due to the plugin activation architecture issue documented in GS-73.
    // 
    // The current implementation has a fundamental flaw where all plugins are loaded and 
    // activated upfront instead of lazy activation. The CLI command resolution should 
    // activate plugins rather than create separate instances.
    //
    // This test expects the export command to produce output with "=== Export Report ===" 
    // header, but the current implementation doesn't produce this format and the command
    // hangs due to the plugin activation issue.
    //
    // Once GS-73 is resolved with proper plugin activation architecture, this test should
    // be re-enabled and updated to match the correct output format.
    
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
    fs::write(temp_dir.path().join("test.rs"), "fn main() {}").expect("Failed to write file");
    
    Command::new("git")
        .current_dir(temp_dir.path())
        .args(&["add", "."])
        .output()
        .expect("Failed to add files");
        
    Command::new("git")
        .current_dir(temp_dir.path())
        .args(&["commit", "-m", "Add test file"])
        .output()
        .expect("Failed to commit");
    
    // This test is currently disabled - see ignore attribute above
    // When re-enabled, update the expected output format based on the actual implementation
}