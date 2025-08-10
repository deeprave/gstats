use crate::scanner::async_engine::events::RepositoryEvent;
use crate::scanner::modes::ScanMode;
use crate::scanner::query::QueryParams;
use std::collections::HashSet;
use log::{debug, info};

/// Advanced event filtering system for optimizing event processing
#[derive(Debug, Clone)]
pub struct AdvancedEventFilter {
    /// Base event filter
    base_filter: crate::scanner::async_engine::events::EventFilter,
    
    /// Processor-specific routing rules
    processor_routing: ProcessorRoutingRules,
    
    /// Event batching configuration
    batching_config: EventBatchingConfig,
    
    /// Memory pressure monitoring
    memory_monitor: MemoryPressureMonitor,
    
    /// Performance statistics
    stats: FilteringStats,
}

/// Rules for routing events to specific processors
#[derive(Debug, Clone)]
pub struct ProcessorRoutingRules {
    /// Events that should only go to history processors
    history_only_events: HashSet<&'static str>,
    
    /// Events that should only go to file processors
    file_only_events: HashSet<&'static str>,
    
    /// Events that should only go to change frequency processors
    change_frequency_only_events: HashSet<&'static str>,
    
    /// Events that should be broadcast to all processors
    broadcast_events: HashSet<&'static str>,
}

/// Configuration for event batching
#[derive(Debug, Clone)]
pub struct EventBatchingConfig {
    /// Maximum batch size for events
    pub max_batch_size: usize,
    
    /// Maximum time to wait before flushing a batch (milliseconds)
    pub max_batch_time_ms: u64,
    
    /// Enable batching for high-throughput scenarios
    pub enable_batching: bool,
    
    /// Batch size threshold for memory pressure
    pub memory_pressure_threshold: usize,
}

/// Memory pressure monitoring for large repositories
#[derive(Debug, Clone)]
pub struct MemoryPressureMonitor {
    /// Maximum memory usage in bytes before applying pressure relief
    pub max_memory_bytes: usize,
    
    /// Current estimated memory usage
    pub current_memory_bytes: usize,
    
    /// Enable memory pressure monitoring
    pub enable_monitoring: bool,
    
    /// Actions to take under memory pressure
    pub pressure_actions: MemoryPressureActions,
}

/// Actions to take when memory pressure is detected
#[derive(Debug, Clone)]
pub struct MemoryPressureActions {
    /// Reduce batch sizes
    pub reduce_batch_size: bool,
    
    /// Skip non-essential events
    pub skip_non_essential: bool,
    
    /// Force cache cleanup
    pub force_cache_cleanup: bool,
    
    /// Throttle event generation
    pub throttle_events: bool,
}

/// Statistics for event filtering performance
#[derive(Debug, Clone, Default)]
pub struct FilteringStats {
    pub total_events_processed: usize,
    pub events_filtered_out: usize,
    pub events_routed_to_history: usize,
    pub events_routed_to_files: usize,
    pub events_routed_to_change_frequency: usize,
    pub events_broadcast: usize,
    pub batches_created: usize,
    pub memory_pressure_events: usize,
    pub cache_cleanups_triggered: usize,
}

impl AdvancedEventFilter {
    /// Create a new advanced event filter
    pub fn new(query_params: QueryParams, modes: ScanMode) -> Self {
        let base_filter = crate::scanner::async_engine::events::EventFilter::from_query_params(query_params, modes);
        
        Self {
            base_filter,
            processor_routing: ProcessorRoutingRules::default(),
            batching_config: EventBatchingConfig::default(),
            memory_monitor: MemoryPressureMonitor::default(),
            stats: FilteringStats::default(),
        }
    }

    /// Create with custom configuration
    pub fn with_config(
        query_params: QueryParams, 
        modes: ScanMode,
        batching_config: EventBatchingConfig,
        memory_monitor: MemoryPressureMonitor,
    ) -> Self {
        let base_filter = crate::scanner::async_engine::events::EventFilter::from_query_params(query_params, modes);
        
        Self {
            base_filter,
            processor_routing: ProcessorRoutingRules::default(),
            batching_config,
            memory_monitor,
            stats: FilteringStats::default(),
        }
    }

