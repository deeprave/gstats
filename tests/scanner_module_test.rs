// Integration tests for scanner module infrastructure

#[test]
fn test_scanner_module_exists() {
    // This test verifies the scanner module can be imported
    // This should fail initially (RED phase)
    let _version = gstats::scanner::version::get_api_version();
}

#[test]
fn test_scanner_submodules_accessible() {
    // This test verifies all scanner submodules are accessible
    // This should fail initially (RED phase)
    
    // Test that we can access the main scanner components
    let _modes = gstats::scanner::modes::get_supported_modes();
    let _config = gstats::scanner::config::ScannerConfig::default();
    
    // Verify module structure is properly organised
    assert!(true); // Placeholder - will be replaced with actual tests
}

#[test]
fn test_scanner_module_documentation() {
    // This test verifies scanner module documentation exists
    // For now, just test that we can access the modules
    use gstats::scanner::traits::Scanner;
    use gstats::scanner::messages::ScanMessage;
    
    // Test that the types exist (compilation check)
    let _ = std::any::type_name::<ScanMessage>();
    let _ = std::any::TypeId::of::<dyn Scanner>();
    assert!(true);
}

// TDD RED Phase: GS-24 Step 2 - API Versioning System Tests
#[test]
fn test_api_versioning_system() {
    // Test that API version follows date-based i64 format (days since epoch)
    let api_version = gstats::scanner::get_api_version();
    
    // API version should be positive
    assert!(api_version > 0, "API version should be positive");
    
    // API version should be in YYYYMMDD format (reasonable date range)
    let min_version = 20200101; // 2020-01-01
    let max_version = 20301231; // 2030-12-31
    assert!(api_version >= min_version, "API version should be after 2020-01-01");
    assert!(api_version <= max_version, "API version should not be too far in future");
}

#[test]
fn test_api_version_compatibility() {
    // Test version compatibility checking functions
    let current_version = gstats::scanner::get_api_version();
    
    // Test self-compatibility
    assert!(gstats::scanner::is_compatible_version(current_version), 
            "Current API version should be compatible with itself");
    
    // Test backward compatibility within reasonable range
    let yesterday = current_version - 1;
    assert!(gstats::scanner::is_compatible_version(yesterday),
            "API should maintain backward compatibility for recent versions");
}

#[test]
fn test_api_version_metadata() {
    // Test that version metadata provides useful information
    let version_info = gstats::scanner::get_version_info();
    
    // Version info should contain release date
    assert!(version_info.contains("release_date"), 
            "Version info should contain release date");
    
    // Version info should contain compatibility information
    assert!(version_info.contains("compatibility"), 
            "Version info should contain compatibility information");
}

// TDD RED Phase: GS-24 Step 3 - Scanning Modes & Bitflags Tests
#[test]
fn test_scan_modes_bitflags() {
    // Test that scan modes can be combined using bitwise operations
    use gstats::scanner::modes::ScanMode;
    
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
}

#[test]
fn test_scan_modes_discovery() {
    // Test that scanning modes can be discovered for plugin compatibility
    let supported_modes = gstats::scanner::modes::get_supported_modes();
    
    // Should contain basic scanning modes
    let files_mode = supported_modes.iter().find(|m| m.flag_value == gstats::scanner::ScanMode::FILES.bits());
    assert!(files_mode.is_some(), "Should support FILES scanning");
    
    let history_mode = supported_modes.iter().find(|m| m.flag_value == gstats::scanner::ScanMode::HISTORY.bits());
    assert!(history_mode.is_some(), "Should support HISTORY scanning");
    
    // Should be non-empty
    assert!(!supported_modes.is_empty(), "Should have at least one supported mode");
}

#[test]
fn test_scan_modes_validation() {
    // Test mode validation functions
    use gstats::scanner::modes::ScanMode;
    
    let valid_mode = ScanMode::FILES;
    let invalid_empty = ScanMode::empty();
    
    // Test validation
    assert!(gstats::scanner::modes::is_valid_mode(valid_mode), 
            "FILES mode should be valid");
    assert!(!gstats::scanner::modes::is_valid_mode(invalid_empty), 
            "Empty mode should be invalid");
    
    // Test mode descriptions
    let description = gstats::scanner::modes::get_mode_description(ScanMode::FILES);
    assert!(!description.is_empty(), "Mode description should not be empty");
    assert!(description.to_lowercase().contains("file"), 
            "FILES mode description should mention files");
}

