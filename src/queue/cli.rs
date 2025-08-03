//! CLI Integration for Queue System
//! 
//! Provides command-line argument support for queue configuration
//! and debugging commands.

use clap::Args;
use crate::queue::{Queue, QueueConfig, QueuePreset, QueueDebug};

/// Queue-related command line arguments
#[derive(Debug, Args)]
pub struct QueueArgs {
    /// Queue capacity (number of messages)
    #[arg(long, value_name = "COUNT")]
    pub queue_capacity: Option<usize>,
    
    /// Queue memory limit in MB
    #[arg(long, value_name = "MB")]
    pub queue_memory_mb: Option<usize>,
    
    /// Use a queue configuration preset
    #[arg(long, value_enum)]
    pub queue_preset: Option<QueuePresetArg>,
    
    /// Enable queue debug logging
    #[arg(long)]
    pub queue_debug: bool,
    
    /// Queue status logging interval in seconds
    #[arg(long, value_name = "SECS", default_value = "60")]
    pub queue_status_interval: u64,
    
    /// Disable queue backoff algorithm
    #[arg(long)]
    pub no_queue_backoff: bool,
    
    /// Disable queue pressure response
    #[arg(long)]
    pub no_queue_pressure_response: bool,
}

/// Queue preset command line argument
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum QueuePresetArg {
    /// Small queue for development
    Dev,
    /// Balanced configuration
    Balanced,
    /// High throughput configuration
    HighThroughput,
    /// Low memory configuration
    LowMemory,
}

impl From<QueuePresetArg> for QueuePreset {
    fn from(arg: QueuePresetArg) -> Self {
        match arg {
            QueuePresetArg::Dev => QueuePreset::Development,
            QueuePresetArg::Balanced => QueuePreset::Balanced,
            QueuePresetArg::HighThroughput => QueuePreset::HighThroughput,
            QueuePresetArg::LowMemory => QueuePreset::LowMemory,
        }
    }
}

impl QueueArgs {
    /// Apply command line arguments to a queue configuration
    pub fn apply_to_config(&self, config: &mut QueueConfig) {
        // Apply preset first if specified
        if let Some(preset_arg) = self.queue_preset {
            let preset_config = QueuePreset::from(preset_arg).config();
            *config = preset_config;
        }
        
        // Override with specific arguments
        if let Some(capacity) = self.queue_capacity {
            config.capacity = capacity;
        }
        
        if let Some(memory_mb) = self.queue_memory_mb {
            config.memory_limit = memory_mb * 1024 * 1024;
        }
        
        if self.queue_debug {
            config.debug.enabled = true;
        }
        
        config.debug.status_interval_secs = self.queue_status_interval;
        
        if self.no_queue_backoff {
            config.enable_backoff = false;
        }
        
        if self.no_queue_pressure_response {
            config.enable_pressure_response = false;
        }
    }
    
    /// Create a queue configuration from command line arguments
    pub fn to_config(&self) -> QueueConfig {
        let mut config = QueueConfig::default();
        self.apply_to_config(&mut config);
        config
    }
}

/// Queue debugging subcommands
#[derive(Debug, clap::Subcommand)]
pub enum QueueCommand {
    /// Show queue status
    Status,
    
    /// Show queue configuration
    Config,
    
    /// Show queue metrics
    Metrics,
    
    /// Reset queue metrics
    ResetMetrics,
}

/// Handle queue debugging commands
pub fn handle_queue_command(cmd: QueueCommand, integration: &crate::queue::ScannerQueueIntegration) {
    match cmd {
        QueueCommand::Status => {
            println!("{}", integration.status());
        }
        
        QueueCommand::Config => {
            let queue = integration.queue();
            println!("Queue Configuration:");
            println!("  Capacity: {}", queue.capacity());
            println!("  Memory Limit: {:.2}MB", queue.memory_limit() as f64 / 1024.0 / 1024.0);
            println!("  Backoff Enabled: {}", queue.is_backoff_enabled());
            println!("  Pressure Response Enabled: {}", queue.is_pressure_response_enabled());
        }
        
        QueueCommand::Metrics => {
            let queue = integration.queue();
            println!("{}", queue.debug_info());
            
            if let Some(consumer) = integration.get_consumer() {
                let metrics = consumer.get_metrics();
                println!("\nConsumer Metrics:");
                println!("  Messages Processed: {}", metrics.messages_processed);
                println!("  Notifications Sent: {}", metrics.notifications_sent);
                println!("  Notification Errors: {}", metrics.notification_errors);
                println!("  Average Batch Size: {:.2}", metrics.average_batch_size);
                println!("  Average Notification Latency: {:?}", metrics.average_notification_latency);
            }
        }
        
        QueueCommand::ResetMetrics => {
            // This would need to be implemented in the queue/consumer
            println!("Metrics reset functionality not yet implemented");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_queue_args_to_config() {
        let args = QueueArgs {
            queue_capacity: Some(5000),
            queue_memory_mb: Some(50),
            queue_preset: None,
            queue_debug: true,
            queue_status_interval: 30,
            no_queue_backoff: false,
            no_queue_pressure_response: false,
        };
        
        let config = args.to_config();
        assert_eq!(config.capacity, 5000);
        assert_eq!(config.memory_limit, 50 * 1024 * 1024);
        assert!(config.debug.enabled);
        assert_eq!(config.debug.status_interval_secs, 30);
        assert!(config.enable_backoff);
        assert!(config.enable_pressure_response);
    }

    #[test]
    fn test_preset_application() {
        let args = QueueArgs {
            queue_capacity: None,
            queue_memory_mb: None,
            queue_preset: Some(QueuePresetArg::Dev),
            queue_debug: false,
            queue_status_interval: 60,
            no_queue_backoff: false,
            no_queue_pressure_response: false,
        };
        
        let config = args.to_config();
        assert_eq!(config.capacity, 1000); // Dev preset
        assert_eq!(config.memory_limit, 10 * 1024 * 1024); // Dev preset
    }

    #[test]
    fn test_override_preset() {
        let args = QueueArgs {
            queue_capacity: Some(2000), // Override preset
            queue_memory_mb: None,
            queue_preset: Some(QueuePresetArg::Dev),
            queue_debug: false,
            queue_status_interval: 60,
            no_queue_backoff: false,
            no_queue_pressure_response: false,
        };
        
        let config = args.to_config();
        assert_eq!(config.capacity, 2000); // Overridden
        assert_eq!(config.memory_limit, 10 * 1024 * 1024); // From preset
    }
}