    /// Apply advanced filtering to an event
    pub fn should_process_event(&mut self, event: &RepositoryEvent) -> FilterDecision {
        self.stats.total_events_processed += 1;

        // First apply base filtering based on event type
        let should_include = match event {
            RepositoryEvent::CommitDiscovered { commit, .. } => {
                self.base_filter.should_include_commit(commit)
            }
            RepositoryEvent::FileScanned { file_info } => {
                self.base_filter.should_include_file(file_info)
            }
            RepositoryEvent::FileChanged { change_data, commit_context, .. } => {
                self.base_filter.should_include_file_change(change_data, commit_context)
            }
            RepositoryEvent::RepositoryStarted { .. } |
            RepositoryEvent::RepositoryCompleted { .. } |
            RepositoryEvent::ScanError { .. } => true, // Always include lifecycle events
        };

        if !should_include {
            self.stats.events_filtered_out += 1;
            return FilterDecision::Skip;
        }

        // Check memory pressure
        if self.memory_monitor.enable_monitoring && self.is_under_memory_pressure() {
            if self.should_skip_under_pressure(event) {
                self.stats.events_filtered_out += 1;
                self.stats.memory_pressure_events += 1;
                return FilterDecision::SkipDueToMemoryPressure;
            }
        }

        // Determine processor routing
        let routing = self.determine_processor_routing(event);
        self.update_routing_stats(&routing);

        FilterDecision::Process { routing }
    }

    /// Determine which processors should receive this event
    fn determine_processor_routing(&self, event: &RepositoryEvent) -> ProcessorRouting {
        let event_type = event.event_type();

        // Check for specific routing rules
        if self.processor_routing.history_only_events.contains(&event_type) {
            return ProcessorRouting::HistoryOnly;
        }

        if self.processor_routing.file_only_events.contains(&event_type) {
            return ProcessorRouting::FileOnly;
        }

        if self.processor_routing.change_frequency_only_events.contains(&event_type) {
            return ProcessorRouting::ChangeFrequencyOnly;
        }

        if self.processor_routing.broadcast_events.contains(&event_type) {
            return ProcessorRouting::Broadcast;
        }

        // Default routing based on event type and active modes
        match event {
            RepositoryEvent::CommitDiscovered { .. } => {
                if self.base_filter.modes.contains(ScanMode::HISTORY) && 
                   self.base_filter.modes.contains(ScanMode::CHANGE_FREQUENCY) {
                    ProcessorRouting::HistoryAndChangeFrequency
                } else if self.base_filter.modes.contains(ScanMode::HISTORY) {
                    ProcessorRouting::HistoryOnly
                } else if self.base_filter.modes.contains(ScanMode::CHANGE_FREQUENCY) {
                    ProcessorRouting::ChangeFrequencyOnly
                } else {
                    ProcessorRouting::None
                }
            }
            RepositoryEvent::FileScanned { .. } => {
                if self.base_filter.modes.contains(ScanMode::FILES) || 
                   self.base_filter.modes.contains(ScanMode::METRICS) {
                    ProcessorRouting::FileOnly
                } else {
                    ProcessorRouting::None
                }
            }
            RepositoryEvent::FileChanged { .. } => {
                if self.base_filter.modes.contains(ScanMode::CHANGE_FREQUENCY) {
                    ProcessorRouting::ChangeFrequencyOnly
                } else {
                    ProcessorRouting::None
                }
            }
            RepositoryEvent::RepositoryStarted { .. } | 
            RepositoryEvent::RepositoryCompleted { .. } => {
                ProcessorRouting::Broadcast
            }
            RepositoryEvent::ScanError { .. } => {
                ProcessorRouting::Broadcast
            }
        }
    }

    /// Check if system is under memory pressure
    fn is_under_memory_pressure(&self) -> bool {
        self.memory_monitor.current_memory_bytes > self.memory_monitor.max_memory_bytes
    }

    /// Determine if event should be skipped under memory pressure
    fn should_skip_under_pressure(&self, event: &RepositoryEvent) -> bool {
        if !self.memory_monitor.pressure_actions.skip_non_essential {
            return false;
        }

        // Skip non-essential events under memory pressure
        match event {
            RepositoryEvent::FileChanged { .. } => {
                // Skip file change events if not critical for active modes
                !self.base_filter.modes.contains(ScanMode::CHANGE_FREQUENCY)
            }
            RepositoryEvent::FileScanned { file_info } => {
                // Skip large files or binary files under pressure
                file_info.size > 1024 * 1024 || file_info.is_binary // Skip files > 1MB or binary
            }
            _ => false, // Don't skip essential events
        }
    }

    /// Update routing statistics
    fn update_routing_stats(&mut self, routing: &ProcessorRouting) {
        match routing {
            ProcessorRouting::HistoryOnly => self.stats.events_routed_to_history += 1,
            ProcessorRouting::FileOnly => self.stats.events_routed_to_files += 1,
            ProcessorRouting::ChangeFrequencyOnly => self.stats.events_routed_to_change_frequency += 1,
            ProcessorRouting::HistoryAndChangeFrequency => {
                self.stats.events_routed_to_history += 1;
                self.stats.events_routed_to_change_frequency += 1;
            }
            ProcessorRouting::Broadcast => self.stats.events_broadcast += 1,
            ProcessorRouting::None => {} // No routing
        }
    }

