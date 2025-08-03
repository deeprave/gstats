//! Memory-Conscious Queue Implementation
//! 
//! MPSC queue with memory monitoring and backoff capabilities

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, AtomicBool, Ordering};
use std::sync::Mutex;
use crossbeam_queue::SegQueue;
use crate::queue::{Queue, QueueError};
use crate::queue::memory_tracker::MemoryTracker;
use crate::queue::versioned_message::QueueMessage;
use crate::queue::backoff::{BackoffAlgorithm, BackoffConfig, BackoffStrategy, BackoffMetrics};
use crate::scanner::messages::ScanMessage;
use std::time::{Duration, Instant};
use std::collections::VecDeque;

/// Queue-specific memory statistics
#[derive(Debug, Clone)]
pub struct QueueMemoryStatistics {
    pub allocated_bytes: usize,
    pub message_count: usize,
    pub capacity: usize,
    pub average_message_size: f64,
    pub utilization_percent: f64,
}

/// Configuration for pressure response system
#[derive(Debug, Clone)]
pub struct PressureResponseConfig {
    pub throttle_threshold: f64,
    pub drop_threshold: f64,
    pub throttle_factor: f64,
    pub recovery_factor: f64,
}

impl PressureResponseConfig {
    /// Create a conservative pressure response configuration (higher thresholds, gentler throttling)
    pub fn conservative() -> Self {
        Self {
            throttle_threshold: 85.0,
            drop_threshold: 95.0,
            throttle_factor: 0.3,
            recovery_factor: 0.9,
        }
    }
    
    /// Create an aggressive pressure response configuration (lower thresholds, stronger throttling)
    pub fn aggressive() -> Self {
        Self {
            throttle_threshold: 60.0,
            drop_threshold: 80.0,
            throttle_factor: 0.8,
            recovery_factor: 0.6,
        }
    }
    
    /// Create a balanced pressure response configuration
    pub fn balanced() -> Self {
        Self {
            throttle_threshold: 75.0,
            drop_threshold: 90.0,
            throttle_factor: 0.5,
            recovery_factor: 0.8,
        }
    }
    
    /// Validate pressure response configuration parameters
    pub fn validate(&self) -> Result<(), String> {
        if self.throttle_threshold < 0.0 || self.throttle_threshold > 100.0 {
            return Err("Throttle threshold must be between 0.0 and 100.0".to_string());
        }
        
        if self.drop_threshold < 0.0 || self.drop_threshold > 100.0 {
            return Err("Drop threshold must be between 0.0 and 100.0".to_string());
        }
        
        if self.drop_threshold <= self.throttle_threshold {
            return Err("Drop threshold must be greater than throttle threshold".to_string());
        }
        
        if self.throttle_factor < 0.0 || self.throttle_factor > 1.0 {
            return Err("Throttle factor must be between 0.0 and 1.0".to_string());
        }
        
        if self.recovery_factor < 0.0 || self.recovery_factor > 1.0 {
            return Err("Recovery factor must be between 0.0 and 1.0".to_string());
        }
        
        Ok(())
    }
}

impl Default for PressureResponseConfig {
    fn default() -> Self {
        Self::balanced()
    }
}

/// Status of pressure response system
#[derive(Debug)]
pub struct PressureResponseStatus {
    pub is_throttling: bool,
    pub current_pressure_level: f64,
}

/// Metrics for pressure response system
#[derive(Debug)]
pub struct PressureResponseMetrics {
    pub messages_dropped: u64,
    pub throttle_events: u64,
    pub recovery_events: u64,
}

static PRESSURE_METRICS: std::sync::Mutex<PressureResponseMetrics> = std::sync::Mutex::new(PressureResponseMetrics {
    messages_dropped: 0,
    throttle_events: 0,
    recovery_events: 0,
});

/// Memory usage sample for tracking recovery trends
#[derive(Debug, Clone)]
struct MemoryUsageSample {
    timestamp: Instant,
    usage_percent: f64,
}

