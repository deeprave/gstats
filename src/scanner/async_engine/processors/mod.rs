use crate::scanner::async_engine::events::RepositoryEvent;
use crate::scanner::messages::ScanMessage;
use crate::scanner::modes::ScanMode;
use crate::scanner::async_engine::error::ScanError;
use crate::plugin::PluginResult;
use async_trait::async_trait;
use std::collections::HashMap;
use log::{debug, info, warn};

pub mod history;
pub mod change_frequency;
pub mod files;

/// Trait for processing repository events and generating scan messages
#[async_trait]
pub trait EventProcessor: Send + Sync {
    /// Get the scan modes this processor supports
    fn supported_modes(&self) -> ScanMode;

    /// Get a unique name for this processor
    fn name(&self) -> &'static str;

    /// Initialize the processor before event processing begins
    async fn initialize(&mut self) -> PluginResult<()> {
        Ok(())
    }

    /// Process a single repository event and generate scan messages
    async fn process_event(&mut self, event: &RepositoryEvent) -> PluginResult<Vec<ScanMessage>>;

    /// Finalize processing and generate any accumulated results
    async fn finalize(&mut self) -> PluginResult<Vec<ScanMessage>> {
        Ok(vec![])
    }

    /// Get processing statistics (optional)
    fn get_stats(&self) -> ProcessorStats {
        ProcessorStats::default()
    }

    /// Check if this processor should handle the given event
    fn should_process_event(&self, event: &RepositoryEvent) -> bool {
        event.is_relevant_for_modes(self.supported_modes())
    }
}

/// Statistics for event processor performance
#[derive(Debug, Clone, Default)]
pub struct ProcessorStats {
    pub events_processed: usize,
    pub messages_generated: usize,
    pub processing_time: std::time::Duration,
    pub errors_encountered: usize,
}

/// Registry for managing event processors
pub struct ProcessorRegistry {
    processors: HashMap<String, Box<dyn EventProcessor>>,
    active_modes: ScanMode,
}

impl ProcessorRegistry {
    /// Create a new processor registry
    pub fn new(modes: ScanMode) -> Self {
        Self {
            processors: HashMap::new(),
            active_modes: modes,
        }
    }

    /// Register a processor
    pub fn register_processor(&mut self, processor: Box<dyn EventProcessor>) {
        let name = processor.name().to_string();
        let supported_modes = processor.supported_modes();
        
        debug!("Registering processor '{}' with modes: {:?}", name, supported_modes);
        
        // Only register if the processor supports any of the active modes
        if supported_modes.intersects(self.active_modes) {
            self.processors.insert(name.clone(), processor);
            info!("Registered processor '{}' for active modes", name);
        } else {
            debug!("Skipping processor '{}' - no matching active modes", name);
        }
    }

    /// Get all registered processors
    pub fn get_processors(&self) -> &HashMap<String, Box<dyn EventProcessor>> {
        &self.processors
    }

    /// Get mutable access to processors
    pub fn get_processors_mut(&mut self) -> &mut HashMap<String, Box<dyn EventProcessor>> {
        &mut self.processors
    }

    /// Initialize all registered processors
    pub async fn initialize_all(&mut self) -> PluginResult<()> {
        for (name, processor) in &mut self.processors {
            debug!("Initializing processor: {}", name);
            if let Err(e) = processor.initialize().await {
                warn!("Failed to initialize processor '{}': {}", name, e);
                return Err(e);
            }
        }
        info!("Initialized {} processors", self.processors.len());
        Ok(())
    }

    /// Process an event through all relevant processors
    pub async fn process_event(&mut self, event: &RepositoryEvent) -> PluginResult<Vec<ScanMessage>> {
        let mut all_messages = Vec::new();

        for (name, processor) in &mut self.processors {
            if processor.should_process_event(event) {
                debug!("Processing event {:?} with processor '{}'", event.event_type(), name);
                
                match processor.process_event(event).await {
                    Ok(messages) => {
                        debug!("Processor '{}' generated {} messages", name, messages.len());
                        all_messages.extend(messages);
                    }
                    Err(e) => {
                        warn!("Processor '{}' failed to process event: {}", name, e);
                        // Continue with other processors rather than failing completely
                    }
                }
            }
        }

        Ok(all_messages)
    }

    /// Finalize all processors and collect final messages
    pub async fn finalize_all(&mut self) -> PluginResult<Vec<ScanMessage>> {
        let mut all_messages = Vec::new();

        for (name, processor) in &mut self.processors {
            debug!("Finalizing processor: {}", name);
            
            match processor.finalize().await {
                Ok(messages) => {
                    debug!("Processor '{}' finalized with {} messages", name, messages.len());
                    all_messages.extend(messages);
                }
                Err(e) => {
                    warn!("Processor '{}' failed to finalize: {}", name, e);
                    // Continue with other processors
                }
            }
        }

        info!("Finalized {} processors with {} total messages", 
              self.processors.len(), all_messages.len());
        Ok(all_messages)
    }

