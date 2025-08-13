use crate::scanner::async_engine::events::RepositoryEvent;
use crate::scanner::query::QueryParams;
use log::{debug, info};

/// Advanced event filtering system for optimizing event processing
#[derive(Debug, Clone)]
pub struct AdvancedEventFilter {
    /// Base event filter
    base_filter: crate::scanner::async_engine::events::EventFilter,
    
    /// Event batching configuration
    batching_config: EventBatchingConfig,
    
    /// Memory pressure monitoring
    memory_monitor: MemoryPressureMonitor,
    
    /// Performance statistics
    stats: FilteringStats,
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
    pub batches_created: usize,
    pub memory_pressure_events: usize,
    pub cache_cleanups_triggered: usize,
}

impl AdvancedEventFilter {
    /// Create a new advanced event filter
    pub fn new(query_params: QueryParams) -> Self {
        let base_filter = crate::scanner::async_engine::events::EventFilter::from_query_params(query_params);
        
        Self {
            base_filter,
            batching_config: EventBatchingConfig::default(),
            memory_monitor: MemoryPressureMonitor::default(),
            stats: FilteringStats::default(),
        }
    }

    /// Create with custom configuration
    pub fn with_config(
        query_params: QueryParams,
        batching_config: EventBatchingConfig,
        memory_monitor: MemoryPressureMonitor,
    ) -> Self {
        let base_filter = crate::scanner::async_engine::events::EventFilter::from_query_params(query_params);
        
        Self {
            base_filter,
            batching_config,
            memory_monitor,
            stats: FilteringStats::default(),
        }
    }

    /// Apply advanced filtering to an event
    pub fn should_process_event(&mut self, event: &RepositoryEvent) -> FilterDecision {
        self.stats.total_events_processed += 1;

        // Check memory pressure - only performance-related filtering allowed
        if self.memory_monitor.enable_monitoring && self.is_under_memory_pressure() {
            if self.should_skip_under_pressure(event) {
                self.stats.events_filtered_out += 1;
                self.stats.memory_pressure_events += 1;
                return FilterDecision::SkipDueToMemoryPressure;
            }
        }

        // All events should be processed (no content-based filtering after event creation)
        // No routing decisions made by scanner - processors handle their own event filtering
        FilterDecision::Process
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
                // Skip file change events under memory pressure
                true // Can be skipped under pressure
            }
            RepositoryEvent::FileScanned { file_info } => {
                // Skip large files or binary files under pressure
                file_info.size > 1024 * 1024 || file_info.is_binary // Skip files > 1MB or binary
            }
            _ => false, // Don't skip essential events
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
    
    /// Process this event (no routing decisions made by scanner)
    Process,
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
    use crate::scanner::query::{DateRange, AuthorFilter};
    use std::time::{SystemTime, UNIX_EPOCH, Duration};

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
        let filter = AdvancedEventFilter::new(query_params);
        
