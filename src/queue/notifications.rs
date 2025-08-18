//! Queue Event Notification System
//!
//! Provides a generic event system for coordinating between queue producers
//! and consumers. Events are broadcast to all subscribers using tokio's
//! broadcast channel for efficient async coordination.

use crate::queue::error::QueueResult;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

/// Events emitted by the queue system for coordination
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum QueueEvent {
    /// Scanning has started
    ScanStarted {
        scan_id: String,
        timestamp: u64,
    },

    /// One or more messages have been added to the queue
    MessageAdded {
        scan_id: String,
        count: usize,
        queue_size: usize,
        timestamp: u64,
    },

    /// Scanning has completed - no more messages will be added
    ScanComplete {
        scan_id: String,
        total_messages: u64,
        timestamp: u64,
    },

    /// Queue has been drained - all messages processed
    QueueDrained {
        scan_id: String,
        timestamp: u64,
    },

    /// Memory usage warning
    MemoryWarning {
        scan_id: String,
        current_size: usize,
        threshold: usize,
        timestamp: u64,
    },
}

impl QueueEvent {
    /// Get the scan ID associated with this event
    pub fn scan_id(&self) -> &str {
        match self {
            QueueEvent::ScanStarted { scan_id, .. } => scan_id,
            QueueEvent::MessageAdded { scan_id, .. } => scan_id,
            QueueEvent::ScanComplete { scan_id, .. } => scan_id,
            QueueEvent::QueueDrained { scan_id, .. } => scan_id,
            QueueEvent::MemoryWarning { scan_id, .. } => scan_id,
        }
    }

    /// Get the timestamp of this event
    pub fn timestamp(&self) -> u64 {
        match self {
            QueueEvent::ScanStarted { timestamp, .. } => *timestamp,
            QueueEvent::MessageAdded { timestamp, .. } => *timestamp,
            QueueEvent::ScanComplete { timestamp, .. } => *timestamp,
            QueueEvent::QueueDrained { timestamp, .. } => *timestamp,
            QueueEvent::MemoryWarning { timestamp, .. } => *timestamp,
        }
    }

    /// Create a scan started event
    pub fn scan_started(scan_id: String) -> Self {
        Self::ScanStarted {
            scan_id,
            timestamp: current_timestamp(),
        }
    }

    /// Create a message added event
    pub fn message_added(scan_id: String, count: usize, queue_size: usize) -> Self {
        Self::MessageAdded {
            scan_id,
            count,
            queue_size,
            timestamp: current_timestamp(),
        }
    }

    /// Create a scan complete event
    pub fn scan_complete(scan_id: String, total_messages: u64) -> Self {
        Self::ScanComplete {
            scan_id,
            total_messages,
            timestamp: current_timestamp(),
        }
    }

}

/// Event notifier for broadcasting queue events
#[derive(Debug)]
pub struct QueueEventNotifier {
    sender: broadcast::Sender<QueueEvent>,
}

impl QueueEventNotifier {
    /// Create a new event notifier with the specified capacity
    pub fn new(capacity: usize) -> Self {
        let (sender, _receiver) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Create a new event notifier with default capacity
    pub fn with_default_capacity() -> Self {
        Self::new(1000) // Default capacity for event buffer
    }

    /// Subscribe to events - returns a receiver for event notifications
    pub fn subscribe(&self) -> broadcast::Receiver<QueueEvent> {
        self.sender.subscribe()
    }

    /// Emit an event to all subscribers
    pub fn emit(&self, event: QueueEvent) -> QueueResult<()> {
        match self.sender.send(event.clone()) {
            Ok(subscriber_count) => {
                log::trace!("Emitted event {:?} to {} subscribers", event, subscriber_count);
                Ok(())
            }
            Err(broadcast::error::SendError(_)) => {
                // This happens when there are no active receivers
                log::trace!("No active subscribers for event {:?}", event);
                Ok(())
            }
        }
    }

}

impl Clone for QueueEventNotifier {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}

/// Get current timestamp in milliseconds since Unix epoch
fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{timeout, Duration};

    #[test]
    fn test_queue_event_creation() {
        let event = QueueEvent::scan_started("test-scan".to_string());
        assert_eq!(event.scan_id(), "test-scan");
        assert!(event.timestamp() > 0);

        if let QueueEvent::ScanStarted { .. } = event {
            // ScanStarted event created successfully
        } else {
            panic!("Expected ScanStarted event");
        }
    }

    #[test]
    fn test_queue_event_notifier_creation() {
        let _notifier = QueueEventNotifier::new(100);
        // Basic creation test - no specific assertions needed
    }

    #[tokio::test]
    async fn test_event_subscription_and_emission() {
        let notifier = QueueEventNotifier::new(10);
        let mut receiver = notifier.subscribe();

        // Subscription successful if no panic occurs

        let event = QueueEvent::scan_started("test".to_string());
        notifier.emit(event.clone()).unwrap();

        let received_event = timeout(Duration::from_millis(100), receiver.recv())
            .await
            .expect("Should receive event within timeout")
            .expect("Should successfully receive event");

        assert_eq!(received_event, event);
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let notifier = QueueEventNotifier::new(10);
        let mut receiver1 = notifier.subscribe();
        let mut receiver2 = notifier.subscribe();

        // Multiple subscribers created successfully

        let event = QueueEvent::message_added("test".to_string(), 1, 5);
        notifier.emit(event.clone()).unwrap();

        // Both receivers should get the event
        let event1 = receiver1.recv().await.unwrap();
        let event2 = receiver2.recv().await.unwrap();

        assert_eq!(event1, event);
        assert_eq!(event2, event);
    }

    #[test]
    fn test_event_no_subscribers() {
        let notifier = QueueEventNotifier::new(10);
        let event = QueueEvent::scan_complete("test".to_string(), 100);
        
        // Should not error when no subscribers
        assert!(notifier.emit(event).is_ok());
    }
}