    /// Get statistics from all processors
    pub fn get_all_stats(&self) -> HashMap<String, ProcessorStats> {
        self.processors
            .iter()
            .map(|(name, processor)| (name.clone(), processor.get_stats()))
            .collect()
    }

    /// Get the number of registered processors
    pub fn processor_count(&self) -> usize {
        self.processors.len()
    }

    /// Check if any processors are registered for the given modes
    pub fn has_processors_for_modes(&self, modes: ScanMode) -> bool {
        self.processors
            .values()
            .any(|processor| processor.supported_modes().intersects(modes))
    }
}

/// Factory for creating event processors based on scan modes
pub struct ProcessorFactory;

impl ProcessorFactory {
    /// Create all processors needed for the given scan modes
    pub fn create_processors_for_modes(modes: ScanMode) -> Vec<Box<dyn EventProcessor>> {
        let mut processors: Vec<Box<dyn EventProcessor>> = Vec::new();

        // Create history processor if needed
        if modes.contains(ScanMode::HISTORY) {
            processors.push(Box::new(history::HistoryEventProcessor::new()));
        }

        // Create change frequency processor if needed
        if modes.contains(ScanMode::CHANGE_FREQUENCY) {
            processors.push(Box::new(change_frequency::ChangeFrequencyEventProcessor::new()));
        }

        // Create file processor if needed
        if modes.contains(ScanMode::FILES) || modes.contains(ScanMode::METRICS) {
            processors.push(Box::new(files::FileEventProcessor::new()));
        }

        info!("Created {} processors for modes: {:?}", processors.len(), modes);
        processors
    }

    /// Create a processor registry with all processors for the given modes
    pub fn create_registry_for_modes(modes: ScanMode) -> ProcessorRegistry {
        let mut registry = ProcessorRegistry::new(modes);
        
        let processors = Self::create_processors_for_modes(modes);
        for processor in processors {
            registry.register_processor(processor);
        }

        registry
    }
}

/// Coordinator for managing event processing pipeline
pub struct EventProcessingCoordinator {
    registry: ProcessorRegistry,
    total_events_processed: usize,
    total_messages_generated: usize,
    processing_start_time: Option<std::time::Instant>,
}

impl EventProcessingCoordinator {
    /// Create a new event processing coordinator
    pub fn new(modes: ScanMode) -> Self {
        let registry = ProcessorFactory::create_registry_for_modes(modes);
        
        Self {
            registry,
            total_events_processed: 0,
            total_messages_generated: 0,
            processing_start_time: None,
        }
    }

    /// Initialize the coordinator and all processors
    pub async fn initialize(&mut self) -> PluginResult<()> {
        self.processing_start_time = Some(std::time::Instant::now());
        self.registry.initialize_all().await
    }

    /// Process a single event through all relevant processors
    pub async fn process_event(&mut self, event: &RepositoryEvent) -> PluginResult<Vec<ScanMessage>> {
        let messages = self.registry.process_event(event).await?;
        
        self.total_events_processed += 1;
        self.total_messages_generated += messages.len();
        
        Ok(messages)
    }

    /// Finalize processing and get final messages
    pub async fn finalize(&mut self) -> PluginResult<Vec<ScanMessage>> {
        let messages = self.registry.finalize_all().await?;
        self.total_messages_generated += messages.len();
        
        if let Some(start_time) = self.processing_start_time {
            let processing_duration = start_time.elapsed();
            info!(
                "Event processing completed: {} events processed, {} messages generated in {:?}",
                self.total_events_processed, self.total_messages_generated, processing_duration
            );
        }
        
        Ok(messages)
    }

    /// Get processing statistics
    pub fn get_processing_stats(&self) -> CoordinatorStats {
        CoordinatorStats {
            total_events_processed: self.total_events_processed,
            total_messages_generated: self.total_messages_generated,
            processing_duration: self.processing_start_time
                .map(|start| start.elapsed())
                .unwrap_or_default(),
            processor_count: self.registry.processor_count(),
            processor_stats: self.registry.get_all_stats(),
        }
    }

    /// Check if the coordinator has processors for the given modes
    pub fn has_processors_for_modes(&self, modes: ScanMode) -> bool {
        self.registry.has_processors_for_modes(modes)
    }
}

/// Statistics for the event processing coordinator
#[derive(Debug, Clone)]
pub struct CoordinatorStats {
    pub total_events_processed: usize,
    pub total_messages_generated: usize,
    pub processing_duration: std::time::Duration,
    pub processor_count: usize,
    pub processor_stats: HashMap<String, ProcessorStats>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::async_engine::events::{RepositoryEvent, CommitInfo, RepositoryStats};
    use std::time::{SystemTime, Duration};