// TDD RED Phase: GS-24 Step 4 - Message Structures Tests
#[test]
fn test_scan_message_creation() {
    // Test that scan messages can be created with proper structure
    use gstats::scanner::messages::{ScanMessage, MessageHeader, MessageData};
    use gstats::scanner::modes::ScanMode;
    
    let header = MessageHeader::new(ScanMode::FILES, 12345);
    let data = MessageData::FileInfo {
        path: "src/main.rs".to_string(),
        size: 1024,
        lines: 50,
    };
    
    let message = ScanMessage::new(header, data);
    
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
fn test_message_serialization() {
    // Test that messages can be serialized/deserialized for queue transmission
    use gstats::scanner::messages::{ScanMessage, MessageHeader, MessageData};
    use gstats::scanner::modes::ScanMode;
    
    let original = ScanMessage::new(
        MessageHeader::new(ScanMode::HISTORY, 67890),
        MessageData::CommitInfo {
            hash: "abc123".to_string(),
            author: "developer".to_string(),
            message: "Fix bug".to_string(),
        }
    );
    
    // Test serialization to bytes
    let serialized = original.to_bytes();
    assert!(!serialized.is_empty(), "Serialized message should not be empty");
    
    // Test deserialization
    let deserialized = ScanMessage::from_bytes(&serialized)
        .expect("Should deserialize successfully");
    
    // Verify round-trip integrity
    assert_eq!(deserialized.header.scan_mode, original.header.scan_mode);
    assert_eq!(deserialized.header.timestamp, original.header.timestamp);
}

#[test]
fn test_message_memory_efficiency() {
    // Test that messages use compact representation for memory efficiency
    use gstats::scanner::messages::{ScanMessage, MessageHeader, MessageData};
    use gstats::scanner::modes::ScanMode;
    
    let message = ScanMessage::new(
        MessageHeader::new(ScanMode::METRICS, 11111),
        MessageData::MetricInfo {
            file_count: 42,
            line_count: 1337,
            complexity: 3.14,
        }
    );
    
    // Test memory footprint is reasonable
    let size = std::mem::size_of_val(&message);
    assert!(size < 1024, "Message should be under 1KB: {} bytes", size);
    
    // Test that header is compact
    let header_size = std::mem::size_of_val(&message.header);
    assert!(header_size < 64, "Header should be under 64 bytes: {} bytes", header_size);
}

// TDD RED Phase: GS-24 Step 5 - Configuration System Tests
#[test]
fn test_scanner_config_creation() {
    // Test that scanner configuration can be created with sensible defaults
    use gstats::scanner::config::ScannerConfig;
    
    let config = ScannerConfig::default();
    
    // Test default values are reasonable
    assert!(config.max_memory_bytes > 0, "Max memory should be positive");
    assert!(config.queue_size > 0, "Queue size should be positive");
    assert!(config.max_memory_bytes >= 1024 * 1024, "Should have at least 1MB default memory");
    assert!(config.queue_size >= 100, "Should have reasonable queue size");
}

#[test]
fn test_scanner_config_validation() {
    // Test configuration validation
    use gstats::scanner::config::ScannerConfig;
    
    let mut config = ScannerConfig::default();
    
    // Test valid configuration
    assert!(config.validate().is_ok(), "Default config should be valid");
    
    // Test invalid configurations
    config.max_memory_bytes = 0;
    assert!(config.validate().is_err(), "Zero memory should be invalid");
    
    config.max_memory_bytes = 1024 * 1024; // Reset to valid
    config.queue_size = 0;
    assert!(config.validate().is_err(), "Zero queue size should be invalid");
}

#[test]
fn test_scanner_config_customization() {
    // Test that configuration can be customized
    use gstats::scanner::config::ScannerConfig;
    
    let custom_config = ScannerConfig::new()
        .with_max_memory(128 * 1024 * 1024) // 128MB
        .with_queue_size(2000)
        .build()
        .expect("Failed to build custom config");
    
    assert_eq!(custom_config.max_memory_bytes, 128 * 1024 * 1024);
    assert_eq!(custom_config.queue_size, 2000);
    
    // Test that custom config is still valid
    assert!(custom_config.validate().is_ok(), "Custom config should be valid");
}

// TDD RED Phase: GS-24 Step 6 - Core Traits Definition Tests
#[test]
fn test_scanner_trait_interface() {
    // Test that scanner trait provides the core scanning interface
    use gstats::scanner::traits::Scanner;
    use gstats::scanner::modes::ScanMode;
    
    // Create a mock scanner implementation
    struct MockScanner;
    
    impl Scanner for MockScanner {
        fn scan(&self, modes: ScanMode) -> Result<(), Box<dyn std::error::Error>> {
            if modes.is_empty() {
                return Err("Cannot scan with empty modes".into());
            }
            Ok(())
        }
        
        fn get_name(&self) -> &str {
            "MockScanner"
        }
        
        fn supports_mode(&self, mode: ScanMode) -> bool {
            matches!(mode, ScanMode::FILES | ScanMode::HISTORY)
        }
    }
    
    let scanner = MockScanner;
    
    // Test scanner interface
    assert_eq!(scanner.get_name(), "MockScanner");
    assert!(scanner.supports_mode(ScanMode::FILES));
    assert!(!scanner.supports_mode(ScanMode::SECURITY));
    
    // Test scanning
    assert!(scanner.scan(ScanMode::FILES).is_ok());
    assert!(scanner.scan(ScanMode::empty()).is_err());
}

#[test]
fn test_scan_processor_trait() {
    // Test scan processor trait for handling scan results
    use gstats::scanner::traits::ScanProcessor;
    use gstats::scanner::messages::{ScanMessage, MessageHeader, MessageData};
    use gstats::scanner::modes::ScanMode;
    
    struct MockProcessor {
        processed_count: std::cell::RefCell<usize>,
    }
    
    impl ScanProcessor for MockProcessor {
        fn process_message(&self, _message: &ScanMessage) -> Result<(), Box<dyn std::error::Error>> {
            *self.processed_count.borrow_mut() += 1;
            Ok(())
        }
        
        fn get_processed_count(&self) -> usize {
            *self.processed_count.borrow()
        }
        
        fn reset(&self) {
            *self.processed_count.borrow_mut() = 0;
        }
    }
    
    let processor = MockProcessor {
        processed_count: std::cell::RefCell::new(0),
    };
    
    // Test processor interface
    assert_eq!(processor.get_processed_count(), 0);
    
    let message = ScanMessage::new(
        MessageHeader::new(ScanMode::FILES, 12345),
        MessageData::FileInfo {
            path: "test.rs".to_string(),
            size: 1024,
            lines: 50,
        }
    );
    
    assert!(processor.process_message(&message).is_ok());
    assert_eq!(processor.get_processed_count(), 1);
    
    processor.reset();
    assert_eq!(processor.get_processed_count(), 0);
}

#[test]
fn test_scan_filter_trait() {
    // Test scan filter trait for filtering scan results
    use gstats::scanner::traits::ScanFilter;
    use gstats::scanner::messages::{ScanMessage, MessageHeader, MessageData};
    use gstats::scanner::modes::ScanMode;
    
    struct FileSizeFilter {
        max_size: u64,
    }
    
    impl ScanFilter for FileSizeFilter {
        fn should_include(&self, message: &ScanMessage) -> bool {
            match &message.data {
                MessageData::FileInfo { size, .. } => *size <= self.max_size,
                _ => true, // Include non-file messages
            }
        }
        
        fn get_filter_name(&self) -> &str {
            "FileSizeFilter"
        }
    }
    
    let filter = FileSizeFilter { max_size: 2048 };
    
    // Test filter interface
    assert_eq!(filter.get_filter_name(), "FileSizeFilter");
    
    // Test small file (should be included)
    let small_file = ScanMessage::new(
        MessageHeader::new(ScanMode::FILES, 12345),
        MessageData::FileInfo {
            path: "small.rs".to_string(),
            size: 1024,
            lines: 50,
        }
    );
    assert!(filter.should_include(&small_file));
    
    // Test large file (should be filtered out)
    let large_file = ScanMessage::new(
        MessageHeader::new(ScanMode::FILES, 12345),
        MessageData::FileInfo {
            path: "large.rs".to_string(),
            size: 4096,
            lines: 200,
        }
    );
    assert!(!filter.should_include(&large_file));
    
    // Test non-file message (should be included)
    let commit_message = ScanMessage::new(
        MessageHeader::new(ScanMode::HISTORY, 12345),
        MessageData::CommitInfo {
            hash: "abc123".to_string(),
            author: "dev".to_string(),
            message: "Fix".to_string(),
        }
    );
    assert!(filter.should_include(&commit_message));
}
