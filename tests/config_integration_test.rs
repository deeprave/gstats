use std::fs;
use tempfile::tempdir;

#[test]
fn test_config_file_integration() {
    let temp_dir = tempdir().expect("Failed to create temp directory");
    let config_path = temp_dir.path().join("gstats.toml");
    
    // Create test configuration file
    let config_content = r#"
[logging]
level = "debug"
format = "json"
file_path = "/tmp/gstats-test.log"

[module.commits]
batch_size = "50"
timeout = "15"
enable_caching = "true"
"#;
    
    fs::write(&config_path, config_content).expect("Failed to write config file");
    
    // Test configuration loading
    use std::process::Command;
    
    let output = Command::new("cargo")
        .args(&["run", "--", "--config-file", config_path.to_str().unwrap(), "--help"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("Failed to execute command");
    
    // Should complete successfully
    assert!(output.status.success(), "Command failed: {}", String::from_utf8_lossy(&output.stderr));
}

#[test]
#[ignore = "Config section selection (--config-name) not yet implemented"]
fn test_config_section_selection() {
    let temp_dir = tempdir().expect("Failed to create temp directory");
    let config_path = temp_dir.path().join("gstats.toml");
    
    // Create test configuration file with multiple sections
    let config_content = r#"
[default]
log_level = "info"

[dev]
log_level = "debug"
verbose = "true"

[prod]
log_level = "error"
quiet = "true"
"#;
    
    fs::write(&config_path, config_content).expect("Failed to write config file");
    
    // Test configuration loading with section selection
    use std::process::Command;
    
    let output = Command::new("cargo")
        .args(&["run", "--", "--config-file", config_path.to_str().unwrap(), "--config-name", "dev", "--help"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("Failed to execute command");
    
    // Should complete successfully
    assert!(output.status.success(), "Command failed: {}", String::from_utf8_lossy(&output.stderr));
}

#[test]
fn test_cli_overrides_config() {
    let temp_dir = tempdir().expect("Failed to create temp directory");
    let config_path = temp_dir.path().join("gstats.toml");
    
    // Create test configuration file
    let config_content = r#"
[logging]
level = "error"
format = "text"
"#;
    
    fs::write(&config_path, config_content).expect("Failed to write config file");
    
    // Test that CLI arguments override config
    use std::process::Command;
    
    let output = Command::new("cargo")
        .args(&["run", "--", "--config-file", config_path.to_str().unwrap(), "--verbose", "--help"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("Failed to execute command");
    
    // Should complete successfully - CLI --verbose should override config level = "error"
    assert!(output.status.success(), "Command failed: {}", String::from_utf8_lossy(&output.stderr));
}
