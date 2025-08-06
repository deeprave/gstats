//! Integration tests for the scanner API
//! 
//! Simplified tests that verify scanner components work with current architecture

use gstats::scanner::{ScannerConfig, ScanMode};

#[test]
fn test_scanner_configuration_system() {
    // Test default configuration
    let default_config = ScannerConfig::default();
    assert!(default_config.max_memory_bytes > 0);
    assert!(default_config.queue_size > 0);
    
    // Test that default configuration has expected structure
    // max_threads is Option<usize>, default is None
    assert!(default_config.max_threads.is_none() || default_config.max_threads.unwrap() >= 1);
}

#[test]
fn test_scanning_modes() {
    // Test that ScanMode bitflags work
    let files_mode = ScanMode::FILES;
    let history_mode = ScanMode::HISTORY;
    let combined = files_mode | history_mode;
    
    assert!(combined.contains(ScanMode::FILES));
    assert!(combined.contains(ScanMode::HISTORY));
    assert!(!combined.contains(ScanMode::METRICS));
    
    // Test empty mode
    let empty = ScanMode::empty();
    assert!(empty.is_empty());
}

#[test]
fn test_message_data_types() {
    use gstats::scanner::messages::{MessageHeader, ScanMessage, MessageData};
    
    // Create a message header
    let header = MessageHeader {
        scan_mode: ScanMode::FILES,
        timestamp: 1234567890,
    };
    
    // Create scan message with file info
    let file_data = MessageData::FileInfo {
        path: "src/main.rs".to_string(),
        size: 1024,
        lines: 50,
    };
    
    let message = ScanMessage {
        header,
        data: file_data,
    };
    
    // Test message properties
    assert_eq!(message.header.scan_mode, ScanMode::FILES);
    assert_eq!(message.header.timestamp, 1234567890);
    
    match message.data {
        MessageData::FileInfo { path, size, lines } => {
            assert_eq!(path, "src/main.rs");
            assert_eq!(size, 1024);
            assert_eq!(lines, 50);
        }
        _ => panic!("Expected FileInfo data"),
    }
}

#[test]
fn test_commit_message_data() {
    use gstats::scanner::messages::{MessageData, ScanMessage, MessageHeader};
    
    let header = MessageHeader {
        scan_mode: ScanMode::HISTORY,
        timestamp: 1234567890,
    };
    
    let commit_data = MessageData::CommitInfo {
        hash: "abc123def456".to_string(),
        author: "test@example.com".to_string(),
        message: "Fix bug in scanner".to_string(),
        timestamp: 1640995200, // Unix timestamp
        changed_files: vec![],
    };
    
    let message = ScanMessage {
        header,
        data: commit_data,
    };
    
    match message.data {
        MessageData::CommitInfo { hash, author, message: msg, timestamp, changed_files } => {
            assert_eq!(hash, "abc123def456");
            assert_eq!(author, "test@example.com");
            assert_eq!(msg, "Fix bug in scanner");
            assert_eq!(timestamp, 1640995200);
            assert_eq!(changed_files.len(), 0);
        }
        _ => panic!("Expected CommitInfo data"),
    }
}

#[test]
fn test_metric_message_data() {
    use gstats::scanner::messages::{MessageData, ScanMessage, MessageHeader};
    
    let header = MessageHeader {
        scan_mode: ScanMode::METRICS,
        timestamp: 1234567890,
    };
    
    let metric_data = MessageData::MetricInfo {
        file_count: 157,
        line_count: 5432,
        complexity: 2.5,
    };
    
    let message = ScanMessage {
        header,
        data: metric_data,
    };
    
    match message.data {
        MessageData::MetricInfo { file_count, line_count, complexity } => {
            assert_eq!(file_count, 157);
            assert_eq!(line_count, 5432);
            assert_eq!(complexity, 2.5);
        }
        _ => panic!("Expected MetricInfo data"),
    }
}

#[test]
fn test_scanner_config_builder() {
    // Test configuration builder patterns if available
    let config = ScannerConfig::default();
    
    // Verify sensible defaults
    assert!(config.max_memory_bytes >= 64 * 1024 * 1024); // At least 64MB
    assert!(config.queue_size >= 1000); // Reasonable queue size
    // max_threads is Option<usize>, can be None  
    if let Some(threads) = config.max_threads {
        assert!(threads >= 1); // At least one thread if set
    }
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
}

#[test]
fn test_message_data_variants() {
    use gstats::scanner::messages::MessageData;
    
    // Test None variant
    let none_data = MessageData::None;
    match none_data {
        MessageData::None => {}, // Expected
        _ => panic!("Expected None variant"),
    }
    
    // Test that all major variants can be created
    let _file_info = MessageData::FileInfo {
        path: "test.rs".to_string(),
        size: 100,
        lines: 10,
    };
    
    let _commit_info = MessageData::CommitInfo {
        hash: "123abc".to_string(),
        author: "dev@test.com".to_string(),
        message: "Test".to_string(),
        timestamp: 1234567890,
        changed_files: vec![],
    };
    
    let _metric_info = MessageData::MetricInfo {
        file_count: 1,
        line_count: 10,
        complexity: 1.0,
    };
}