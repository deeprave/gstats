//! Queue Configuration Module
//! 
//! Provides configuration structures and parsing for the queue system,
//! integrating with the application's configuration discovery.

use serde::{Deserialize, Serialize};
use std::time::Duration;
use crate::queue::{BackoffConfig, BackoffStrategy, PressureResponseConfig, ConsumerConfig};

/// Main queue configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct QueueConfig {
    /// Queue capacity (number of messages)
    pub capacity: usize,
    
    /// Memory limit in bytes
    pub memory_limit: usize,
    
    /// Enable memory tracking
    pub enable_memory_tracking: bool,
    
    /// Enable backoff algorithm
    pub enable_backoff: bool,
    
    /// Backoff configuration
    pub backoff: BackoffConfigSerde,
    
    /// Enable pressure response
    pub enable_pressure_response: bool,
    
    /// Pressure response configuration
    pub pressure_response: PressureResponseConfigSerde,
    
    /// Consumer configuration
    pub consumer: ConsumerConfigSerde,
    
    /// Debug logging configuration
    pub debug: DebugConfig,
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            capacity: 10000,
            memory_limit: 100 * 1024 * 1024, // 100MB
            enable_memory_tracking: true,
            enable_backoff: true,
            backoff: BackoffConfigSerde::default(),
            enable_pressure_response: true,
            pressure_response: PressureResponseConfigSerde::default(),
            consumer: ConsumerConfigSerde::default(),
            debug: DebugConfig::default(),
        }
    }
}

/// Serializable backoff configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BackoffConfigSerde {
    pub strategy: String,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
    pub multiplier: f64,
    pub max_retries: u32,
    pub memory_pressure_threshold: f64,
    
    // Strategy-specific fields
    pub increment_ms: Option<u64>,
    pub success_factor: Option<f64>,
    pub failure_factor: Option<f64>,
    pub memory_recovery_factor: Option<f64>,
}

impl Default for BackoffConfigSerde {
    fn default() -> Self {
        Self {
            strategy: "exponential".to_string(),
            initial_delay_ms: 10,
            max_delay_ms: 5000,
            multiplier: 2.0,
            max_retries: 10,
            memory_pressure_threshold: 80.0,
            increment_ms: None,
            success_factor: None,
            failure_factor: None,
            memory_recovery_factor: None,
        }
    }
}

impl BackoffConfigSerde {
    /// Convert to BackoffConfig and BackoffStrategy
    pub fn to_backoff_config(&self) -> (BackoffConfig, BackoffStrategy) {
        let config = BackoffConfig {
            initial_delay_ms: self.initial_delay_ms,
            max_delay_ms: self.max_delay_ms,
            multiplier: self.multiplier,
            max_retries: self.max_retries,
            memory_pressure_threshold: self.memory_pressure_threshold,
        };
        
        let strategy = match self.strategy.as_str() {
            "linear" => BackoffStrategy::Linear {
                base_delay_ms: self.initial_delay_ms,
                increment_ms: self.increment_ms.unwrap_or(10),
                max_delay_ms: self.max_delay_ms,
            },
            "adaptive" => BackoffStrategy::Adaptive {
                initial_delay_ms: self.initial_delay_ms,
                success_factor: self.success_factor.unwrap_or(0.8),
                failure_factor: self.failure_factor.unwrap_or(1.5),
                memory_recovery_factor: self.memory_recovery_factor.unwrap_or(0.6),
            },
            _ => BackoffStrategy::Exponential {
                base_delay_ms: self.initial_delay_ms,
                multiplier: self.multiplier,
                max_delay_ms: self.max_delay_ms,
            },
        };
        
        (config, strategy)
    }
}

/// Serializable pressure response configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PressureResponseConfigSerde {
    pub throttle_threshold: f64,
    pub drop_threshold: f64,
    pub throttle_factor: f64,
    pub recovery_factor: f64,
}

impl Default for PressureResponseConfigSerde {
    fn default() -> Self {
        Self {
            throttle_threshold: 75.0,
            drop_threshold: 90.0,
            throttle_factor: 0.5,
            recovery_factor: 0.8,
        }
    }
}

impl From<PressureResponseConfigSerde> for PressureResponseConfig {
    fn from(serde_config: PressureResponseConfigSerde) -> Self {
        PressureResponseConfig {
            throttle_threshold: serde_config.throttle_threshold,
            drop_threshold: serde_config.drop_threshold,
            throttle_factor: serde_config.throttle_factor,
            recovery_factor: serde_config.recovery_factor,
        }
    }
}

/// Serializable consumer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ConsumerConfigSerde {
    pub poll_interval_ms: u64,
    pub batch_size: usize,
    pub notification_timeout_ms: u64,
    pub continue_on_error: bool,
}

impl Default for ConsumerConfigSerde {
    fn default() -> Self {
        Self {
            poll_interval_ms: 10,
            batch_size: 100,
            notification_timeout_ms: 1000,
            continue_on_error: true,
        }
    }
}

impl From<ConsumerConfigSerde> for ConsumerConfig {
    fn from(serde_config: ConsumerConfigSerde) -> Self {
        ConsumerConfig {
            poll_interval_ms: serde_config.poll_interval_ms,
            batch_size: serde_config.batch_size,
            notification_timeout_ms: serde_config.notification_timeout_ms,
            continue_on_error: serde_config.continue_on_error,
        }
    }
}

