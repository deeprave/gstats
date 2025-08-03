//! Memory Tracking and Monitoring
//! 
//! Provides memory usage tracking and monitoring for queue operations

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use std::collections::VecDeque;

/// Memory usage tracker for queue operations
pub struct MemoryTracker {
    allocated_bytes: AtomicUsize,
    max_bytes: usize,
    peak_bytes: AtomicUsize,
    allocation_count: AtomicUsize,
    deallocation_count: AtomicUsize,
    history: Arc<Mutex<Option<MemoryHistory>>>,
    leak_detection: Arc<Mutex<Option<LeakDetector>>>,
}

/// Memory usage history tracking
#[derive(Debug)]
struct MemoryHistory {
    samples: VecDeque<MemoryHistorySample>,
    max_samples: usize,
}

/// Single memory history sample
#[derive(Debug, Clone)]
pub struct MemoryHistorySample {
    pub timestamp: u64,
    pub bytes_allocated: usize,
}

/// Memory leak detection
#[derive(Debug)]
struct LeakDetector {
    allocation_pattern: VecDeque<(u64, usize)>, // (timestamp, size)
    deallocation_pattern: VecDeque<(u64, usize)>,
    window_size: usize,
}

/// Memory statistics snapshot
#[derive(Debug, Clone)]
pub struct MemoryStatistics {
    pub current_bytes: usize,
    pub peak_bytes: usize,
    pub total_allocations: usize,
    pub total_deallocations: usize,
    pub limit_bytes: usize,
    pub average_allocation_size: f64,
    pub fragmentation_ratio: f64,
}

/// Memory pressure levels
#[derive(Debug, PartialEq, PartialOrd, Clone, Copy)]
pub enum MemoryPressureLevel {
    Normal,    // < 70% usage
    Moderate,  // 70-85% usage
    High,      // 85-95% usage
    Critical,  // > 95% usage
}

/// Leak detection information
#[derive(Debug, Clone)]
pub struct LeakInformation {
    pub potential_leak_bytes: usize,
    pub allocation_deallocation_ratio: f64,
}

impl MemoryTracker {
    /// Create a new memory tracker with specified limit
    pub fn new(max_bytes: usize) -> Self {
        Self {
            allocated_bytes: AtomicUsize::new(0),
            max_bytes,
            peak_bytes: AtomicUsize::new(0),
            allocation_count: AtomicUsize::new(0),
            deallocation_count: AtomicUsize::new(0),
            history: Arc::new(Mutex::new(None)),
            leak_detection: Arc::new(Mutex::new(None)),
        }
    }

    /// Track allocation of specified bytes
    pub fn allocate(&self, bytes: usize) -> bool {
        let current = self.allocated_bytes.load(Ordering::Relaxed);
        if current + bytes > self.max_bytes {
            false // Would exceed limit
        } else {
            let new_value = self.allocated_bytes.fetch_add(bytes, Ordering::Relaxed) + bytes;
            
            // Update peak if necessary
            let mut peak = self.peak_bytes.load(Ordering::Relaxed);
            while new_value > peak {
                match self.peak_bytes.compare_exchange_weak(
                    peak,
                    new_value,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => break,
                    Err(p) => peak = p,
                }
            }
            
            // Update counters
            self.allocation_count.fetch_add(1, Ordering::Relaxed);
            
            // Update history if enabled
            if let Ok(mut history_opt) = self.history.lock() {
                if let Some(history) = history_opt.as_mut() {
                    history.add_sample(new_value);
                }
            }
            
            // Update leak detection if enabled
            if let Ok(mut leak_opt) = self.leak_detection.lock() {
                if let Some(leak) = leak_opt.as_mut() {
                    leak.record_allocation(bytes);
                }
            }
            
            true
        }
    }

    /// Track deallocation of specified bytes
    pub fn deallocate(&self, bytes: usize) {
        self.allocated_bytes.fetch_sub(bytes, Ordering::Relaxed);
        self.deallocation_count.fetch_add(1, Ordering::Relaxed);
        
        // Update leak detection if enabled
        if let Ok(mut leak_opt) = self.leak_detection.lock() {
            if let Some(leak) = leak_opt.as_mut() {
                leak.record_deallocation(bytes);
            }
        }
    }

