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
fn test_export_json_format_with_data() {
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
    
    // Run the export command (which defaults to json)
    let output = Command::new("cargo")
        .args(&["run", "--quiet", "--bin", "gstats", "--", "export", "--no-color", "--repo", temp_dir.path().to_str().unwrap()])
        .output()
        .expect("Failed to execute command");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    println!("Export JSON output:\n{}", stdout);
    
    // Find the JSON part in the output (it's after the Export Report header)
    let report_start = stdout.find("=== Export Report ===")
        .expect("Should find Export Report header");
    
    // Look for JSON after the report header
    let json_start = stdout[report_start..].find("{")
        .map(|pos| report_start + pos)
        .expect("JSON output should start with { after Export Report header");
    
    // Find the end of the JSON object - look for the last } in the remaining text
    let json_end = stdout[json_start..].rfind("}")
        .map(|pos| json_start + pos)
        .expect("JSON output should end with }");
    
    let json_str = &stdout[json_start..=json_end];
    
    // Try to parse as JSON
    let json: serde_json::Value = serde_json::from_str(json_str)
        .expect("Export output should be valid JSON");
    
    // Check that exported_data contains actual data
    if let Some(exported_data) = json.get("exported_data") {
        if let Some(exported_str) = exported_data.as_str() {
            let exported_json: serde_json::Value = serde_json::from_str(exported_str)
                .expect("Exported data should be valid JSON");
            
            // Check for non-empty scan results
            if let Some(scan_results) = exported_json.get("scan_results") {
                assert!(
                    scan_results.as_array().map_or(false, |arr| !arr.is_empty()),
                    "Scan results should not be empty"
                );
            }
        }
    }
}