/// Simple memory usage history for adaptive backoff
#[derive(Debug)]
struct MemoryUsageHistory {
    samples: VecDeque<MemoryUsageSample>,
    max_samples: usize,
}

impl MemoryUsageHistory {
    fn new(max_samples: usize) -> Self {
        Self {
            samples: VecDeque::with_capacity(max_samples),
            max_samples,
        }
    }

    fn add_sample(&mut self, usage_percent: f64) {
        let sample = MemoryUsageSample {
            timestamp: Instant::now(),
            usage_percent,
        };

        if self.samples.len() >= self.max_samples {
            self.samples.pop_front();
        }
        self.samples.push_back(sample);
    }

    /// Check if memory usage is trending downward (recovering)
    fn is_recovering(&self) -> bool {
        if self.samples.len() < 2 {
            return false;
        }

        let recent_samples: Vec<_> = self.samples.iter().rev().take(3).collect();
        if recent_samples.len() < 2 {
            return false;
        }

        // Check if usage is generally decreasing
        recent_samples.windows(2).all(|window| {
            window[0].usage_percent <= window[1].usage_percent
        })
    }
}

/// Memory-monitored MPSC queue for ScanMessages
pub struct MemoryQueue {
    inner: Arc<SegQueue<ScanMessage>>,
    capacity: usize,
    memory_limit: usize,
    current_size: Arc<AtomicUsize>,
    memory_tracker: Option<Arc<MemoryTracker>>,
    // Track this queue's individual contribution to shared tracker
    individual_memory_usage: Arc<AtomicUsize>,
    // Backoff algorithm for memory pressure handling
    backoff: Arc<Mutex<BackoffAlgorithm>>,
    // Pressure response system
    pressure_response_enabled: Arc<AtomicBool>,
    pressure_config: Arc<Mutex<Option<PressureResponseConfig>>>,
    extreme_pressure_mode: Arc<AtomicBool>,
    // Memory usage history for adaptive backoff
    memory_history: Arc<Mutex<MemoryUsageHistory>>,
}

/// Memory-monitored MPSC queue for versioned messages
pub struct VersionedMemoryQueue {
    inner: Arc<SegQueue<QueueMessage>>,
    capacity: usize,
    memory_limit: usize,
    current_size: Arc<AtomicUsize>,
    memory_tracker: Option<Arc<MemoryTracker>>,
}

impl MemoryQueue {
    /// Create a new memory queue with specified capacity and memory limit
    pub fn new(capacity: usize, memory_limit: usize) -> Self {
        Self {
            inner: Arc::new(SegQueue::new()),
            capacity,
            memory_limit,
            current_size: Arc::new(AtomicUsize::new(0)),
            memory_tracker: None,
            individual_memory_usage: Arc::new(AtomicUsize::new(0)),
            backoff: Arc::new(Mutex::new(BackoffAlgorithm::new(BackoffConfig::default()))),
            pressure_response_enabled: Arc::new(AtomicBool::new(false)),
            pressure_config: Arc::new(Mutex::new(None)),
            extreme_pressure_mode: Arc::new(AtomicBool::new(false)),
            memory_history: Arc::new(Mutex::new(MemoryUsageHistory::new(10))),
        }
    }

    /// Create a new memory queue with shared memory tracker
    pub fn with_shared_tracker(capacity: usize, memory_limit: usize, tracker: Arc<MemoryTracker>) -> Self {
        Self {
            inner: Arc::new(SegQueue::new()),
            capacity,
            memory_limit,
            current_size: Arc::new(AtomicUsize::new(0)),
            memory_tracker: Some(tracker),
            individual_memory_usage: Arc::new(AtomicUsize::new(0)),
            backoff: Arc::new(Mutex::new(BackoffAlgorithm::new(BackoffConfig::default()))),
            pressure_response_enabled: Arc::new(AtomicBool::new(false)),
            pressure_config: Arc::new(Mutex::new(None)),
            extreme_pressure_mode: Arc::new(AtomicBool::new(false)),
            memory_history: Arc::new(Mutex::new(MemoryUsageHistory::new(10))),
        }
    }
    