    /// Get current allocated bytes
    pub fn allocated_bytes(&self) -> usize {
        self.allocated_bytes.load(Ordering::Relaxed)
    }

    /// Get available bytes
    pub fn available_bytes(&self) -> usize {
        self.max_bytes.saturating_sub(self.allocated_bytes())
    }

    /// Get peak bytes allocated
    pub fn peak_bytes(&self) -> usize {
        self.peak_bytes.load(Ordering::Relaxed)
    }

    /// Get allocation count
    pub fn allocation_count(&self) -> usize {
        self.allocation_count.load(Ordering::Relaxed)
    }

    /// Get deallocation count
    pub fn deallocation_count(&self) -> usize {
        self.deallocation_count.load(Ordering::Relaxed)
    }

    /// Get memory usage as percentage
    pub fn usage_percent(&self) -> f64 {
        (self.allocated_bytes() as f64 / self.max_bytes as f64) * 100.0
    }

    /// Check if memory usage exceeds threshold
    pub fn exceeds_threshold(&self, threshold_percent: f64) -> bool {
        self.usage_percent() > threshold_percent
    }

    /// Get current memory pressure level
    pub fn get_pressure_level(&self) -> MemoryPressureLevel {
        let usage = self.usage_percent();
        if usage < 70.0 {
            MemoryPressureLevel::Normal
        } else if usage < 85.0 {
            MemoryPressureLevel::Moderate
        } else if usage < 95.0 {
            MemoryPressureLevel::High
        } else {
            MemoryPressureLevel::Critical
        }
    }

    /// Get the memory limit for this tracker
    pub fn memory_limit(&self) -> usize {
        self.max_bytes
    }

    /// Get memory statistics snapshot
    pub fn get_statistics(&self) -> MemoryStatistics {
        let allocations = self.allocation_count();
        let deallocations = self.deallocation_count();
        let current = self.allocated_bytes();
        let peak = self.peak_bytes();
        
        // Calculate average based on peak usage, not current
        // This gives a better representation of typical allocation size
        let avg_size = if allocations > 0 {
            peak as f64 / allocations as f64
        } else {
            0.0
        };
        
        // Simple fragmentation estimation based on allocation patterns
        let fragmentation = if allocations > deallocations && deallocations > 0 {
            let ratio = deallocations as f64 / allocations as f64;
            (1.0 - ratio).min(1.0).max(0.0)
        } else {
            0.0
        };
        
        MemoryStatistics {
            current_bytes: current,
            peak_bytes: peak,
            total_allocations: allocations,
            total_deallocations: deallocations,
            limit_bytes: self.max_bytes,
            average_allocation_size: avg_size,
            fragmentation_ratio: fragmentation,
        }
    }

    /// Enable history tracking with specified sample limit
    pub fn enable_history_tracking(&self, max_samples: usize) {
        let mut history_opt = self.history.lock().unwrap();
        *history_opt = Some(MemoryHistory::new(max_samples));
    }

    /// Get usage history
    pub fn get_usage_history(&self) -> Vec<MemoryHistorySample> {
        if let Ok(history_opt) = self.history.lock() {
            if let Some(history) = history_opt.as_ref() {
                return history.get_samples();
            }
        }
        Vec::new()
    }

    /// Enable leak detection
    pub fn enable_leak_detection(&self) {
        let mut leak_opt = self.leak_detection.lock().unwrap();
        *leak_opt = Some(LeakDetector::new(200)); // 200 sample window - smaller for testing
    }

    /// Check if there's a potential memory leak
    pub fn has_potential_leak(&self) -> bool {
        if let Ok(leak_opt) = self.leak_detection.lock() {
            if let Some(leak) = leak_opt.as_ref() {
                return leak.has_potential_leak();
            }
        }
        false
    }

    /// Get leak information
    pub fn get_leak_information(&self) -> LeakInformation {
        if let Ok(leak_opt) = self.leak_detection.lock() {
            if let Some(leak) = leak_opt.as_ref() {
                return leak.get_leak_information();
            }
        }
        LeakInformation {
            potential_leak_bytes: 0,
            allocation_deallocation_ratio: 1.0,
        }
    }

