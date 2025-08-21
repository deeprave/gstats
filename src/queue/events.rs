//! Queue Event Publishing and Subscription
//!
//! This module handles the event-driven aspects of the multi-consumer queue,
//! including publishing QueueEvents and subscribing to ScanEvents for
//! statistical tracking across multiple scanner sessions.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use async_trait::async_trait;
use log::{info, debug, trace};

use crate::notifications::AsyncNotificationManager;
use crate::notifications::events::{QueueEvent, ScanEvent};
use crate::notifications::traits::{Publisher, Subscriber, NotificationManager};
use crate::notifications::error::NotificationResult;
use crate::queue::statistics::ScanStatistics;

/// Event handling implementation for MultiConsumerQueue
pub struct QueueEventHandler {
    pub scan_statistics: Arc<RwLock<HashMap<String, ScanStatistics>>>,
    pub notification_manager: Arc<AsyncNotificationManager<QueueEvent>>,
}

impl QueueEventHandler {
    pub fn new(
        scan_statistics: Arc<RwLock<HashMap<String, ScanStatistics>>>,
        notification_manager: Arc<AsyncNotificationManager<QueueEvent>>,
    ) -> Self {
        Self {
            scan_statistics,
            notification_manager,
        }
    }

    /// Helper method to publish QueueEvents
    pub async fn publish_queue_event(&self, event: crate::queue::notifications::QueueEvent) -> NotificationResult<()> {
        // Convert our queue notifications QueueEvent to the notification system's QueueEvent
        let notification_event = match event {
            crate::queue::notifications::QueueEvent::ScanStarted { scan_id, timestamp } => {
                QueueEvent::MessageAdded {
                    queue_id: scan_id,
                    message_type: "scan_started".to_string(),
                    queue_size: 0,
                    memory_usage_bytes: 0,
                    added_at: std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(timestamp),
                }
            },
            crate::queue::notifications::QueueEvent::MessageAdded { scan_id, count, queue_size, timestamp } => {
                QueueEvent::MessageAdded {
                    queue_id: scan_id,
                    message_type: "scan_message".to_string(),
                    queue_size,
                    memory_usage_bytes: (count * 1000) as u64, // Rough estimate
                    added_at: std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(timestamp),
                }
            },
            crate::queue::notifications::QueueEvent::ScanComplete { scan_id, total_messages, timestamp } => {
                QueueEvent::QueueEmpty {
                    queue_id: scan_id,
                    last_message_processed_at: Some(std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(timestamp)),
                    total_processed: total_messages,
                    emptied_at: std::time::SystemTime::now(),
                }
            },
            _ => {
                // For other event types, create a generic message added event
                QueueEvent::MessageAdded {
                    queue_id: "unknown".to_string(),
                    message_type: "generic".to_string(),
                    queue_size: 0,
                    memory_usage_bytes: 0,
                    added_at: std::time::SystemTime::now(),
                }
            }
        };
        
        self.notification_manager.publish(notification_event).await
    }
    
    /// Log scan completion statistics
    pub async fn log_scan_completion_statistics(&self, scan_id: &str, queue_total_messages: u64, active_consumers: usize, memory_usage: u64) {
        let stats = self.scan_statistics.read().await;
        if let Some(scan_stats) = stats.get(scan_id) {
            if let Some(duration) = scan_stats.duration() {
                info!(
                    "Scan '{}' completed: {} messages processed in {:.2}s", 
                    scan_id, 
                    scan_stats.total_messages, 
                    duration.as_secs_f64()
                );
            } else {
                info!(
                    "Scan '{}' completed: {} messages processed", 
                    scan_id, 
                    scan_stats.total_messages
                );
            }
        }
        
        // Also log overall queue statistics
        info!(
            "Queue statistics: {} total messages, {} active consumers, {} KB memory usage",
            queue_total_messages,
            active_consumers,
            memory_usage / 1024
        );
    }
}

#[async_trait]
impl Publisher<QueueEvent> for QueueEventHandler {
    /// Publish a queue event to all subscribers
    async fn publish(&self, event: QueueEvent) -> NotificationResult<()> {
        self.notification_manager.publish(event).await
    }
}

#[async_trait]
impl Subscriber<ScanEvent> for QueueEventHandler {
    fn subscriber_id(&self) -> &str {
        "queue-event-handler"
    }
    
    async fn handle_event(&self, event: ScanEvent) -> NotificationResult<()> {
        match event {
            ScanEvent::ScanStarted { scan_id } => {
                info!("Queue: Scan started for scan_id '{}'", scan_id);
                
                // Update scan statistics - mark scan as started
                {
                    let mut stats = self.scan_statistics.write().await;
                    let scan_stats = stats.entry(scan_id.clone()).or_insert_with(ScanStatistics::new);
                    scan_stats.start_scan();
                }
                
                // Create and publish QueueEvent
                let queue_event = crate::queue::notifications::QueueEvent::scan_started(scan_id);
                self.publish_queue_event(queue_event).await?;
            },
            
            ScanEvent::ScanDataReady { scan_id, message_count, .. } => {
                debug!("Queue: Scan data ready for scan_id '{}', {} messages", scan_id, message_count);
                
                // Update accumulated message count
                {
                    let mut stats = self.scan_statistics.write().await;
                    let scan_stats = stats.entry(scan_id.clone()).or_insert_with(ScanStatistics::new);
                    scan_stats.accumulated_message_count += message_count;
                }
                
                // Note: queue_size would need to be passed in or retrieved differently
                // For now, we'll use 0 as a placeholder - this will need to be fixed when integrating
                let queue_size = 0;
                
                // Create and publish QueueEvent
                let queue_event = crate::queue::notifications::QueueEvent::message_added(
                    scan_id, 
                    message_count, 
                    queue_size
                );
                self.publish_queue_event(queue_event).await?;
            },
            
            ScanEvent::ScanCompleted { scan_id, duration, .. } => {
                info!("Queue: Scan completed for scan_id '{}' in {:?}", scan_id, duration);
                
                // Update scan statistics - mark scan as completed
                let total_messages = {
                    let mut stats = self.scan_statistics.write().await;
                    if let Some(scan_stats) = stats.get_mut(&scan_id) {
                        scan_stats.complete_scan();
                        scan_stats.total_messages = scan_stats.accumulated_message_count as u64;
                        scan_stats.total_messages
                    } else {
                        0
                    }
                };
                
                // Create and publish QueueEvent
                let queue_event = crate::queue::notifications::QueueEvent::scan_complete(
                    scan_id.clone(), 
                    total_messages
                );
                self.publish_queue_event(queue_event).await?;
                
                // Log scan completion statistics
                // Note: These values would need to be passed in from the queue
                self.log_scan_completion_statistics(&scan_id, total_messages, 0, 0).await;
            },
            
            _ => {
                // Other scan events (ScanProgress, ScanWarning, ScanError) don't require queue action
                trace!("Queue: Ignoring scan event: {:?}", event);
            }
        }
        
        Ok(())
    }
}