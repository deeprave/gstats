//! Core Scanner Traits
//! 
//! Trait definitions for scanner components.

use crate::scanner::messages::ScanMessage;
use crate::scanner::modes::ScanMode;

/// Main scanner interface
pub trait Scanner {
    /// Start scanning with specified modes
    fn scan(&self, modes: ScanMode) -> Result<(), Box<dyn std::error::Error>>;
    
    /// Get the name of this scanner implementation
    fn get_name(&self) -> &str;
    
    /// Check if this scanner supports the given mode
    fn supports_mode(&self, mode: ScanMode) -> bool;
}

/// Scan processor trait for handling scan results
pub trait ScanProcessor {
    /// Process a scan message
    fn process_message(&self, message: &ScanMessage) -> Result<(), Box<dyn std::error::Error>>;
    
    /// Get the number of processed messages
    fn get_processed_count(&self) -> usize;
    
    /// Reset the processor state
    fn reset(&self);
}

/// Scan filter trait for filtering scan results
pub trait ScanFilter {
    /// Determine if a message should be included in results
    fn should_include(&self, message: &ScanMessage) -> bool;
    
    /// Get the name of this filter
    fn get_filter_name(&self) -> &str;
}

/// Message producer trait for queue integration
pub trait MessageProducer {
    /// Produce a scan message
    fn produce_message(&self, message: ScanMessage);
    
    /// Get the name of this producer
    fn get_producer_name(&self) -> &str;
}

/// Version compatibility checking trait
pub trait VersionCompatible {
    /// Check if this component is compatible with a required API version
    fn is_compatible(&self, required_version: i64) -> bool;
    
    /// Get the API version this component was built for
    fn get_component_version(&self) -> i64;
}

/// Scan result aggregator trait
pub trait ScanAggregator {
    /// Aggregate multiple scan results
    fn aggregate(&self, messages: &[ScanMessage]) -> Result<ScanMessage, Box<dyn std::error::Error>>;
    
    /// Get the aggregation strategy name
    fn get_strategy_name(&self) -> &str;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::messages::{MessageHeader, MessageData};

    #[test]
    fn test_scanner_trait() {
        struct TestScanner;
        
        impl Scanner for TestScanner {
            fn scan(&self, _modes: ScanMode) -> Result<(), Box<dyn std::error::Error>> {
                Ok(())
            }
            
            fn get_name(&self) -> &str {
                "TestScanner"
            }
            
            fn supports_mode(&self, mode: ScanMode) -> bool {
                mode.contains(ScanMode::FILES)
            }
        }
        
        let scanner = TestScanner;
        assert_eq!(scanner.get_name(), "TestScanner");
        assert!(scanner.supports_mode(ScanMode::FILES));
        assert!(!scanner.supports_mode(ScanMode::SECURITY));
    }

    #[test]
    fn test_scan_filter_trait() {
        struct TestFilter;
        
        impl ScanFilter for TestFilter {
            fn should_include(&self, _message: &ScanMessage) -> bool {
                true
            }
            
            fn get_filter_name(&self) -> &str {
                "TestFilter"
            }
        }
        
        let filter = TestFilter;
        assert_eq!(filter.get_filter_name(), "TestFilter");
        
        let message = ScanMessage::new(
            MessageHeader::new(ScanMode::FILES, 12345),
            MessageData::FileInfo {
                path: "test.rs".to_string(),
                size: 1024,
                lines: 50,
            }
        );
        assert!(filter.should_include(&message));
    }

    #[test]
    fn test_version_compatible_trait() {
        struct TestComponent;
        
        impl VersionCompatible for TestComponent {
            fn is_compatible(&self, required_version: i64) -> bool {
                required_version <= self.get_component_version()
            }
            
            fn get_component_version(&self) -> i64 {
                20000 // Mock version
            }
        }
        
        let component = TestComponent;
        assert_eq!(component.get_component_version(), 20000);
        assert!(component.is_compatible(19000));
        assert!(!component.is_compatible(21000));
    }
}
