//! Memory Tracking and Monitoring
//! 
//! Provides memory usage tracking for queue operations

use std::sync::atomic::{AtomicUsize, Ordering};

/// Memory usage tracker for queue operations
pub struct MemoryTracker {
    allocated_bytes: AtomicUsize,
    max_bytes: usize,
}

impl MemoryTracker {
    /// Create a new memory tracker with specified limit
    pub fn new(max_bytes: usize) -> Self {
        Self {
            allocated_bytes: AtomicUsize::new(0),
            max_bytes,
        }
    }

    /// Track allocation of specified bytes
    pub fn allocate(&self, bytes: usize) -> bool {
        let current = self.allocated_bytes.load(Ordering::Relaxed);
        if current + bytes > self.max_bytes {
            false // Would exceed limit
        } else {
            self.allocated_bytes.fetch_add(bytes, Ordering::Relaxed);
            true
        }
    }

    /// Track deallocation of specified bytes
    pub fn deallocate(&self, bytes: usize) {
        self.allocated_bytes.fetch_sub(bytes, Ordering::Relaxed);
    }

    /// Get current allocated bytes
    pub fn allocated_bytes(&self) -> usize {
        self.allocated_bytes.load(Ordering::Relaxed)
    }

    /// Get memory usage as percentage
    pub fn usage_percent(&self) -> f64 {
        (self.allocated_bytes() as f64 / self.max_bytes as f64) * 100.0
    }

    /// Check if memory usage exceeds threshold
    pub fn exceeds_threshold(&self, threshold_percent: f64) -> bool {
        self.usage_percent() > threshold_percent
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