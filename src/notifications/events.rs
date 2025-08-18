//! Notification Event Types
//! 
//! Defines the various event types that can flow through the notification system.
//! Events are strongly typed and implement the NotificationEvent trait.

use std::sync::Arc;
use std::time::{Duration, SystemTime};
use serde::{Deserialize, Serialize};
use crate::plugin::data_export::PluginDataExport;

/// Base trait for all notification events
pub trait NotificationEvent: Send + Sync + Clone + std::fmt::Debug + 'static {}

/// Scanner lifecycle and progress events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScanEvent {
    /// Scanning process started
    ScanStarted {
        scan_id: String,
    },
    
    /// Scanning progress update
    ScanProgress {
        scan_id: String,
        progress: f64,
        phase: String,
    },
    
    /// Recoverable warning during scanning
    ScanWarning {
        scan_id: String,
        warning: String,
        recoverable: bool,
    },
    
    /// Data is ready for plugin processing
    ScanDataReady {
        scan_id: String,
        data_type: String,
        message_count: usize,
    },
    
    /// Scanning completed successfully
    ScanCompleted {
        scan_id: String,
        duration: Duration,
        warnings: Vec<String>,
    },
    
    /// Scanning encountered an error
    ScanError {
        scan_id: String,
        error: String,
        fatal: bool,
    },
    
    /// Processed data ready for export
    DataReady {
        scan_id: String,
        plugin_id: String,
        data_type: String,
    },
}

/// Queue state and message events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QueueEvent {
    /// Message added to queue
    MessageAdded {
        queue_id: String,
        message_type: String,
        queue_size: usize,
        memory_usage_bytes: u64,
        added_at: SystemTime,
    },
    
    /// Message processed from queue
    MessageProcessed {
        queue_id: String,
        message_type: String,
        processing_time_ms: u64,
        remaining_messages: usize,
        processed_at: SystemTime,
    },
    
    /// Queue is empty
    QueueEmpty {
        queue_id: String,
        last_message_processed_at: Option<SystemTime>,
        total_processed: u64,
        emptied_at: SystemTime,
    },
    
    /// Queue is full (backpressure)
    QueueFull {
        queue_id: String,
        capacity: usize,
        memory_limit_bytes: Option<u64>,
        oldest_message_age_ms: Option<u64>,
        full_at: SystemTime,
    },
    
    /// Memory pressure detected
    MemoryPressure {
        queue_id: String,
        current_usage_bytes: u64,
        limit_bytes: u64,
        pressure_level: MemoryPressureLevel,
        detected_at: SystemTime,
    },
    
    /// Consumer registered with queue
    ConsumerRegistered {
        queue_id: String,
        consumer_id: String,
        plugin_name: String,
        total_consumers: usize,
        registered_at: SystemTime,
    },
    
    /// Consumer deregistered from queue
    ConsumerDeregistered {
        queue_id: String,
        consumer_id: String,
        plugin_name: String,
        total_consumers: usize,
        deregistered_at: SystemTime,
    },
}

/// Plugin lifecycle and coordination events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginEvent {
    /// Plugin started processing
    PluginStarted {
        plugin_id: String,
        plugin_type: String,
        started_at: SystemTime,
    },
    
    /// Plugin completed processing
    PluginCompleted {
        plugin_id: String,
        processing_time: Duration,
        items_processed: u64,
        results_generated: usize,
        completed_at: SystemTime,
    },
    
    /// Plugin generated results
    ResultsReady {
        plugin_id: String,
        result_type: String,
        result_count: usize,
        data_size_bytes: Option<u64>,
        ready_at: SystemTime,
    },
    
    /// Plugin encountered an error
    PluginError {
        plugin_id: String,
        error_type: String,
        error_message: String,
        recoverable: bool,
        occurred_at: SystemTime,
    },
    
    /// Plugin state changed
    PluginStateChanged {
        plugin_id: String,
        old_state: String,
        new_state: String,
        changed_at: SystemTime,
    },
    
    /// Plugin has data ready for export
    DataReady {
        plugin_id: String,
        scan_id: String,
        export: Arc<PluginDataExport>,
    },
}

/// System-wide events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SystemEvent {
    /// System startup
    SystemStartup {
        version: String,
        started_at: SystemTime,
    },
    
    /// System shutdown initiated
    SystemShutdown {
        reason: String,
        graceful: bool,
        shutdown_at: SystemTime,
    },
    
    /// Configuration changed
    ConfigurationChanged {
        config_section: String,
        changed_keys: Vec<String>,
        changed_at: SystemTime,
    },
    
    /// Resource usage warning
    ResourceWarning {
        resource_type: String,
        current_usage: f64,
        threshold: f64,
        unit: String,
        detected_at: SystemTime,
    },
}

/// Memory pressure levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MemoryPressureLevel {
    /// Normal memory usage
    Normal,
    
    /// Moderate pressure - should start cleanup
    Moderate,
    
    /// High pressure - aggressive cleanup needed
    High,
    
    /// Critical pressure - emergency measures
    Critical,
}

// Implement NotificationEvent for all event types
impl NotificationEvent for ScanEvent {}
impl NotificationEvent for QueueEvent {}
impl NotificationEvent for PluginEvent {}
impl NotificationEvent for SystemEvent {}

/// Combined event type for systems that need to handle multiple event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UnifiedEvent {
    Scan(ScanEvent),
    Queue(QueueEvent),
    Plugin(PluginEvent),
    System(SystemEvent),
}

impl NotificationEvent for UnifiedEvent {}

impl From<ScanEvent> for UnifiedEvent {
    fn from(event: ScanEvent) -> Self {
        UnifiedEvent::Scan(event)
    }
}

impl From<QueueEvent> for UnifiedEvent {
    fn from(event: QueueEvent) -> Self {
        UnifiedEvent::Queue(event)
    }
}

impl From<PluginEvent> for UnifiedEvent {
    fn from(event: PluginEvent) -> Self {
        UnifiedEvent::Plugin(event)
    }
}

impl From<SystemEvent> for UnifiedEvent {
    fn from(event: SystemEvent) -> Self {
        UnifiedEvent::System(event)
    }
}

/// Helper functions for creating common events
impl ScanEvent {
    /// Create a scan started event
    pub fn started(scan_id: String) -> Self {
        Self::ScanStarted {
            scan_id,
        }
    }
    
    /// Create a scan completed event
    pub fn completed(scan_id: String, duration: Duration, warnings: Vec<String>) -> Self {
        Self::ScanCompleted {
            scan_id,
            duration,
            warnings,
        }
    }
    
    /// Create a scan progress event
    pub fn progress(scan_id: String, progress: f64, phase: String) -> Self {
        Self::ScanProgress {
            scan_id,
            progress,
            phase,
        }
    }
    
    /// Create a scan data ready event
    pub fn scan_data_ready(scan_id: String, data_type: String, message_count: usize) -> Self {
        Self::ScanDataReady {
            scan_id,
            data_type,
            message_count,
        }
    }
    
    
    
    /// Create a scan error event
    pub fn error(scan_id: String, error: String, fatal: bool) -> Self {
        Self::ScanError {
            scan_id,
            error,
            fatal,
        }
    }
}

impl QueueEvent {
}

