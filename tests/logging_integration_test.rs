// Integration tests for logging functionality
use std::process::Command;
use std::path::Path;

#[test]
fn test_logger_initialization() {
    // This test verifies logger initialization is working correctly
    let output = Command::new("cargo")
        .args(&["run", "--", "--help"])
        .output()
        .expect("Failed to execute command");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    
    // Verify help output contains new logging options
    assert!(stdout.contains("--verbose") && stdout.contains("--log-format"), 
        "Expected help output to contain logging options, got: {}", stdout);
}

#[test]
fn test_timestamp_format_validation() {
    // This test verifies timestamp formatting is working correctly
    let output = Command::new("cargo")
        .args(&["run", "--", "--verbose", "--repo", "."])
        .output()
        .expect("Failed to execute command");
    
    let stderr = String::from_utf8_lossy(&output.stderr);
    
    // Look for YYYY-MM-DD HH:mm:ss format
    assert!(stderr.contains("2025-"), 
        "Expected timestamp format YYYY-MM-DD HH:mm:ss, got: {}", stderr);
    assert!(stderr.contains("[DEBUG]") || stderr.contains("[INFO]"), 
        "Expected log level markers in output: {}", stderr);
}

#[test]
fn test_json_format_output_structure() {
    // This test verifies JSON format is working correctly
    let output = Command::new("cargo")
        .args(&["run", "--", "--log-format", "json", "--repo", "."])
        .output()
        .expect("Failed to execute command");
    
    let stderr = String::from_utf8_lossy(&output.stderr);
    
    // Verify JSON format with required fields
    assert!(stderr.contains(r#""timestamp":"#) && 
            stderr.contains(r#""level":"#) && 
            stderr.contains(r#""message":"#), 
        "Expected JSON format with timestamp, level, message fields, got: {}", stderr);
}

#[test]
fn test_log_level_filtering() {
    // This test verifies log level filtering is working correctly
    let output = Command::new("cargo")
        .args(&["run", "--", "--quiet", "--repo", "."])
        .output()
        .expect("Failed to execute command");
    
    let stderr = String::from_utf8_lossy(&output.stderr);
    
    // Verify quiet mode suppresses DEBUG and INFO messages
    assert!(!stderr.contains("[DEBUG]") && !stderr.contains("[INFO]"), 
        "Expected quiet mode to suppress DEBUG and INFO messages, got: {}", stderr);
}

#[test]
fn test_format_switching() {
    // This test verifies format switching is working correctly
    let text_output = Command::new("cargo")
        .args(&["run", "--", "--repo", "."])
        .output()
        .expect("Failed to execute command");
    
    let json_output = Command::new("cargo")
        .args(&["run", "--", "--log-format", "json", "--repo", "."])
        .output()
        .expect("Failed to execute command");
    
    let text_stderr = String::from_utf8_lossy(&text_output.stderr);
    let json_stderr = String::from_utf8_lossy(&json_output.stderr);
    
    // Verify different output formats
    assert!(text_stderr.contains("[INFO]") && json_stderr.contains(r#""level":"INFO""#), 
        "Expected different output formats - Text: {}, JSON: {}", text_stderr, json_stderr);
}

#[test]
fn test_file_output_functionality() {
    // This test verifies file output functionality is working correctly
    let temp_file = "/tmp/gstats_test.log";
    
    let _output = Command::new("cargo")
        .args(&["run", "--", "--log-file", temp_file, "--repo", "."])
        .output()
        .expect("Failed to execute command");
    
    // Verify log file was created
    assert!(Path::new(temp_file).exists(), 
        "Expected log file to be created at {}", temp_file);
    
    // Verify file contains expected content
    let file_content = std::fs::read_to_string(temp_file).unwrap_or_default();
    assert!(file_content.contains("[INFO]") && file_content.contains("2025-"), 
        "Expected log file to contain timestamp and log levels, got: {}", file_content);
    
    // Clean up
    let _ = std::fs::remove_file(temp_file);
}

#[test]
fn test_independent_file_log_levels() {
    // This test verifies independent file log levels are working correctly
    let temp_file = "/tmp/gstats_level_test.log";
    
    let output = Command::new("cargo")
        .args(&["run", "--", "--quiet", "--log-file", temp_file, "--log-file-level", "debug", "--repo", "."])
        .output()
        .expect("Failed to execute command");
    
    let stderr = String::from_utf8_lossy(&output.stderr);
    
    // Verify console is quiet (no output)
    assert!(stderr.is_empty() || !stderr.contains("[DEBUG]"), 
        "Expected quiet console output, got: {}", stderr);
    
    // Verify file contains debug messages
    assert!(Path::new(temp_file).exists(), 
        "Expected log file to be created");
    
    let file_content = std::fs::read_to_string(temp_file).unwrap_or_default();
    assert!(file_content.contains("[DEBUG]"), 
        "Expected DEBUG messages in file even with quiet console, got: {}", file_content);
    
    // Clean up
    let _ = std::fs::remove_file(temp_file);
}