    /// Create a new memory queue with memory tracking enabled
    pub fn with_memory_tracking(capacity: usize, memory_limit: usize) -> Self {
        Self {
            inner: Arc::new(SegQueue::new()),
            capacity,
            memory_limit,
            current_size: Arc::new(AtomicUsize::new(0)),
            memory_tracker: Some(Arc::new(MemoryTracker::new(memory_limit))),
            individual_memory_usage: Arc::new(AtomicUsize::new(0)),
            backoff: Arc::new(Mutex::new(BackoffAlgorithm::new(BackoffConfig::default()))),
            pressure_response_enabled: Arc::new(AtomicBool::new(false)),
            pressure_config: Arc::new(Mutex::new(None)),
            extreme_pressure_mode: Arc::new(AtomicBool::new(false)),
            memory_history: Arc::new(Mutex::new(MemoryUsageHistory::new(10))),
        }
    }
    
    /// Get current memory usage in bytes (individual queue contribution)
    pub fn memory_usage(&self) -> usize {
        self.individual_memory_usage.load(Ordering::Relaxed)
    }
    
    /// Get memory usage as percentage of limit
    pub fn memory_usage_percent(&self) -> f64 {
        match &self.memory_tracker {
            Some(tracker) => tracker.usage_percent(),
            None => 0.0,
        }
    }
    
    /// Check if memory usage exceeds given threshold percentage
    pub fn exceeds_memory_threshold(&self, threshold_percent: f64) -> bool {
        match &self.memory_tracker {
            Some(tracker) => tracker.exceeds_threshold(threshold_percent),
            None => false,
        }
    }
    
    /// Get current memory pressure level
    pub fn get_memory_pressure_level(&self) -> crate::queue::memory_tracker::MemoryPressureLevel {
        match &self.memory_tracker {
            Some(tracker) => tracker.get_pressure_level(),
            None => crate::queue::memory_tracker::MemoryPressureLevel::Normal,
        }
    }
    
    /// Get memory limit in bytes
    pub fn get_memory_limit(&self) -> usize {
        match &self.memory_tracker {
            Some(tracker) => tracker.memory_limit(),
            None => 0,
        }
    }
    
    /// Create a new versioned memory queue
    pub fn new_versioned(capacity: usize, memory_limit: usize) -> VersionedMemoryQueue {
        VersionedMemoryQueue {
            inner: Arc::new(SegQueue::new()),
            capacity,
            memory_limit,
            current_size: Arc::new(AtomicUsize::new(0)),
            memory_tracker: None,
        }
    }
    
    /// Create a new versioned memory queue with memory tracking
    pub fn new_versioned_with_memory_tracking(capacity: usize, memory_limit: usize) -> VersionedMemoryQueue {
        VersionedMemoryQueue {
            inner: Arc::new(SegQueue::new()),
            capacity,
            memory_limit,
            current_size: Arc::new(AtomicUsize::new(0)),
            memory_tracker: Some(Arc::new(MemoryTracker::new(memory_limit))),
        }
    }

    /// Estimate memory size of a message
    fn estimate_message_size(message: &ScanMessage) -> usize {
        // Simple estimation - in practice this could be more sophisticated
        std::mem::size_of::<ScanMessage>() + 
        bincode::serialized_size(message).unwrap_or(256) as usize
    }

    /// Get queue-specific memory statistics
    pub fn get_memory_statistics(&self) -> QueueMemoryStatistics {
        let size = self.size();
        let capacity = self.capacity();
        let allocated = self.memory_usage();
        
        QueueMemoryStatistics {
            allocated_bytes: allocated,
            message_count: size,
            capacity,
            average_message_size: if size > 0 { allocated as f64 / size as f64 } else { 0.0 },
            utilization_percent: (size as f64 / capacity as f64) * 100.0,
        }
    }

