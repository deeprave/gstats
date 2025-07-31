//! Backoff Algorithms and Strategies
//! 
//! Provides adaptive backoff algorithms for memory pressure situations

use std::time::{Duration, Instant};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use crate::queue::memory_tracker::MemoryPressureLevel;

/// Configuration for backoff behavior
#[derive(Debug, Clone)]
pub struct BackoffConfig {
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
    pub multiplier: f64,
    pub max_retries: u32,
    pub memory_pressure_threshold: f64,
}

impl Default for BackoffConfig {
    fn default() -> Self {
        Self {
            initial_delay_ms: 10,
            max_delay_ms: 5000,
            multiplier: 2.0,
            max_retries: 10,
            memory_pressure_threshold: 80.0,
        }
    }
}

impl BackoffConfig {
    /// Create a conservative backoff configuration (longer delays, more retries)
    pub fn conservative() -> Self {
        Self {
            initial_delay_ms: 50,
            max_delay_ms: 10000,
            multiplier: 2.5,
            max_retries: 15,
            memory_pressure_threshold: 70.0,
        }
    }
    
    /// Create an aggressive backoff configuration (shorter delays, fewer retries)
    pub fn aggressive() -> Self {
        Self {
            initial_delay_ms: 5,
            max_delay_ms: 1000,
            multiplier: 1.5,
            max_retries: 5,
            memory_pressure_threshold: 90.0,
        }
    }
    
    /// Create a balanced backoff configuration (moderate settings)
    pub fn balanced() -> Self {
        Self {
            initial_delay_ms: 20,
            max_delay_ms: 3000,
            multiplier: 2.0,
            max_retries: 8,
            memory_pressure_threshold: 75.0,
        }
    }
    
    /// Validate backoff configuration parameters
    pub fn validate(&self) -> Result<(), String> {
        if self.initial_delay_ms == 0 {
            return Err("Initial delay must be greater than 0".to_string());
        }
        
        if self.max_delay_ms < self.initial_delay_ms {
            return Err("Max delay must be greater than or equal to initial delay".to_string());
        }
        
        if self.multiplier <= 1.0 {
            return Err("Multiplier must be greater than 1.0".to_string());
        }
        
        if self.max_retries == 0 {
            return Err("Max retries must be greater than 0".to_string());
        }
        
        if self.memory_pressure_threshold < 0.0 || self.memory_pressure_threshold > 100.0 {
            return Err("Memory pressure threshold must be between 0.0 and 100.0".to_string());
        }
        
        Ok(())
    }
}

/// Different backoff strategy implementations
#[derive(Debug, Clone)]
pub enum BackoffStrategy {
    Exponential {
        base_delay_ms: u64,
        multiplier: f64,
        max_delay_ms: u64,
    },
    Linear {
        base_delay_ms: u64,
        increment_ms: u64,
        max_delay_ms: u64,
    },
    Adaptive {
        initial_delay_ms: u64,
        success_factor: f64,
        failure_factor: f64,
        memory_recovery_factor: f64,
    },
}

impl Default for BackoffStrategy {
    fn default() -> Self {
        Self::Exponential {
            base_delay_ms: 10,
            multiplier: 2.0,
            max_delay_ms: 5000,
        }
    }
}

/// Backoff algorithm implementation
pub struct BackoffAlgorithm {
    config: BackoffConfig,
    strategy: BackoffStrategy,
    current_level: AtomicU32,
    total_events: AtomicU64,
    total_duration: AtomicU64,
    last_success_time: Arc<Mutex<Option<Instant>>>,
    enabled: bool,
}

/// Metrics for backoff events
#[derive(Debug)]
pub struct BackoffMetrics {
    pub total_backoff_events: u64,
    pub total_backoff_duration: Duration,
    pub average_backoff_delay: Duration,
    pub current_backoff_level: u32,
}

