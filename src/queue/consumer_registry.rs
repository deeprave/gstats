//! Consumer Registry Management
//!
//! This module manages consumer registration, progress tracking, and lifecycle
//! for the multi-consumer queue system. It handles multiple consumers consuming
//! from the same queue with independent progress tracking.

use std::collections::HashMap;
use std::time::{Duration, Instant};
use crate::queue::error::{QueueError, QueueResult};

/// Registry for tracking active consumers and their progress
#[derive(Debug)]
pub struct ConsumerRegistry {
    /// Active consumers and their progress
    pub(crate) consumers: HashMap<String, ConsumerProgress>,
}

/// Progress tracking for individual consumers
#[derive(Debug, Clone)]
pub struct ConsumerProgress {
    /// Last acknowledged sequence number
    pub last_acknowledged_seq: u64,
    
    /// Number of messages processed
    pub messages_processed: u64,
    
    /// Last update timestamp
    pub last_update: Instant,
    
    /// Consumer creation time
    pub created_at: Instant,
    
    /// Average processing rate (messages/second)
    pub processing_rate: f64,
}

impl ConsumerRegistry {
    /// Create a new consumer registry
    pub fn new(_timeout: Duration) -> Self {
        Self {
            consumers: HashMap::new(),
        }
    }
    
    /// Register a new consumer
    pub fn register_consumer(&mut self, consumer_id: String, _plugin_name: String, _priority: i32) -> QueueResult<()> {
        if self.consumers.contains_key(&consumer_id) {
            return Err(QueueError::operation_failed(
                format!("Consumer {} already registered", consumer_id)
            ));
        }
        
        let now = Instant::now();
        let progress = ConsumerProgress {
            last_acknowledged_seq: 0,
            messages_processed: 0,
            last_update: now,
            created_at: now,
            processing_rate: 0.0,
        };
        
        self.consumers.insert(consumer_id, progress);
        Ok(())
    }
    
    /// Deregister a consumer
    pub fn deregister_consumer(&mut self, consumer_id: &str) -> QueueResult<()> {
        if self.consumers.remove(consumer_id).is_none() {
            return Err(QueueError::operation_failed(
                format!("Consumer {} not found", consumer_id)
            ));
        }
        Ok(())
    }
    
    /// Update consumer progress
    pub fn update_progress(&mut self, consumer_id: &str, acknowledged_seq: u64) -> QueueResult<()> {
        if let Some(progress) = self.consumers.get_mut(consumer_id) {
            progress.last_acknowledged_seq = acknowledged_seq;
            progress.messages_processed += 1;
            progress.last_update = Instant::now();
            
            // Update processing rate
            let elapsed = progress.created_at.elapsed().as_secs_f64();
            if elapsed > 0.0 {
                progress.processing_rate = progress.messages_processed as f64 / elapsed;
            }
            
            Ok(())
        } else {
            Err(QueueError::operation_failed(
                format!("Consumer {} not found", consumer_id)
            ))
        }
    }
    
    /// Get all consumer progress
    pub fn get_all_progress(&self) -> Vec<&ConsumerProgress> {
        self.consumers.values().collect()
    }
    
    /// Get specific consumer progress
    pub fn get_progress(&self, consumer_id: &str) -> Option<&ConsumerProgress> {
        self.consumers.get(consumer_id)
    }
    
    /// Get the number of active consumers
    pub fn active_count(&self) -> usize {
        self.consumers.len()
    }
    
    /// Get all consumer IDs
    pub fn get_consumer_ids(&self) -> Vec<String> {
        self.consumers.keys().cloned().collect()
    }
    
    /// Find the minimum sequence number across all consumers (for garbage collection)
    pub fn get_min_sequence(&self) -> u64 {
        self.consumers
            .values()
            .map(|progress| progress.last_acknowledged_seq)
            .min()
            .unwrap_or(0)
    }
    
    /// Get consumers that haven't updated within the timeout period
    pub fn get_stale_consumers(&self, timeout: Duration) -> Vec<String> {
        let now = Instant::now();
        self.consumers
            .iter()
            .filter_map(|(id, progress)| {
                if now.duration_since(progress.last_update) > timeout {
                    Some(id.clone())
                } else {
                    None
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_consumer_registry_creation() {
        let registry = ConsumerRegistry::new(Duration::from_secs(60));
        assert_eq!(registry.active_count(), 0);
        assert!(registry.get_consumer_ids().is_empty());
    }

    #[test]
    fn test_consumer_registration() {
        let mut registry = ConsumerRegistry::new(Duration::from_secs(60));
        
        // Register a consumer
        registry.register_consumer(
            "test-consumer".to_string(),
            "test-plugin".to_string(),
            1
        ).unwrap();
        
        assert_eq!(registry.active_count(), 1);
        assert!(registry.get_progress("test-consumer").is_some());
        
        // Try to register the same consumer again (should fail)
        let result = registry.register_consumer(
            "test-consumer".to_string(),
            "test-plugin".to_string(),
            1
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_consumer_deregistration() {
        let mut registry = ConsumerRegistry::new(Duration::from_secs(60));
        
        // Register and then deregister
        registry.register_consumer(
            "test-consumer".to_string(),
            "test-plugin".to_string(),
            1
        ).unwrap();
        
        assert_eq!(registry.active_count(), 1);
        
        registry.deregister_consumer("test-consumer").unwrap();
        assert_eq!(registry.active_count(), 0);
        
        // Try to deregister non-existent consumer
        let result = registry.deregister_consumer("non-existent");
        assert!(result.is_err());
    }

    #[test]
    fn test_progress_update() {
        let mut registry = ConsumerRegistry::new(Duration::from_secs(60));
        
        registry.register_consumer(
            "test-consumer".to_string(),
            "test-plugin".to_string(),
            1
        ).unwrap();
        
        // Update progress
        registry.update_progress("test-consumer", 100).unwrap();
        
        let progress = registry.get_progress("test-consumer").unwrap();
        assert_eq!(progress.last_acknowledged_seq, 100);
        assert_eq!(progress.messages_processed, 1);
        
        // Update again
        registry.update_progress("test-consumer", 200).unwrap();
        
        let progress = registry.get_progress("test-consumer").unwrap();
        assert_eq!(progress.last_acknowledged_seq, 200);
        assert_eq!(progress.messages_processed, 2);
    }

    #[test]
    fn test_min_sequence_calculation() {
        let mut registry = ConsumerRegistry::new(Duration::from_secs(60));
        
        // Empty registry should return 0
        assert_eq!(registry.get_min_sequence(), 0);
        
        // Add consumers with different progress
        registry.register_consumer("consumer1".to_string(), "plugin1".to_string(), 1).unwrap();
        registry.register_consumer("consumer2".to_string(), "plugin2".to_string(), 1).unwrap();
        registry.register_consumer("consumer3".to_string(), "plugin3".to_string(), 1).unwrap();
        
        registry.update_progress("consumer1", 100).unwrap();
        registry.update_progress("consumer2", 50).unwrap();
        registry.update_progress("consumer3", 75).unwrap();
        
        // Minimum should be 50
        assert_eq!(registry.get_min_sequence(), 50);
    }
}