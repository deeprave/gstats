use crate::scanner::async_engine::events::RepositoryEvent;
use crate::scanner::async_engine::shared_state::{SharedProcessorState, RepositoryMetadata};
use crate::scanner::messages::ScanMessage;
use crate::scanner::async_engine::error::ScanError;
use crate::plugin::PluginResult;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use log::{debug, info, warn};

pub mod event_processor;
pub mod history;
pub mod files;
pub mod statistics;

// Re-export the generic EventProcessor trait and related types
pub use event_processor::{EventProcessor, ProcessorStats, ProcessorConfig, SharedStateAccess};

/// Processor factory for creating event processors
/// 
/// This factory creates processors for the scanner module. Plugin modules
/// will have their own processor factories.
pub struct ProcessorFactory;

impl ProcessorFactory {
    /// Create all available processors without mode filtering
    pub fn create_processors() -> Vec<Box<dyn EventProcessor>> {
        let mut processors: Vec<Box<dyn EventProcessor>> = Vec::new();

        // Include all processors - scanner now processes all data types
        processors.push(Box::new(statistics::StatisticsProcessor::new()));
        processors.push(Box::new(history::HistoryEventProcessor::new()));
        processors.push(Box::new(files::FileEventProcessor::new()));

        debug!("Created {} processors", processors.len());
        processors
    }

    /// Get all available processor names
    pub fn available_processors() -> Vec<&'static str> {
        vec![
            "statistics",
            "history", 
            "files",
            // Note: change_frequency moved to metrics plugin
        ]
    }

    /// Check if a processor is available
    pub fn is_available(processor_name: &str) -> bool {
        match processor_name {
            "statistics" | "history" | "files" => true,
            _ => false,
        }
    }
}

/// Registry for managing event processors
/// 
/// This registry can be used by both scanner and plugin modules to manage
/// their processors in a consistent way.
pub struct ProcessorRegistry {
    processors: HashMap<String, Box<dyn EventProcessor>>,
    shared_state: Option<Arc<SharedProcessorState>>,
}

impl ProcessorRegistry {
    /// Create a new processor registry
    pub fn new() -> Self {
        Self {
            processors: HashMap::new(),
            shared_state: None,
        }
    }

    /// Register a processor
    pub fn register(&mut self, processor: Box<dyn EventProcessor>) {
        let name = processor.name().to_string();
        debug!("Registering processor: {}", name);
        self.processors.insert(name, processor);
    }

    /// Set shared state for all processors
    pub fn set_shared_state(&mut self, shared_state: Arc<SharedProcessorState>) {
        self.shared_state = Some(shared_state.clone());
        for processor in self.processors.values_mut() {
            processor.set_shared_state(shared_state.clone());
        }
    }

    /// Initialize all processors
    pub async fn initialize_all(&mut self) -> PluginResult<()> {
        for (name, processor) in &mut self.processors {
            debug!("Initializing processor: {}", name);
            processor.initialize().await?;
        }
        Ok(())
    }

    /// Process an event with all relevant processors
    pub async fn process_event(&mut self, event: &RepositoryEvent) -> PluginResult<Vec<ScanMessage>> {
        let mut all_messages = Vec::new();

        for (name, processor) in &mut self.processors {
            if processor.should_process_event(event) {
                match processor.process_event(event).await {
                    Ok(mut messages) => {
                        debug!("Processor {} generated {} messages", name, messages.len());
                        all_messages.append(&mut messages);
                    }
                    Err(e) => {
                        warn!("Processor {} failed to process event: {}", name, e);
                        // Continue processing with other processors
                    }
                }
            }
        }

        Ok(all_messages)
    }

    /// Finalize all processors
    pub async fn finalize_all(&mut self) -> PluginResult<Vec<ScanMessage>> {
        let mut all_messages = Vec::new();

        for (name, processor) in &mut self.processors {
            debug!("Finalizing processor: {}", name);
            match processor.finalize().await {
                Ok(mut messages) => {
                    debug!("Processor {} generated {} final messages", name, messages.len());
                    all_messages.append(&mut messages);
                }
                Err(e) => {
                    warn!("Processor {} failed to finalize: {}", name, e);
                    // Continue finalizing other processors
                }
            }
        }

        Ok(all_messages)
    }

    /// Get statistics from all processors
    pub fn get_all_stats(&self) -> HashMap<String, ProcessorStats> {
        self.processors
            .iter()
            .map(|(name, processor)| (name.clone(), processor.get_stats()))
            .collect()
    }

    /// Get processor count
    pub fn processor_count(&self) -> usize {
        self.processors.len()
    }

    /// Get processor names
    pub fn processor_names(&self) -> Vec<String> {
        self.processors.keys().cloned().collect()
    }
}

impl Default for ProcessorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Event processing coordinator
/// 
/// Coordinates event processing across multiple processors with error handling
/// and performance monitoring.
pub struct EventProcessingCoordinator {
    registry: ProcessorRegistry,
    total_events_processed: usize,
    total_processing_time: Duration,
}