    /// Generate a memory usage report
    pub fn generate_memory_report(&self) -> String {
        let stats = self.get_memory_statistics();
        let usage_percent = self.memory_usage_percent();
        let pressure = if let Some(tracker) = &self.memory_tracker {
            format!("{:?}", tracker.get_pressure_level())
        } else {
            "N/A".to_string()
        };

        format!(
            "Memory Usage Report\n\
            Current Usage: {} bytes\n\
            Peak Usage: {} bytes\n\
            Available: {} bytes\n\
            Usage Percentage: {:.2}%\n\
            Message Count: {}\n\
            Average Message Size: {:.2} bytes\n\
            Memory Pressure: {}",
            stats.allocated_bytes,
            self.memory_tracker.as_ref().map_or(0, |t| t.peak_bytes()),
            self.memory_tracker.as_ref().map_or(0, |t| t.available_bytes()),
            usage_percent,
            stats.message_count,
            stats.average_message_size,
            pressure
        )
    }

    /// Generate a detailed memory usage report
    pub fn generate_detailed_memory_report(&self) -> String {
        let basic_report = self.generate_memory_report();
        
        if let Some(tracker) = &self.memory_tracker {
            let detailed_stats = tracker.get_statistics();
            format!(
                "{}\n\n\
                Allocation History\n\
                Total Allocations: {}\n\
                Total Deallocations: {}\n\
                Fragmentation\n\
                Fragmentation Ratio: {:.2}\n\
                Recommendations\n\
                Defragmentation Recommended: {}",
                basic_report,
                detailed_stats.total_allocations,
                detailed_stats.total_deallocations,
                detailed_stats.fragmentation_ratio,
                tracker.should_recommend_defragmentation()
            )
        } else {
            format!("{}\n\nDetailed tracking not enabled", basic_report)
        }
    }

    // Backoff Algorithm Methods
    
    /// Enable backoff algorithm for memory pressure handling
    pub fn enable_backoff_algorithm(&self) {
        if let Ok(mut backoff) = self.backoff.lock() {
            backoff.enable();
        }
    }
    
    /// Disable backoff algorithm
    pub fn disable_backoff_algorithm(&self) {
        if let Ok(mut backoff) = self.backoff.lock() {
            backoff.disable();
        }
    }
    
    /// Configure backoff parameters with validation
    pub fn configure_backoff(&self, config: BackoffConfig) -> Result<(), String> {
        config.validate()?;
        if let Ok(mut backoff) = self.backoff.lock() {
            backoff.set_config(config);
            Ok(())
        } else {
            Err("Failed to acquire backoff lock".to_string())
        }
    }
    
    /// Configure backoff parameters without validation (for compatibility)
    pub fn configure_backoff_unchecked(&self, config: BackoffConfig) {
        if let Ok(mut backoff) = self.backoff.lock() {
            backoff.set_config(config);
        }
    }
    
    /// Set backoff strategy
    pub fn set_backoff_strategy(&self, strategy: BackoffStrategy) {
        if let Ok(mut backoff) = self.backoff.lock() {
            backoff.set_strategy(strategy);
        }
    }
    
    /// Get backoff metrics
    pub fn get_backoff_metrics(&self) -> BackoffMetrics {
        if let Ok(backoff) = self.backoff.lock() {
            backoff.get_metrics()
        } else {
            BackoffMetrics {
                total_backoff_events: 0,
                total_backoff_duration: Duration::from_millis(0),
                average_backoff_delay: Duration::from_millis(0),
                current_backoff_level: 0,
            }
        }
    }
    