        assert_eq!(filter.stats.total_events_processed, 0);
        assert!(filter.batching_config.enable_batching);
        assert!(filter.memory_monitor.enable_monitoring);
    }




    #[test]
    fn test_memory_pressure_handling() {
        let query_params = QueryParams::default();
        let mut memory_monitor = MemoryPressureMonitor::default();
        memory_monitor.max_memory_bytes = 1024; // Very low threshold for testing
        
        let mut filter = AdvancedEventFilter::with_config(
            query_params,
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
        let filter = AdvancedEventFilter::new(query_params);
        
        assert!(filter.should_use_batching());
        assert_eq!(filter.get_batch_config().max_batch_size, 100);
        assert_eq!(filter.get_batch_config().max_batch_time_ms, 100);
    }

    #[test]
    fn test_statistics_tracking() {
        let query_params = QueryParams::default();
        let mut filter = AdvancedEventFilter::new(query_params);
        
        // Process different types of events
        let commit = create_test_commit();
        let commit_event = RepositoryEvent::CommitDiscovered { commit, index: 0 };
        filter.should_process_event(&commit_event);
        
        let file = create_test_file();
        let file_event = RepositoryEvent::FileScanned { file_info: file };
        filter.should_process_event(&file_event);
        
        let stats = filter.get_stats();
        assert_eq!(stats.total_events_processed, 2);
        // No routing statistics tracked anymore - all events are simply processed
    }

    #[test]
    fn test_stats_reset() {
        let query_params = QueryParams::default();
        let mut filter = AdvancedEventFilter::new(query_params);
        
        // Process an event
        let commit = create_test_commit();
        let event = RepositoryEvent::CommitDiscovered { commit, index: 0 };
        filter.should_process_event(&event);
        
        assert_eq!(filter.get_stats().total_events_processed, 1);
        
        // Reset stats
        filter.reset_stats();
        assert_eq!(filter.get_stats().total_events_processed, 0);
    }

    #[test]
    fn test_no_post_event_content_filtering() {
        // Test that events are not filtered based on content after creation
        // This test should fail until post-event filtering is removed
        
        let mut query_params = QueryParams::default();
        // Set very restrictive filters that should NOT affect post-event processing
        query_params.date_range = Some(DateRange {
            start: Some(UNIX_EPOCH + Duration::from_secs(9999999999)), // Far future
            end: None,
        });
        query_params.authors = AuthorFilter {
            include: vec!["nonexistent_author".to_string()],
            exclude: vec![],
        };
        
        let mut filter = AdvancedEventFilter::new(query_params);
        
        // Create events that would normally be filtered out by content
        let old_commit = create_test_commit(); // This has old timestamp and different author
        let commit_event = RepositoryEvent::CommitDiscovered { commit: old_commit, index: 0 };
        
        let file = create_test_file();
        let file_event = RepositoryEvent::FileScanned { file_info: file };
        
        // Process events - should NOT be filtered by content since they're already created
        let commit_decision = filter.should_process_event(&commit_event);
        let file_decision = filter.should_process_event(&file_event);
        
        // Events should be processed regardless of query_params content filters
        // because post-event filtering should be removed
        match commit_decision {
            FilterDecision::Process => {}, // Expected after removing post-event filtering
            FilterDecision::Skip => panic!("Post-event content filtering still exists - commit should be processed"),
            FilterDecision::SkipDueToMemoryPressure => panic!("Unexpected memory pressure during test"),
        }
        
        match file_decision {
            FilterDecision::Process => {}, // Expected after removing post-event filtering  
            FilterDecision::Skip => panic!("Post-event content filtering still exists - file should be processed"),
            FilterDecision::SkipDueToMemoryPressure => panic!("Unexpected memory pressure during test"),
        }
        
        let stats = filter.get_stats();
        assert_eq!(stats.total_events_processed, 2, "All events should be processed without content filtering");
    }

    #[test]
    fn test_no_processor_routing_decisions() {
        // Test that no processor routing decisions are made
        // Scanner now simply processes events without routing decisions
        
        let query_params = QueryParams::default();
        let mut filter = AdvancedEventFilter::new(query_params);
        
        let commit = create_test_commit();
        let event = RepositoryEvent::CommitDiscovered { commit, index: 0 };
        
        let decision = filter.should_process_event(&event);
        
        // After removing routing, the decision should be a simple Process
        match decision {
            FilterDecision::Process => {
                // Success - no routing information, just process the event
            },
            FilterDecision::Skip => {
                panic!("Events should be processed, not skipped");
            },
            FilterDecision::SkipDueToMemoryPressure => {
                panic!("Unexpected memory pressure during test");
            }
        }
        
        // Verify that no routing statistics are tracked
        let stats = filter.get_stats();
        assert_eq!(stats.total_events_processed, 1);
        // No routing fields should exist in stats anymore
    }
}