    // Mock processor for testing
    struct MockProcessor {
        name: &'static str,
        modes: ScanMode,
        messages_generated: usize,
    }

    impl MockProcessor {
        fn new(name: &'static str, modes: ScanMode) -> Self {
            Self {
                name,
                modes,
                messages_generated: 0,
            }
        }
    }

    #[async_trait]
    impl EventProcessor for MockProcessor {
        fn supported_modes(&self) -> ScanMode {
            self.modes
        }

        fn name(&self) -> &'static str {
            self.name
        }

        async fn process_event(&mut self, _event: &RepositoryEvent) -> PluginResult<Vec<ScanMessage>> {
            self.messages_generated += 1;
            Ok(vec![]) // Return empty messages for testing
        }

        fn get_stats(&self) -> ProcessorStats {
            ProcessorStats {
                messages_generated: self.messages_generated,
                ..Default::default()
            }
        }
    }

    #[tokio::test]
    async fn test_processor_registry_creation() {
        let registry = ProcessorRegistry::new(ScanMode::FILES | ScanMode::HISTORY);
        assert_eq!(registry.processor_count(), 0);
        assert_eq!(registry.active_modes, ScanMode::FILES | ScanMode::HISTORY);
    }

    #[tokio::test]
    async fn test_processor_registration() {
        let mut registry = ProcessorRegistry::new(ScanMode::FILES);
        
        let processor = Box::new(MockProcessor::new("test", ScanMode::FILES));
        registry.register_processor(processor);
        
        assert_eq!(registry.processor_count(), 1);
        assert!(registry.has_processors_for_modes(ScanMode::FILES));
    }

    #[tokio::test]
    async fn test_processor_filtering_by_modes() {
        let mut registry = ProcessorRegistry::new(ScanMode::FILES);
        
        // This processor should be registered (matches active modes)
        let files_processor = Box::new(MockProcessor::new("files", ScanMode::FILES));
        registry.register_processor(files_processor);
        
        // This processor should NOT be registered (doesn't match active modes)
        let history_processor = Box::new(MockProcessor::new("history", ScanMode::HISTORY));
        registry.register_processor(history_processor);
        
        assert_eq!(registry.processor_count(), 1);
        assert!(registry.get_processors().contains_key("files"));
        assert!(!registry.get_processors().contains_key("history"));
    }

    #[tokio::test]
    async fn test_event_processing() {
        let mut registry = ProcessorRegistry::new(ScanMode::FILES);
        let processor = Box::new(MockProcessor::new("test", ScanMode::FILES));
        registry.register_processor(processor);

        let event = RepositoryEvent::RepositoryStarted {
            total_commits: Some(10),
            total_files: Some(5),
            scan_modes: ScanMode::FILES,
        };

        let messages = registry.process_event(&event).await.unwrap();
        assert_eq!(messages.len(), 0); // Mock processor returns empty messages
    }

    #[tokio::test]
    async fn test_processor_factory() {
        let processors = ProcessorFactory::create_processors_for_modes(
            ScanMode::FILES | ScanMode::HISTORY
        );
        
        // Should create processors for both FILES and HISTORY modes
        assert!(processors.len() >= 2);
        
        let modes: Vec<ScanMode> = processors
            .iter()
            .map(|p| p.supported_modes())
            .collect();
        
        assert!(modes.iter().any(|m| m.contains(ScanMode::FILES)));
        assert!(modes.iter().any(|m| m.contains(ScanMode::HISTORY)));
    }

    #[tokio::test]
    async fn test_event_processing_coordinator() {
        let mut coordinator = EventProcessingCoordinator::new(ScanMode::FILES);
        coordinator.initialize().await.unwrap();

        let event = RepositoryEvent::RepositoryCompleted {
            stats: RepositoryStats {
                total_commits: 0,
                total_files: 5,
                total_changes: 0,
                scan_duration: Duration::from_secs(1),
                events_emitted: 1,
            },
        };

        let messages = coordinator.process_event(&event).await.unwrap();
        let final_messages = coordinator.finalize().await.unwrap();

        let stats = coordinator.get_processing_stats();
        assert_eq!(stats.total_events_processed, 1);
        assert!(stats.processing_duration > Duration::from_nanos(0));
    }

    fn create_test_commit() -> CommitInfo {
        CommitInfo {
            hash: "abc123".to_string(),
            short_hash: "abc123".to_string(),
            author_name: "Test Author".to_string(),
            author_email: "test@example.com".to_string(),
            committer_name: "Test Author".to_string(),
            committer_email: "test@example.com".to_string(),
            timestamp: SystemTime::UNIX_EPOCH + Duration::from_secs(1000),
            message: "Test commit".to_string(),
            parent_hashes: vec![],
            changed_files: vec!["test.rs".to_string()],
            insertions: 10,
            deletions: 5,
        }
    }
}