/// Debug configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DebugConfig {
    /// Enable debug logging
    pub enabled: bool,
    
    /// Status logging interval in seconds
    pub status_interval_secs: u64,
    
    /// Log memory usage details
    pub log_memory_usage: bool,
    
    /// Log consumer metrics
    pub log_consumer_metrics: bool,
}

impl Default for DebugConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            status_interval_secs: 60,
            log_memory_usage: true,
            log_consumer_metrics: true,
        }
    }
}

/// Queue configuration preset
#[derive(Debug, Clone, Copy)]
pub enum QueuePreset {
    /// Small queue for development/testing
    Development,
    /// Balanced configuration for normal use
    Balanced,
    /// Large queue for high-throughput scenarios
    HighThroughput,
    /// Memory-constrained configuration
    LowMemory,
}

impl QueuePreset {
    /// Get queue configuration for this preset
    pub fn config(self) -> QueueConfig {
        match self {
            QueuePreset::Development => QueueConfig {
                capacity: 1000,
                memory_limit: 10 * 1024 * 1024, // 10MB
                ..Default::default()
            },
            
            QueuePreset::Balanced => QueueConfig::default(),
            
            QueuePreset::HighThroughput => QueueConfig {
                capacity: 50000,
                memory_limit: 500 * 1024 * 1024, // 500MB
                consumer: ConsumerConfigSerde {
                    batch_size: 500,
                    ..Default::default()
                },
                ..Default::default()
            },
            
            QueuePreset::LowMemory => QueueConfig {
                capacity: 5000,
                memory_limit: 50 * 1024 * 1024, // 50MB
                backoff: BackoffConfigSerde {
                    strategy: "aggressive".to_string(),
                    memory_pressure_threshold: 70.0,
                    ..Default::default()
                },
                pressure_response: PressureResponseConfigSerde {
                    throttle_threshold: 60.0,
                    drop_threshold: 80.0,
                    ..Default::default()
                },
                ..Default::default()
            },
        }
    }
}

/// Load queue configuration from TOML string
pub fn load_queue_config(toml_str: &str) -> Result<QueueConfig, toml::de::Error> {
    toml::from_str(toml_str)
}

/// Save queue configuration to TOML string
pub fn save_queue_config(config: &QueueConfig) -> Result<String, toml::ser::Error> {
    toml::to_string_pretty(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = QueueConfig::default();
        assert_eq!(config.capacity, 10000);
        assert_eq!(config.memory_limit, 100 * 1024 * 1024);
        assert!(config.enable_memory_tracking);
        assert!(config.enable_backoff);
    }

    #[test]
    fn test_config_serialization() {
        let config = QueueConfig::default();
        let toml_str = save_queue_config(&config).unwrap();
        assert!(toml_str.contains("capacity = 10000"));
        assert!(toml_str.contains("memory_limit = 104857600"));
        
        let loaded_config = load_queue_config(&toml_str).unwrap();
        assert_eq!(loaded_config.capacity, config.capacity);
        assert_eq!(loaded_config.memory_limit, config.memory_limit);
    }

    #[test]
    fn test_backoff_config_conversion() {
        let serde_config = BackoffConfigSerde {
            strategy: "linear".to_string(),
            initial_delay_ms: 20,
            increment_ms: Some(15),
            ..Default::default()
        };
        
        let (config, strategy) = serde_config.to_backoff_config();
        assert_eq!(config.initial_delay_ms, 20);
        
        match strategy {
            BackoffStrategy::Linear { base_delay_ms, increment_ms, .. } => {
                assert_eq!(base_delay_ms, 20);
                assert_eq!(increment_ms, 15);
            }
            _ => panic!("Expected linear strategy"),
        }
    }

    #[test]
    fn test_presets() {
        let dev_config = QueuePreset::Development.config();
        assert_eq!(dev_config.capacity, 1000);
        assert_eq!(dev_config.memory_limit, 10 * 1024 * 1024);
        
        let high_throughput = QueuePreset::HighThroughput.config();
        assert_eq!(high_throughput.capacity, 50000);
        assert_eq!(high_throughput.consumer.batch_size, 500);
    }

    #[test]
    fn test_config_from_toml() {
        let toml_str = r#"
capacity = 5000
memory_limit = 52428800
enable_backoff = true

[backoff]
strategy = "adaptive"
initial_delay_ms = 15
memory_recovery_factor = 0.7

[consumer]
batch_size = 200
poll_interval_ms = 5

[debug]
enabled = true
status_interval_secs = 30
"#;
        
        let config = load_queue_config(toml_str).unwrap();
        assert_eq!(config.capacity, 5000);
        assert_eq!(config.memory_limit, 52428800);
        assert_eq!(config.backoff.strategy, "adaptive");
        assert_eq!(config.backoff.memory_recovery_factor, Some(0.7));
        assert_eq!(config.consumer.batch_size, 200);
        assert!(config.debug.enabled);
        assert_eq!(config.debug.status_interval_secs, 30);
    }
}