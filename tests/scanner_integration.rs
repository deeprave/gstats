//! Integration tests for the complete scanner API
//! 
//! These tests verify that all scanner components work together correctly
//! and demonstrate proper API usage patterns.

use gstats::scanner::{
    self, ScannerConfig, QueryBuilder, QueryParams, DateRange, FilePathFilter, AuthorFilter,
    VersionCompatible, ModeInfo,
    get_api_version, get_version_info, is_api_compatible,
    get_supported_modes,
};
use std::path::PathBuf;
use std::time::{SystemTime, Duration};

#[test]
fn test_scanner_api_version_compatibility() {
    // Test API version functions
    let version = get_api_version();
    assert!(version > 20250101); // Should be after Jan 1, 2025
    assert!(version < 20300101); // Sanity check - not in far future
    
    // Test compatibility checking
    assert!(is_api_compatible(version));
    assert!(is_api_compatible(version - 1)); // Should be compatible with recent versions
    assert!(!is_api_compatible(version + 1000)); // Future versions not compatible
    
    // Test version info
    let info = get_version_info();
    assert!(info.contains("api_version"));
    assert!(info.contains("release_date"));
}

#[test]
fn test_scanner_configuration_system() {
    // Test default configuration
    let default_config = ScannerConfig::default();
    assert!(default_config.max_memory_bytes > 0);
    assert!(default_config.queue_size > 0);
    
    // Test configuration builder
    let config = ScannerConfig::builder()
        .max_threads(4)
        .chunk_size(1000)
        .buffer_size(8192)
        .performance_mode(true)
        .with_max_memory(128 * 1024 * 1024)
        .with_queue_size(2000)
        .build()
        .expect("Failed to build scanner config");
    
    assert_eq!(config.max_memory_bytes, 128 * 1024 * 1024);
    assert_eq!(config.queue_size, 2000);
}

#[test]
fn test_query_builder_api() {
    let yesterday = SystemTime::now() - Duration::from_secs(86400);
    let tomorrow = SystemTime::now() + Duration::from_secs(86400);
    
    // Test comprehensive query building
    let query = QueryBuilder::new()
        .since(yesterday)
        .until(tomorrow)
        .include_path("src/")
        .exclude_path("target/")
        .author("alice@example.com")
        .exclude_author("bot@example.com")
        .limit(100)
        .build()
        .expect("Failed to build query");
    
    // Verify query parameters
    assert!(query.date_range.is_some());
    let date_range = query.date_range.as_ref().unwrap();
    assert!(date_range.start.is_some());
    assert!(date_range.end.is_some());
    
    // Check file path filters
    assert_eq!(query.file_paths.include.len(), 1);
    assert_eq!(query.file_paths.exclude.len(), 1);
    
    // Check author filters
    assert_eq!(query.authors.include.len(), 1);
    assert_eq!(query.authors.exclude.len(), 1);
    
    assert_eq!(query.limit, Some(100));
}

#[test]
fn test_scanning_modes_discovery() {
    // Test mode discovery API
    let modes = get_supported_modes();
    
    // Should have at least the None mode
    assert!(!modes.is_empty());
    
    // Verify mode information structure
    for mode in modes {
        assert!(!mode.name.is_empty());
        assert!(!mode.description.is_empty());
        assert!(mode.flag_value >= 0);
    }
}

#[test]
fn test_filter_composition_and_chaining() {
    use gstats::scanner::{DateFilter, PathFilter, CommitData};
    use gstats::scanner::filters::ScanFilter;
    use std::ops::ControlFlow;
    
    // Create a date filter
    let date_range = DateRange {
        start: Some(SystemTime::now() - Duration::from_secs(86400 * 30)), // 30 days ago
        end: Some(SystemTime::now()),
    };
    let date_filter = DateFilter::new(date_range);
    
    // Create a path filter
    let path_filter_params = FilePathFilter {
        include: vec![PathBuf::from("src/")],
        exclude: vec![PathBuf::from("target/")],
    };
    let path_filter = PathFilter::new(path_filter_params);
    
    // Test filter composition with CommitData
    let test_data = CommitData {
        timestamp: SystemTime::now() - Duration::from_secs(86400), // 1 day ago
        author: "alice@example.com".to_string(),
        file_paths: vec!["src/main.rs".to_string()],
        message: "Test commit".to_string(),
    };
    
    // Both filters should pass for this data
    match date_filter.apply(&test_data) {
        ControlFlow::Continue(()) => {},
        ControlFlow::Break(()) => panic!("Date filter should pass"),
    }
    
    match path_filter.apply(&test_data) {
        ControlFlow::Continue(()) => {},
        ControlFlow::Break(()) => panic!("Path filter should pass"),
    }
}

#[test]
fn test_scanner_convenience_functions() {
    // Test quick scan function (to be implemented)
    let params = QueryParams::default();
    let config = ScannerConfig::default();
    
    // Verify we can create scanner components
    // (Actual implementation would perform scanning)
    assert!(scanner::validate_query_params(&params).is_ok());
    assert!(scanner::validate_config(&config).is_ok());
}

#[test]
fn test_message_serialization_integration() {
    use gstats::scanner::messages::{MessageHeader, ScanMessage, MessageData};
    use gstats::scanner::ScanMode;
    
    // Create a message header
    let header = MessageHeader {
        scan_mode: ScanMode::empty(),
        timestamp: 1234567890,
    };
    
    // Create scan message
    let message = ScanMessage {
        header,
        data: MessageData::None,
    };
    
    // Test message properties
    assert_eq!(message.header.scan_mode, ScanMode::empty());
    assert_eq!(message.header.timestamp, 1234567890);
}

#[test]
fn test_trait_based_scanner_api() {
    // Mock scanner implementation for testing traits
    struct MockScanner {
        version: i64,
    }
    
    impl VersionCompatible for MockScanner {
        fn is_compatible(&self, required_version: i64) -> bool {
            required_version >= self.version
        }
        
        fn get_component_version(&self) -> i64 {
            self.version
        }
    }
    
    // Test trait usage
    let scanner = MockScanner { version: get_api_version() - 5 };
    assert!(scanner.is_compatible(get_api_version()));
    assert!(!scanner.is_compatible(get_api_version() - 10));
}

#[test]
fn test_end_to_end_api_usage() {
    // This test demonstrates the complete API usage pattern
    
    // 1. Check API compatibility
    let current_version = get_api_version();
    assert!(is_api_compatible(current_version));
    
    // 2. Create scanner configuration
    let config = ScannerConfig::builder()
        .max_threads(2)
        .performance_mode(false)
        .build()
        .expect("Failed to build config");
    
    // 3. Build query parameters
    let query = QueryBuilder::new()
        .since(SystemTime::now() - Duration::from_secs(86400 * 7)) // Last week
        .include_path("src/")
        .limit(50)
        .build()
        .expect("Failed to build query");
    
    // 4. Get supported scanning modes
    let modes = get_supported_modes();
    assert!(!modes.is_empty());
    
    // 5. Validate configuration and query
    assert!(scanner::validate_config(&config).is_ok());
    assert!(scanner::validate_query_params(&query).is_ok());
    
    // The actual scanning would happen here in a real implementation
    // For now, we're just testing that all the APIs work together
}