impl BackoffAlgorithm {
    pub fn new(config: BackoffConfig) -> Self {
        Self {
            config,
            strategy: BackoffStrategy::default(),
            current_level: AtomicU32::new(0),
            total_events: AtomicU64::new(0),
            total_duration: AtomicU64::new(0),
            last_success_time: Arc::new(Mutex::new(None)),
            enabled: false,
        }
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_strategy(&mut self, strategy: BackoffStrategy) {
        self.strategy = strategy;
    }

    pub fn set_config(&mut self, config: BackoffConfig) {
        self.config = config;
    }

    /// Calculate and apply backoff delay based on current state
    pub fn apply_backoff(&self, memory_pressure: MemoryPressureLevel, memory_usage_percent: f64) -> Duration {
        if !self.enabled {
            return Duration::from_millis(0);
        }

        // Check if backoff should be triggered
        if !self.should_trigger_backoff(memory_pressure, memory_usage_percent) {
            self.reset_level_internal();
            return Duration::from_millis(0);
        }

        let level = self.current_level.load(Ordering::Relaxed);
        let delay = self.calculate_delay_with_memory_context(level, memory_usage_percent);

        // Apply the delay
        std::thread::sleep(delay);

        // Update metrics
        self.record_backoff_event(delay);

        // Increment backoff level for next time
        self.increment_backoff_level();

        delay
    }

    fn should_trigger_backoff(&self, pressure: MemoryPressureLevel, usage_percent: f64) -> bool {
        match pressure {
            MemoryPressureLevel::Normal => false,
            MemoryPressureLevel::Moderate => usage_percent > self.config.memory_pressure_threshold,
            MemoryPressureLevel::High | MemoryPressureLevel::Critical => true,
        }
    }

    fn calculate_delay(&self, level: u32) -> Duration {
        self.calculate_delay_with_memory_context(level, 0.0)
    }

    fn calculate_delay_with_memory_context(&self, level: u32, memory_usage_percent: f64) -> Duration {
        match &self.strategy {
            BackoffStrategy::Exponential { base_delay_ms, multiplier, max_delay_ms } => {
                let delay_ms = (*base_delay_ms as f64 * multiplier.powi(level as i32)) as u64;
                Duration::from_millis(delay_ms.min(*max_delay_ms))
            }
            BackoffStrategy::Linear { base_delay_ms, increment_ms, max_delay_ms } => {
                let delay_ms = base_delay_ms + (increment_ms * level as u64);
                Duration::from_millis(delay_ms.min(*max_delay_ms))
            }
            BackoffStrategy::Adaptive { initial_delay_ms, success_factor, failure_factor, memory_recovery_factor } => {
                let mut delay_ms = *initial_delay_ms;
                
                // Adjust based on recent success/failure
                if let Ok(last_success) = self.last_success_time.lock() {
                    if let Some(last_time) = *last_success {
                        let time_since_success = last_time.elapsed().as_millis() as u64;
                        if time_since_success < 1000 { // Recent success
                            delay_ms = (delay_ms as f64 * success_factor) as u64;
                        } else {
                            delay_ms = (delay_ms as f64 * failure_factor) as u64;
                        }
                    }
                }
                
                // Apply memory recovery factor based on current memory usage
                // If memory usage is decreasing (lower percentage), reduce backoff delay
                if memory_usage_percent > 0.0 && memory_usage_percent < self.config.memory_pressure_threshold {
                    // Memory pressure is decreasing, apply recovery factor
                    delay_ms = (delay_ms as f64 * memory_recovery_factor) as u64;
                }
                
                Duration::from_millis(delay_ms.min(self.config.max_delay_ms))
            }
        }
    }

    fn increment_backoff_level(&self) {
        let current = self.current_level.load(Ordering::Relaxed);
        if current < self.config.max_retries {
            self.current_level.store(current + 1, Ordering::Relaxed);
        }
    }

    fn reset_level_internal(&self) {
        self.current_level.store(0, Ordering::Relaxed);
        
        // Record successful operation
        if let Ok(mut last_success) = self.last_success_time.lock() {
            *last_success = Some(Instant::now());
        }
    }

    fn record_backoff_event(&self, delay: Duration) {
        self.total_events.fetch_add(1, Ordering::Relaxed);
        self.total_duration.fetch_add(delay.as_millis() as u64, Ordering::Relaxed);
    }

    pub fn get_metrics(&self) -> BackoffMetrics {
        let events = self.total_events.load(Ordering::Relaxed);
        let total_duration_ms = self.total_duration.load(Ordering::Relaxed);
        
        let average_delay = if events > 0 {
            Duration::from_millis(total_duration_ms / events)
        } else {
            Duration::from_millis(0)
        };

        BackoffMetrics {
            total_backoff_events: events,
            total_backoff_duration: Duration::from_millis(total_duration_ms),
            average_backoff_delay: average_delay,
            current_backoff_level: self.current_level.load(Ordering::Relaxed),
        }
    }

    pub fn reset_metrics(&self) {
        self.total_events.store(0, Ordering::Relaxed);
        self.total_duration.store(0, Ordering::Relaxed);
        self.current_level.store(0, Ordering::Relaxed);
    }

    /// Reset backoff level and record success (public method)
    pub fn reset_backoff_level(&self) {
        self.reset_level_internal();
    }

    /// Get max retries from config
    pub fn max_retries(&self) -> u32 {
        self.config.max_retries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backoff_algorithm_creation() {
        let config = BackoffConfig::default();
        let algorithm = BackoffAlgorithm::new(config);
        assert!(!algorithm.is_enabled());
    }

    #[test]
    fn test_exponential_backoff_calculation() {
        let config = BackoffConfig::default();
        let mut algorithm = BackoffAlgorithm::new(config);
        algorithm.enable();
        
        // Test exponential delay calculation
        let delay1 = algorithm.calculate_delay(0);
        let delay2 = algorithm.calculate_delay(1);
        let delay3 = algorithm.calculate_delay(2);
        
        assert!(delay2 > delay1);
        assert!(delay3 > delay2);
    }

    #[test]
    fn test_backoff_metrics() {
        let config = BackoffConfig::default();
        let algorithm = BackoffAlgorithm::new(config);
        
        let metrics = algorithm.get_metrics();
        assert_eq!(metrics.total_backoff_events, 0);
        assert_eq!(metrics.current_backoff_level, 0);
    }
}