impl EventProcessingCoordinator {
    /// Create a new coordinator
    pub fn new() -> Self {
        Self {
            registry: ProcessorRegistry::new(),
            total_events_processed: 0,
            total_processing_time: Duration::default(),
        }
    }

    /// Add a processor to the coordinator
    pub fn add_processor(&mut self, processor: Box<dyn EventProcessor>) {
        self.registry.register(processor);
    }

    /// Set shared state for all processors
    pub fn set_shared_state(&mut self, shared_state: Arc<SharedProcessorState>) {
        self.registry.set_shared_state(shared_state);
    }

    /// Initialize all processors
    pub async fn initialize(&mut self) -> PluginResult<()> {
        info!("Initializing {} processors", self.registry.processor_count());
        self.registry.initialize_all().await
    }

    /// Process an event with timing and error handling
    pub async fn process_event(&mut self, event: &RepositoryEvent) -> PluginResult<Vec<ScanMessage>> {
        let start_time = std::time::Instant::now();
        
        let messages = self.registry.process_event(event).await?;
        
        self.total_events_processed += 1;
        self.total_processing_time += start_time.elapsed();
        
        Ok(messages)
    }

    /// Finalize all processors
    pub async fn finalize(&mut self) -> PluginResult<Vec<ScanMessage>> {
        info!("Finalizing {} processors", self.registry.processor_count());
        self.registry.finalize_all().await
    }

    /// Get comprehensive statistics
    pub fn get_stats(&self) -> CoordinatorStats {
        CoordinatorStats {
            total_events_processed: self.total_events_processed,
            total_processing_time: self.total_processing_time,
            processor_count: self.registry.processor_count(),
            processor_stats: self.registry.get_all_stats(),
        }
    }
}

impl Default for EventProcessingCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics for the event processing coordinator
#[derive(Debug, Clone)]
pub struct CoordinatorStats {
    pub total_events_processed: usize,
    pub total_processing_time: Duration,
    pub processor_count: usize,
    pub processor_stats: HashMap<String, ProcessorStats>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::async_engine::events::RepositoryEvent;

    #[tokio::test]
    async fn test_processor_factory() {
        let processors = ProcessorFactory::create_processors();
        
        // Should include all processors: statistics, files, and history
        assert_eq!(processors.len(), 3);
        
        let available = ProcessorFactory::available_processors();
        assert!(available.contains(&"statistics"));
        assert!(available.contains(&"history"));
        assert!(available.contains(&"files"));
    }

    #[tokio::test]
    async fn test_processor_factory_plugin_delegation() {
        // This test verifies that ProcessorFactory can delegate processor creation to plugins
        // For now, this is a placeholder test that will be implemented in the next phase
        
        // TODO: Implement plugin-delegated processor creation
        // The factory should:
        // 1. Query active plugins for their processors
        // 2. Collect processors from plugins based on scan modes
        // 3. Return combined processor list
        
        let processors = ProcessorFactory::create_processors();
        
        // For now, verify that basic scanner processors are still created
        assert_eq!(processors.len(), 3); // statistics, files, history
        
        let processor_names: Vec<&str> = processors.iter().map(|p| p.name()).collect();
        assert!(processor_names.contains(&"StatisticsProcessor"));
        assert!(processor_names.contains(&"files"));
        assert!(processor_names.contains(&"history"));
    }

    #[tokio::test]
    async fn test_processor_registry() {
        let mut registry = ProcessorRegistry::new();
        
        assert_eq!(registry.processor_count(), 0);
        
        // Add a processor
        let processor = Box::new(statistics::StatisticsProcessor::new());
        registry.register(processor);
        
        assert_eq!(registry.processor_count(), 1);
        assert!(registry.processor_names().contains(&"StatisticsProcessor".to_string()));
    }

    #[tokio::test]
    async fn test_event_processing_coordinator() {
        let mut coordinator = EventProcessingCoordinator::new();
        
        // Add a processor
        let processor = Box::new(statistics::StatisticsProcessor::new());
        coordinator.add_processor(processor);
        
        // Initialize
        coordinator.initialize().await.unwrap();
        
        // Process an event
        let event = RepositoryEvent::RepositoryStarted {
            total_commits: Some(10),
            total_files: Some(5),
        };
        
        let messages = coordinator.process_event(&event).await.unwrap();
        // Statistics processor doesn't generate messages during processing
        assert!(messages.is_empty());
        
        // Finalize
        let final_messages = coordinator.finalize().await.unwrap();
        // Statistics processor should generate a message during finalization
        assert!(!final_messages.is_empty());
        
        // Check stats
        let stats = coordinator.get_stats();
        assert_eq!(stats.total_events_processed, 1);
        assert_eq!(stats.processor_count, 1);
    }
}
