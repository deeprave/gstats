//! Integration tests for scanner module infrastructure

use gstats::scanner::ScannerConfig;

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
fn test_scan_message_creation() {
    // Test that scan messages can be created with proper structure
    use gstats::scanner::messages::{ScanMessage, MessageHeader, MessageData};
    
    let header = MessageHeader::new(12345);
    let data = MessageData::FileInfo {
        path: "src/main.rs".to_string(),
        size: 1024,
        lines: 50,
    };
    
    let message = ScanMessage { header, data };
    
    // Test message structure
    assert_eq!(message.header.sequence, 12345);
    
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
    
    let header = MessageHeader::new(67890);
    let data = MessageData::CommitInfo {
        hash: "abc123".to_string(),
        author: "developer".to_string(),
        message: "Fix bug".to_string(),
        timestamp: 1640995200, // Jan 1, 2022
        changed_files: vec![],
    };
    
    let message = ScanMessage { header, data };
    
    // Test message structure
    assert_eq!(message.header.sequence, 67890);
    
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
    
    let header = MessageHeader::new(9876543210);
    
    assert_eq!(header.sequence, 9876543210);
}