    /// Enqueue with backoff for memory pressure handling
    pub fn enqueue_with_backoff(&self, message: ScanMessage) -> Result<(), QueueError> {
        // Check if we should drop the message due to extreme pressure
        if self.extreme_pressure_mode.load(Ordering::Relaxed) {
            if let Ok(mut metrics) = PRESSURE_METRICS.lock() {
                metrics.messages_dropped += 1;
            }
            log::warn!("Queue: Dropping message due to extreme memory pressure");
            return Err(QueueError::MessageDropped("Extreme memory pressure".to_string()));
        }
        
        // Record current memory usage for adaptive backoff
        if let Some(tracker) = &self.memory_tracker {
            let usage_percent = tracker.usage_percent();
            if let Ok(mut history) = self.memory_history.lock() {
                history.add_sample(usage_percent);
            }
            
            // Apply pressure response if enabled
            if self.pressure_response_enabled.load(Ordering::Relaxed) {
                if let Ok(config) = self.pressure_config.lock() {
                    if let Some(cfg) = config.as_ref() {
                        // Check if we should drop messages due to extreme pressure
                        if usage_percent > cfg.drop_threshold {
                            if let Ok(mut metrics) = PRESSURE_METRICS.lock() {
                                metrics.messages_dropped += 1;
                            }
                            log::warn!("Queue: Dropping message - memory usage {:.1}% exceeds drop threshold {:.1}%", 
                                usage_percent, cfg.drop_threshold);
                            return Err(QueueError::MessageDropped("Memory pressure too high".to_string()));
                        }
                        
                        // Apply throttling if above throttle threshold
                        if usage_percent > cfg.throttle_threshold {
                            if let Ok(mut metrics) = PRESSURE_METRICS.lock() {
                                metrics.throttle_events += 1;
                            }
                            
                            // Calculate throttle delay based on pressure level
                            let pressure_ratio = (usage_percent - cfg.throttle_threshold) / (100.0 - cfg.throttle_threshold);
                            let throttle_delay_ms = (pressure_ratio * cfg.throttle_factor * 100.0) as u64;
                            
                            if throttle_delay_ms > 0 {
                                let throttle_delay = Duration::from_millis(throttle_delay_ms);
                                std::thread::sleep(throttle_delay);
                            }
                        }
                    }
                }
            }
        }
        
        // Apply backoff delay once if enabled and memory pressure exists
        if let (Some(tracker), Ok(backoff)) = (&self.memory_tracker, self.backoff.lock()) {
            if backoff.is_enabled() {
                let pressure_level = tracker.get_pressure_level();
                let usage_percent = tracker.usage_percent();
                let _delay = backoff.apply_backoff(pressure_level, usage_percent);
            }
        }
        
        // Try to enqueue the message once
        match self.enqueue(message) {
            Ok(()) => {
                // Success - reset backoff level on successful enqueue
                if let Ok(backoff) = self.backoff.lock() {
                    if backoff.is_enabled() {
                        backoff.reset_backoff_level();
                    }
                }
                
                // Record recovery event if pressure response was active
                if self.pressure_response_enabled.load(Ordering::Relaxed) {
                    if let Some(tracker) = &self.memory_tracker {
                        let usage_percent = tracker.usage_percent();
                        if let Ok(config) = self.pressure_config.lock() {
                            if let Some(cfg) = config.as_ref() {
                                if usage_percent < cfg.throttle_threshold {
                                    if let Ok(mut metrics) = PRESSURE_METRICS.lock() {
                                        metrics.recovery_events += 1;
                                    }
                                }
                            }
                        }
                    }
                }
                
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
    
    // Pressure Response System Methods
    
    /// Enable pressure response system
    pub fn enable_pressure_response(&self) {
        self.pressure_response_enabled.store(true, Ordering::Relaxed);
    }
    
    /// Disable pressure response system
    pub fn disable_pressure_response(&self) {
        self.pressure_response_enabled.store(false, Ordering::Relaxed);
    }
    
    /// Configure pressure response parameters with validation
    pub fn configure_pressure_response(&self, config: PressureResponseConfig) -> Result<(), String> {
        config.validate()?;
        if let Ok(mut pressure_config) = self.pressure_config.lock() {
            *pressure_config = Some(config);
            Ok(())
        } else {
            Err("Failed to acquire pressure config lock".to_string())
        }
    }
    
    /// Configure pressure response parameters without validation (for compatibility)
    pub fn configure_pressure_response_unchecked(&self, config: PressureResponseConfig) {
        if let Ok(mut pressure_config) = self.pressure_config.lock() {
            *pressure_config = Some(config);
        }
    }
    
    /// Get pressure response status
    pub fn get_pressure_response_status(&self) -> PressureResponseStatus {
        let current_pressure = self.memory_usage_percent();
        let is_throttling = if let Ok(config) = self.pressure_config.lock() {
            if let Some(cfg) = config.as_ref() {
                current_pressure > cfg.throttle_threshold
            } else {
                false
            }
        } else {
            false
        };
        
        PressureResponseStatus {
            is_throttling,
            current_pressure_level: current_pressure,
        }
    }
    
    /// Get pressure response metrics
    pub fn get_pressure_response_metrics(&self) -> PressureResponseMetrics {
        if let Ok(metrics) = PRESSURE_METRICS.lock() {
            PressureResponseMetrics {
                messages_dropped: metrics.messages_dropped,
                throttle_events: metrics.throttle_events,
                recovery_events: metrics.recovery_events,
            }
        } else {
            PressureResponseMetrics {
                messages_dropped: 0,
                throttle_events: 0,
                recovery_events: 0,
            }
        }
    }
    
    /// Set extreme pressure mode for testing
    pub fn set_extreme_pressure_mode(&self, enabled: bool) {
        self.extreme_pressure_mode.store(enabled, Ordering::Relaxed);
    }
    
    /// Check if memory usage is recovering (trending downward)
    pub fn is_memory_recovering(&self) -> bool {
        if let Ok(history) = self.memory_history.lock() {
            history.is_recovering()
        } else {
            false
        }
    }
    
    /// Get memory limit
    pub fn memory_limit(&self) -> usize {
        self.memory_limit
    }
    
    /// Check if backoff is enabled
    pub fn is_backoff_enabled(&self) -> bool {
        if let Ok(backoff) = self.backoff.lock() {
            backoff.is_enabled()
        } else {
            false
        }
    }
    
    /// Check if pressure response is enabled
    pub fn is_pressure_response_enabled(&self) -> bool {
        self.pressure_response_enabled.load(Ordering::Relaxed)
    }
}

impl Queue<ScanMessage> for MemoryQueue {
    fn enqueue(&self, message: ScanMessage) -> Result<(), QueueError> {
        // Check capacity limit
        let current_size = self.current_size.load(Ordering::Relaxed);
        if current_size >= self.capacity {
            return Err(QueueError::QueueFull);
        }
        
        // Check memory limit if tracking is enabled
        let message_size = Self::estimate_message_size(&message);
        if let Some(tracker) = &self.memory_tracker {
            if !tracker.allocate(message_size) {
                return Err(QueueError::MemoryLimitExceeded);
            }
        }
        
        // Track individual queue contribution
        self.individual_memory_usage.fetch_add(message_size, Ordering::Relaxed);
        
        // Enqueue the message
        self.inner.push(message);
        self.current_size.fetch_add(1, Ordering::Relaxed);
        
        Ok(())
    }
    
    fn dequeue(&self) -> Result<Option<ScanMessage>, QueueError> {
        match self.inner.pop() {
            Some(message) => {
                self.current_size.fetch_sub(1, Ordering::Relaxed);
                
                let message_size = Self::estimate_message_size(&message);
                
                // Update memory tracking if enabled  
                if let Some(tracker) = &self.memory_tracker {
                    tracker.deallocate(message_size);
                }
                
                // Update individual queue contribution
                self.individual_memory_usage.fetch_sub(message_size, Ordering::Relaxed);
                
                Ok(Some(message))
            }
            None => Ok(None), // Empty queue, not an error
        }
    }
    
    fn size(&self) -> usize {
        self.current_size.load(Ordering::Relaxed)
    }
    
    fn is_empty(&self) -> bool {
        self.size() == 0
    }
    
    fn capacity(&self) -> usize {
        self.capacity
    }
}

impl VersionedMemoryQueue {
    /// Get current memory usage in bytes
    pub fn memory_usage(&self) -> usize {
        match &self.memory_tracker {
            Some(tracker) => tracker.allocated_bytes(),
            None => 0,
        }
    }
    
    /// Get memory usage as percentage of limit
    pub fn memory_usage_percent(&self) -> f64 {
        match &self.memory_tracker {
            Some(tracker) => tracker.usage_percent(),
            None => 0.0,
        }
    }
    
    /// Check if memory usage exceeds given threshold percentage
    pub fn exceeds_memory_threshold(&self, threshold_percent: f64) -> bool {
        match &self.memory_tracker {
            Some(tracker) => tracker.exceeds_threshold(threshold_percent),
            None => false,
        }
    }
    
    /// Get current queue size
    pub fn size(&self) -> usize {
        self.current_size.load(Ordering::Relaxed)
    }
    
    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.size() == 0
    }
    
    /// Get queue capacity
    pub fn capacity(&self) -> usize {
        self.capacity
    }
    
    /// Enqueue a versioned message
    pub fn enqueue_versioned(&self, message: QueueMessage) -> Result<(), QueueError> {
        // Check version compatibility
        if !message.is_version_compatible() {
            return Err(QueueError::VersioningError(format!(
                "Incompatible message version: {}", message.version
            )));
        }
        
        // Check capacity limit
        let current_size = self.current_size.load(Ordering::Relaxed);
        if current_size >= self.capacity {
            return Err(QueueError::QueueFull);
        }
        
        // Check memory limit if tracking is enabled
        if let Some(tracker) = &self.memory_tracker {
            let message_size = Self::estimate_versioned_message_size(&message);
            if !tracker.allocate(message_size) {
                return Err(QueueError::MemoryLimitExceeded);
            }
        }
        
        // Enqueue the message
        self.inner.push(message);
        self.current_size.fetch_add(1, Ordering::Relaxed);
        
        Ok(())
    }
    
    /// Dequeue a versioned message
    pub fn dequeue_versioned(&self) -> Result<Option<QueueMessage>, QueueError> {
        match self.inner.pop() {
            Some(message) => {
                self.current_size.fetch_sub(1, Ordering::Relaxed);
                
                // Update memory tracking if enabled
                if let Some(tracker) = &self.memory_tracker {
                    let message_size = Self::estimate_versioned_message_size(&message);
                    tracker.deallocate(message_size);
                }
                
                Ok(Some(message))
            }
            None => Ok(None), // Empty queue, not an error
        }
    }
    
    /// Estimate memory size of a versioned message
    fn estimate_versioned_message_size(message: &QueueMessage) -> usize {
        // Simple estimation - in practice this could be more sophisticated
        std::mem::size_of::<QueueMessage>() + 
        bincode::serialized_size(message).unwrap_or(512) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::messages::{MessageHeader, MessageData};
    use crate::scanner::modes::ScanMode;

    #[test]
    fn test_memory_queue_creation_basic() {
        let queue = MemoryQueue::new(100, 1024 * 1024);
        assert_eq!(queue.capacity(), 100);
        // Other assertions will fail until we implement the methods
    }

    fn create_test_message() -> ScanMessage {
        ScanMessage::new(
            MessageHeader::new(ScanMode::FILES, 12345),
            MessageData::FileInfo {
                path: "test.rs".to_string(),
                size: 1024,
                lines: 50,
            }
        )
    }
}