    /// Check if defragmentation should be recommended
    pub fn should_recommend_defragmentation(&self) -> bool {
        let stats = self.get_statistics();
        stats.fragmentation_ratio > 0.3 // 30% fragmentation threshold
    }
}

impl MemoryHistory {
    fn new(max_samples: usize) -> Self {
        Self {
            samples: VecDeque::with_capacity(max_samples),
            max_samples,
        }
    }

    fn add_sample(&mut self, bytes_allocated: usize) {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        
        self.samples.push_back(MemoryHistorySample {
            timestamp,
            bytes_allocated,
        });
        
        // Maintain window size
        while self.samples.len() > self.max_samples {
            self.samples.pop_front();
        }
    }

    fn get_samples(&self) -> Vec<MemoryHistorySample> {
        self.samples.iter().cloned().collect()
    }
}

impl LeakDetector {
    fn new(window_size: usize) -> Self {
        Self {
            allocation_pattern: VecDeque::with_capacity(window_size),
            deallocation_pattern: VecDeque::with_capacity(window_size),
            window_size,
        }
    }

    fn record_allocation(&mut self, size: usize) {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        
        self.allocation_pattern.push_back((timestamp, size));
        
        // Maintain window
        while self.allocation_pattern.len() > self.window_size {
            self.allocation_pattern.pop_front();
        }
    }

    fn record_deallocation(&mut self, size: usize) {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        
        self.deallocation_pattern.push_back((timestamp, size));
        
        // Maintain window
        while self.deallocation_pattern.len() > self.window_size {
            self.deallocation_pattern.pop_front();
        }
    }

    fn has_potential_leak(&self) -> bool {
        // Use byte-based analysis rather than just counting operations
        let leak_info = self.get_leak_information();
        
        // If there's a significant amount of unaccounted bytes
        if leak_info.potential_leak_bytes > 10240 { // 10KB threshold
            return true;
        }
        
        // If allocation/deallocation ratio is too high
        if leak_info.allocation_deallocation_ratio > 2.0 {
            return true;
        }
        
        // Simple count-based heuristic for cases with no deallocations
        let alloc_count = self.allocation_pattern.len();
        let dealloc_count = self.deallocation_pattern.len();
        
        if alloc_count > 10 && dealloc_count == 0 {
            return true; // No deallocations at all with significant allocations
        }
        
        false
    }

    fn get_leak_information(&self) -> LeakInformation {
        let alloc_bytes: usize = self.allocation_pattern.iter().map(|(_, size)| size).sum();
        let dealloc_bytes: usize = self.deallocation_pattern.iter().map(|(_, size)| size).sum();
        
        let ratio = if dealloc_bytes > 0 {
            alloc_bytes as f64 / dealloc_bytes as f64
        } else if alloc_bytes > 0 {
            f64::INFINITY
        } else {
            1.0
        };
        
        LeakInformation {
            potential_leak_bytes: alloc_bytes.saturating_sub(dealloc_bytes),
            allocation_deallocation_ratio: ratio,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_tracker_creation() {
        let tracker = MemoryTracker::new(1024);
        assert_eq!(tracker.allocated_bytes(), 0);
        assert_eq!(tracker.usage_percent(), 0.0);
    }

    #[test]
    fn test_memory_allocation_tracking() {
        let tracker = MemoryTracker::new(1024);
        
        assert!(tracker.allocate(512));
        assert_eq!(tracker.allocated_bytes(), 512);
        assert_eq!(tracker.usage_percent(), 50.0);
        
        tracker.deallocate(256);
        assert_eq!(tracker.allocated_bytes(), 256);
        assert_eq!(tracker.usage_percent(), 25.0);
    }

    #[test]  
    fn test_memory_limit_enforcement() {
        let tracker = MemoryTracker::new(1024);
        
        assert!(tracker.allocate(1000));
        assert!(!tracker.allocate(100)); // Should fail - would exceed limit
        assert_eq!(tracker.allocated_bytes(), 1000); // Should not have allocated
    }
}