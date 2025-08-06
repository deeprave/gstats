//! Integration tests for scanner module infrastructure

use gstats::scanner::{ScannerConfig, ScanMode};

#[test]
fn test_scanner_module_exists() {
    // Test that the scanner module can be imported
    let _config = ScannerConfig::default();
}

#[test]
fn test_scanner_config_defaults() {
    // Test that scanner configuration has sensible defaults
    let config = ScannerConfig::default();
    
    // Test default values are reasonable
    assert!(config.max_memory_bytes > 0, "Max memory should be positive");
    assert!(config.queue_size > 0, "Queue size should be positive");
    assert!(config.max_memory_bytes >= 1024 * 1024, "Should have at least 1MB default memory");
    assert!(config.queue_size >= 100, "Should have reasonable queue size");
    // max_threads is Option<usize>, default is None
    assert!(config.max_threads.is_none() || config.max_threads.unwrap() >= 1, "Should have at least one thread if set");
}

#[test]
fn test_scan_modes_bitflags() {
    // Test that scan modes can be combined using bitwise operations
    let files_only = ScanMode::FILES;
    let history_only = ScanMode::HISTORY;
    let combined = ScanMode::FILES | ScanMode::HISTORY;
    
    // Test individual modes
    assert!(combined.contains(ScanMode::FILES), "Combined mode should contain FILES");
    assert!(combined.contains(ScanMode::HISTORY), "Combined mode should contain HISTORY");
    
    // Test bitwise operations
    assert_ne!(files_only, history_only, "Different modes should not be equal");
    assert!(combined.intersects(files_only), "Combined should intersect with FILES");
    assert!(combined.intersects(history_only), "Combined should intersect with HISTORY");
    
    // Test empty mode
    let empty = ScanMode::empty();
    assert!(empty.is_empty(), "Empty mode should be empty");
    assert!(!empty.contains(ScanMode::FILES), "Empty mode should not contain FILES");
}

#[test]
fn test_scan_message_creation() {
    // Test that scan messages can be created with proper structure
    use gstats::scanner::messages::{ScanMessage, MessageHeader, MessageData};
    
    let header = MessageHeader {
        scan_mode: ScanMode::FILES,
        timestamp: 12345,
    };
    let data = MessageData::FileInfo {
        path: "src/main.rs".to_string(),
        size: 1024,
        lines: 50,
    };
    
    let message = ScanMessage { header, data };
    
    // Test message structure
    assert_eq!(message.header.scan_mode, ScanMode::FILES);
    assert_eq!(message.header.timestamp, 12345);
    
    // Test data extraction
    if let MessageData::FileInfo { path, size, lines } = &message.data {
        assert_eq!(path, "src/main.rs");
        assert_eq!(*size, 1024);
        assert_eq!(*lines, 50);
    } else {
        panic!("Expected FileInfo data");
    }
}

#[test]
fn test_commit_message_creation() {
    use gstats::scanner::messages::{ScanMessage, MessageHeader, MessageData};
    
    let header = MessageHeader {
        scan_mode: ScanMode::HISTORY,
        timestamp: 67890,
    };
    let data = MessageData::CommitInfo {
        hash: "abc123".to_string(),
        author: "developer".to_string(),
        message: "Fix bug".to_string(),
        timestamp: 1640995200, // Jan 1, 2022
        changed_files: vec![],
    };
    
    let message = ScanMessage { header, data };
    
    // Test message structure
    assert_eq!(message.header.scan_mode, ScanMode::HISTORY);
    assert_eq!(message.header.timestamp, 67890);
    
    // Test commit data
    if let MessageData::CommitInfo { hash, author, message: msg, timestamp, changed_files } = &message.data {
        assert_eq!(hash, "abc123");
        assert_eq!(author, "developer");
        assert_eq!(msg, "Fix bug");
        assert_eq!(*timestamp, 1640995200);
        assert_eq!(changed_files.len(), 0);
    } else {
        panic!("Expected CommitInfo data");
    }
}

#[test]
fn test_message_data_variants() {
    use gstats::scanner::messages::MessageData;
    
    // Test None variant
    let none_data = MessageData::None;
    matches!(none_data, MessageData::None);
    
    // Test FileInfo variant
    let file_data = MessageData::FileInfo {
        path: "test.rs".to_string(),
        size: 100,
        lines: 10,
    };
    matches!(file_data, MessageData::FileInfo { .. });
    
    // Test CommitInfo variant
    let commit_data = MessageData::CommitInfo {
        hash: "123abc".to_string(),
        author: "dev@test.com".to_string(),
        message: "Test".to_string(),
        timestamp: 1234567890,
        changed_files: vec![],
    };
    matches!(commit_data, MessageData::CommitInfo { .. });
    
    // Test MetricInfo variant
    let metric_data = MessageData::MetricInfo {
        file_count: 1,
        line_count: 10,
        complexity: 1.0,
    };
    matches!(metric_data, MessageData::MetricInfo { .. });
}

#[test]
fn test_scan_mode_operations() {
    // Test all available scan modes
    let all_modes = ScanMode::all();
    assert!(all_modes.contains(ScanMode::FILES));
    assert!(all_modes.contains(ScanMode::HISTORY));
    
    // Test mode combinations
    let files_and_history = ScanMode::FILES | ScanMode::HISTORY;
    assert!(files_and_history.intersects(ScanMode::FILES));
    assert!(files_and_history.intersects(ScanMode::HISTORY));
    
    // Test mode subtraction
    let only_files = files_and_history - ScanMode::HISTORY;
    assert!(only_files.contains(ScanMode::FILES));
    assert!(!only_files.contains(ScanMode::HISTORY));
    
    // Test empty vs non-empty
    assert!(!ScanMode::empty().intersects(ScanMode::FILES));
    assert!(ScanMode::FILES.intersects(ScanMode::FILES));
}

#[test]
fn test_scanner_config_fields() {
    // Test that scanner configuration has expected fields
    let config = ScannerConfig::default();
    
    // Test all required fields exist and have sensible values
    assert!(config.max_memory_bytes >= 64 * 1024 * 1024); // At least 64MB
    assert!(config.queue_size >= 1000); // At least 1000 items
    // max_threads is Option<usize>, can be None
    if let Some(threads) = config.max_threads {
        assert!(threads >= 1 && threads <= 256); // Reasonable thread count if set
    }
}

#[test]
fn test_message_header_fields() {
    use gstats::scanner::messages::MessageHeader;
    
    let header = MessageHeader {
        scan_mode: ScanMode::METRICS,
        timestamp: 9876543210,
    };
    
    assert_eq!(header.scan_mode, ScanMode::METRICS);
    assert_eq!(header.timestamp, 9876543210);
}