    /// Update memory usage estimate
    pub fn update_memory_usage(&mut self, bytes: usize) {
        self.memory_monitor.current_memory_bytes = bytes;
        
        if self.is_under_memory_pressure() {
            self.handle_memory_pressure();
        }
    }

    /// Handle memory pressure by taking configured actions
    fn handle_memory_pressure(&mut self) {
        debug!("Memory pressure detected: {} bytes used, {} bytes max", 
               self.memory_monitor.current_memory_bytes, 
               self.memory_monitor.max_memory_bytes);

        if self.memory_monitor.pressure_actions.reduce_batch_size {
            self.batching_config.max_batch_size = 
                (self.batching_config.max_batch_size / 2).max(1);
            debug!("Reduced batch size to {}", self.batching_config.max_batch_size);
        }

        if self.memory_monitor.pressure_actions.force_cache_cleanup {
            self.stats.cache_cleanups_triggered += 1;
            info!("Memory pressure triggered cache cleanup");
        }
    }

    /// Get filtering statistics
    pub fn get_stats(&self) -> &FilteringStats {
        &self.stats
    }

    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.stats = FilteringStats::default();
    }

    /// Check if batching is enabled and should be used
    pub fn should_use_batching(&self) -> bool {
        self.batching_config.enable_batching && 
        !self.is_under_memory_pressure()
    }

    /// Get current batch configuration
    pub fn get_batch_config(&self) -> &EventBatchingConfig {
        &self.batching_config
    }
}

/// Decision made by the filter about an event
#[derive(Debug, Clone, PartialEq)]
pub enum FilterDecision {
    /// Skip this event entirely
    Skip,
    
    /// Skip due to memory pressure
    SkipDueToMemoryPressure,
    
    /// Process this event with specific routing
    Process { routing: ProcessorRouting },
}

/// Routing decision for processors
#[derive(Debug, Clone, PartialEq)]
pub enum ProcessorRouting {
    /// Send only to history processors
    HistoryOnly,
    
    /// Send only to file processors
    FileOnly,
    
    /// Send only to change frequency processors
    ChangeFrequencyOnly,
    
    /// Send to both history and change frequency processors
    HistoryAndChangeFrequency,
    
    /// Broadcast to all active processors
    Broadcast,
    
    /// Don't send to any processors
    None,
}

impl Default for ProcessorRoutingRules {
    fn default() -> Self {
        let mut history_only = HashSet::new();
        history_only.insert("CommitDiscovered");

        let mut file_only = HashSet::new();
        file_only.insert("FileScanned");

        let mut change_frequency_only = HashSet::new();
        change_frequency_only.insert("FileChanged");

        let mut broadcast = HashSet::new();
        broadcast.insert("RepositoryStarted");
        broadcast.insert("RepositoryCompleted");
        broadcast.insert("ScanError");

        Self {
            history_only_events: history_only,
            file_only_events: file_only,
            change_frequency_only_events: change_frequency_only,
            broadcast_events: broadcast,
        }
    }
}

impl Default for EventBatchingConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 100,
            max_batch_time_ms: 100,
            enable_batching: true,
            memory_pressure_threshold: 50,
        }
    }
}

impl Default for MemoryPressureMonitor {
    fn default() -> Self {
        Self {
            max_memory_bytes: 256 * 1024 * 1024, // 256MB default
            current_memory_bytes: 0,
            enable_monitoring: true,
            pressure_actions: MemoryPressureActions::default(),
        }
    }
}

