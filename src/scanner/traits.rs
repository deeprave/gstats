//! Core Scanner Traits
//! 
//! Trait definitions for scanner components.

use crate::scanner::messages::ScanMessage;

/// Message producer trait for queue integration
#[async_trait::async_trait]
pub trait MessageProducer: Send + Sync {
    /// Produce a scan message to the queue
    async fn produce_message(&self, message: ScanMessage) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    
    /// Get the name of this producer
    fn get_producer_name(&self) -> &str;
}

/// Queue-based message producer that writes messages to the SharedMessageQueue
pub struct QueueMessageProducer {
    queue: crate::queue::SharedMessageQueue,
    name: String,
}

impl QueueMessageProducer {
    /// Create a new queue-based message producer
    pub fn new(queue: crate::queue::SharedMessageQueue, name: String) -> Self {
        Self { queue, name }
    }
}

#[async_trait::async_trait]
impl MessageProducer for QueueMessageProducer {
    async fn produce_message(&self, message: ScanMessage) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let sequence = self.queue.enqueue(message).await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        
        log::trace!("Message {} produced by {} to queue", sequence, self.name);
        Ok(())
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
