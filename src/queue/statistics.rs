//! Queue Statistics Management
//!
//! This module provides statistics tracking for both the overall queue
//! and individual scan sessions. It supports multi-scanner environments
//! where each scan has its own statistical tracking.

use std::time::{Duration, Instant};

/// Statistics for the multi-consumer queue
#[derive(Debug, Clone)]
pub struct QueueStatistics {
    /// Current queue size (number of messages)
    pub queue_size: usize,
    
    /// Current memory usage (bytes)
    pub memory_usage: u64,
    
    /// Number of active consumers
    pub active_consumers: usize,
    
    /// Total messages processed
    pub total_messages: u64,
}

/// Per-scan statistics and state tracking
#[derive(Debug, Clone)]
pub struct ScanStatistics {
    /// Whether this scan is active
    pub active: bool,
    
    /// Accumulated message count for this scan
    pub accumulated_message_count: usize,
    
    /// Scan start timestamp
    pub scan_start_time: Option<Instant>,
    
    /// Scan completion timestamp
    pub scan_end_time: Option<Instant>,
    
    /// Total messages processed for this scan
    pub total_messages: u64,
}

impl ScanStatistics {
    pub fn new() -> Self {
        Self {
            active: false,
            accumulated_message_count: 0,
            scan_start_time: None,
            scan_end_time: None,
            total_messages: 0,
        }
    }
    
    pub fn start_scan(&mut self) {
        self.active = true;
        self.scan_start_time = Some(Instant::now());
        self.scan_end_time = None;
    }
    
    pub fn complete_scan(&mut self) {
        self.active = false;
        self.scan_end_time = Some(Instant::now());
    }
    
    pub fn duration(&self) -> Option<Duration> {
        match (self.scan_start_time, self.scan_end_time) {
            (Some(start), Some(end)) => Some(end.duration_since(start)),
            (Some(start), None) => Some(Instant::now().duration_since(start)),
            _ => None,
        }
    }
}

impl Default for ScanStatistics {
    fn default() -> Self {
        Self::new()
    }
}