impl Default for MemoryPressureActions {
    fn default() -> Self {
        Self {
            reduce_batch_size: true,
            skip_non_essential: true,
            force_cache_cleanup: true,
            throttle_events: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::async_engine::events::{CommitInfo, FileInfo};
    use std::time::SystemTime;

    fn create_test_commit() -> CommitInfo {
        CommitInfo {
            hash: "abc123".to_string(),
            short_hash: "abc123".to_string(),
            author_name: "Test Author".to_string(),
            author_email: "test@example.com".to_string(),
            committer_name: "Test Author".to_string(),
            committer_email: "test@example.com".to_string(),
            timestamp: SystemTime::now(),
            message: "Test commit".to_string(),
            parent_hashes: vec![],
            changed_files: vec![],
            insertions: 10,
            deletions: 5,
        }
    }

    fn create_test_file() -> FileInfo {
        FileInfo {
            path: std::path::PathBuf::from("/test/file.rs"),
            relative_path: "file.rs".to_string(),
            size: 1024,
            extension: Some("rs".to_string()),
            is_binary: false,
            line_count: Some(50),
            last_modified: Some(SystemTime::now()),
        }
    }

    #[test]
    fn test_advanced_filter_creation() {
        let query_params = QueryParams::default();
        let modes = ScanMode::HISTORY | ScanMode::FILES;
        let filter = AdvancedEventFilter::new(query_params, modes);
        
        assert_eq!(filter.stats.total_events_processed, 0);
        assert!(filter.batching_config.enable_batching);
        assert!(filter.memory_monitor.enable_monitoring);
    }

    #[test]
    fn test_processor_routing_commit_event() {
        let query_params = QueryParams::default();
        let modes = ScanMode::HISTORY;
        let mut filter = AdvancedEventFilter::new(query_params, modes);
        
        let commit = create_test_commit();
        let event = RepositoryEvent::CommitDiscovered { commit, index: 0 };
        
        let decision = filter.should_process_event(&event);
        match decision {
            FilterDecision::Process { routing } => {
                assert_eq!(routing, ProcessorRouting::HistoryOnly);
            }
            _ => panic!("Expected Process decision"),
        }
        
        assert_eq!(filter.stats.events_routed_to_history, 1);
    }

    #[test]
    fn test_processor_routing_file_event() {
        let query_params = QueryParams::default();
        let modes = ScanMode::FILES;
        let mut filter = AdvancedEventFilter::new(query_params, modes);
        
        let file = create_test_file();
        let event = RepositoryEvent::FileScanned { file_info: file };
        
        let decision = filter.should_process_event(&event);
        match decision {
            FilterDecision::Process { routing } => {
                assert_eq!(routing, ProcessorRouting::FileOnly);
            }
            _ => panic!("Expected Process decision"),
        }
        
        assert_eq!(filter.stats.events_routed_to_files, 1);
    }

    #[test]
    fn test_broadcast_routing() {
        let query_params = QueryParams::default();
        let modes = ScanMode::HISTORY | ScanMode::FILES;
        let mut filter = AdvancedEventFilter::new(query_params, modes);
        
        let event = RepositoryEvent::RepositoryStarted {
            total_commits: Some(100),
            total_files: Some(50),
            scan_modes: modes,
        };
        
        let decision = filter.should_process_event(&event);
        match decision {
            FilterDecision::Process { routing } => {
                assert_eq!(routing, ProcessorRouting::Broadcast);
            }
            _ => panic!("Expected Process decision"),
        }
        
        assert_eq!(filter.stats.events_broadcast, 1);
    }

    #[test]
    fn test_memory_pressure_handling() {
        let query_params = QueryParams::default();
        let modes = ScanMode::FILES;
        let mut memory_monitor = MemoryPressureMonitor::default();
        memory_monitor.max_memory_bytes = 1024; // Very low threshold for testing
        
        let mut filter = AdvancedEventFilter::with_config(
            query_params,
            modes,
            EventBatchingConfig::default(),
            memory_monitor,
        );
        
        // Simulate high memory usage
        filter.update_memory_usage(2048); // Above threshold
        
        // Create a large file event
        let mut large_file = create_test_file();
        large_file.size = 2 * 1024 * 1024; // 2MB file
        let event = RepositoryEvent::FileScanned { file_info: large_file };
        
        let decision = filter.should_process_event(&event);
        assert_eq!(decision, FilterDecision::SkipDueToMemoryPressure);
        assert_eq!(filter.stats.memory_pressure_events, 1);
    }

    #[test]
    fn test_batching_configuration() {
        let query_params = QueryParams::default();
        let modes = ScanMode::HISTORY;
        let filter = AdvancedEventFilter::new(query_params, modes);
        
        assert!(filter.should_use_batching());
        assert_eq!(filter.get_batch_config().max_batch_size, 100);
        assert_eq!(filter.get_batch_config().max_batch_time_ms, 100);
    }

    #[test]
    fn test_statistics_tracking() {
        let query_params = QueryParams::default();
        let modes = ScanMode::HISTORY | ScanMode::FILES;
        let mut filter = AdvancedEventFilter::new(query_params, modes);
        
        // Process different types of events
        let commit = create_test_commit();
        let commit_event = RepositoryEvent::CommitDiscovered { commit, index: 0 };
        filter.should_process_event(&commit_event);
        
        let file = create_test_file();
        let file_event = RepositoryEvent::FileScanned { file_info: file };
        filter.should_process_event(&file_event);
        
        let stats = filter.get_stats();
        assert_eq!(stats.total_events_processed, 2);
        assert_eq!(stats.events_routed_to_history, 1);
        assert_eq!(stats.events_routed_to_files, 1);
    }

    #[test]
    fn test_stats_reset() {
        let query_params = QueryParams::default();
        let modes = ScanMode::HISTORY;
        let mut filter = AdvancedEventFilter::new(query_params, modes);
        
        // Process an event
        let commit = create_test_commit();
        let event = RepositoryEvent::CommitDiscovered { commit, index: 0 };
        filter.should_process_event(&event);
        
        assert_eq!(filter.get_stats().total_events_processed, 1);
        
        // Reset stats
        filter.reset_stats();
        assert_eq!(filter.get_stats().total_events_processed, 0);
    }
}
