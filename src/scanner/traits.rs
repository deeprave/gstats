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

}
