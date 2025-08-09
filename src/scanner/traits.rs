//! Core Scanner Traits
//! 
//! Trait definitions for scanner components.

use crate::scanner::messages::ScanMessage;

/// Message producer trait for queue integration
pub trait MessageProducer {
    /// Produce a scan message
    fn produce_message(&self, message: ScanMessage);
    
    /// Get the name of this producer
    fn get_producer_name(&self) -> &str;
}

/// Simple callback-based message producer that bypasses queues
pub struct CallbackMessageProducer {
    name: String,
}

impl CallbackMessageProducer {
    /// Create a new callback-based message producer
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

impl MessageProducer for CallbackMessageProducer {
    fn produce_message(&self, _message: ScanMessage) {
        // Messages are handled directly via plugin callbacks, so this is a no-op
        log::debug!("Message produced by {} (handled via plugin callbacks)", self.name);
    }
    
    fn get_producer_name(&self) -> &str {
        &self.name
    }
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
