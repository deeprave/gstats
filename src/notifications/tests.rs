//! Tests for the Generic Notification System

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

use crate::notifications::{
    AsyncNotificationManager, NotificationManager, Publisher, Subscriber,
    ScanEvent, QueueEvent, PluginEvent, NotificationResult, NotificationError
};
// Removed unused imports: EventFilter, RateLimit, OverflowAction
use crate::scanner::modes::ScanMode;

/// Mock subscriber for testing
struct MockSubscriber {
    id: String,
    received_events: Arc<Mutex<Vec<ScanEvent>>>,
    should_fail: bool,
}

impl MockSubscriber {
    fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            received_events: Arc::new(Mutex::new(Vec::new())),
            should_fail: false,
        }
    }
    
    fn new_failing(id: &str) -> Self {
        Self {
            id: id.to_string(),
            received_events: Arc::new(Mutex::new(Vec::new())),
            should_fail: true,
        }
    }
    
    async fn get_received_events(&self) -> Vec<ScanEvent> {
        self.received_events.lock().await.clone()
    }
    
    async fn clear_events(&self) {
        self.received_events.lock().await.clear();
    }
}

#[async_trait::async_trait]
impl Subscriber<ScanEvent> for MockSubscriber {
    async fn handle_event(&self, event: ScanEvent) -> NotificationResult<()> {
        if self.should_fail {
            return Err(NotificationError::generic("Mock subscriber failure"));
        }
        
        self.received_events.lock().await.push(event);
        Ok(())
    }
    
    fn subscriber_id(&self) -> &str {
        &self.id
    }
}

/// Mock publisher for testing
struct MockPublisher {
    id: String,
    manager: AsyncNotificationManager<ScanEvent>,
}

impl MockPublisher {
    fn new(id: &str, manager: AsyncNotificationManager<ScanEvent>) -> Self {
        Self {
            id: id.to_string(),
            manager,
        }
    }
}

#[async_trait::async_trait]
impl Publisher<ScanEvent> for MockPublisher {
    async fn publish(&self, event: ScanEvent) -> NotificationResult<()> {
        self.manager.publish(event).await
    }
    
    async fn publish_to(&self, event: ScanEvent, subscriber_id: &str) -> NotificationResult<()> {
        self.manager.publish_to(event, subscriber_id).await
    }
    
    fn publisher_id(&self) -> &str {
        &self.id
    }
}

#[tokio::test]
async fn test_notification_manager_creation() {
    let manager = AsyncNotificationManager::<ScanEvent>::new();
    assert_eq!(manager.subscriber_count().await, 0);
}

#[tokio::test]
async fn test_subscriber_registration() {
    let mut manager = AsyncNotificationManager::<ScanEvent>::new();
    let subscriber = Arc::new(MockSubscriber::new("test_subscriber"));
    
    // Subscribe
    let result = manager.subscribe(subscriber.clone()).await;
    assert!(result.is_ok());
    assert_eq!(manager.subscriber_count().await, 1);
    assert!(manager.has_subscriber("test_subscriber").await);
    
    // Try to subscribe same subscriber again (should fail)
    let result = manager.subscribe(subscriber).await;
    assert!(result.is_err());
    assert_eq!(manager.subscriber_count().await, 1);
}

#[tokio::test]
async fn test_subscriber_unregistration() {
    let mut manager = AsyncNotificationManager::<ScanEvent>::new();
    let subscriber = Arc::new(MockSubscriber::new("test_subscriber"));
    
    // Subscribe and then unsubscribe
    manager.subscribe(subscriber).await.unwrap();
    assert_eq!(manager.subscriber_count().await, 1);
    
    let result = manager.unsubscribe("test_subscriber").await;
    assert!(result.is_ok());
    assert_eq!(manager.subscriber_count().await, 0);
    assert!(!manager.has_subscriber("test_subscriber").await);
    
    // Try to unsubscribe non-existent subscriber
    let result = manager.unsubscribe("non_existent").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_event_publishing() {
    let mut manager = AsyncNotificationManager::<ScanEvent>::new();
    let subscriber = Arc::new(MockSubscriber::new("test_subscriber"));
    
    manager.subscribe(subscriber.clone()).await.unwrap();
    
    // Publish an event
    let event = ScanEvent::started("scan_001".to_string(), ScanMode::FILES);
    let result = manager.publish(event.clone()).await;
    assert!(result.is_ok());
    
    // Check that subscriber received the event
    tokio::time::sleep(Duration::from_millis(10)).await; // Give time for async delivery
    let received_events = subscriber.get_received_events().await;
    assert_eq!(received_events.len(), 1);
    
    match &received_events[0] {
        ScanEvent::ScanStarted { scan_id, modes, .. } => {
            assert_eq!(scan_id, "scan_001");
            assert_eq!(*modes, ScanMode::FILES);
        }
        _ => panic!("Expected ScanStarted event"),
    }
}

#[tokio::test]
async fn test_targeted_publishing() {
    let mut manager = AsyncNotificationManager::<ScanEvent>::new();
    let subscriber1 = Arc::new(MockSubscriber::new("subscriber1"));
    let subscriber2 = Arc::new(MockSubscriber::new("subscriber2"));
    
    manager.subscribe(subscriber1.clone()).await.unwrap();
    manager.subscribe(subscriber2.clone()).await.unwrap();
    
    // Publish to specific subscriber
    let event = ScanEvent::started("scan_001".to_string(), ScanMode::FILES);
    let result = manager.publish_to(event, "subscriber1").await;
    assert!(result.is_ok());
    
    // Check that only subscriber1 received the event
    tokio::time::sleep(Duration::from_millis(10)).await;
    assert_eq!(subscriber1.get_received_events().await.len(), 1);
    assert_eq!(subscriber2.get_received_events().await.len(), 0);
}

#[tokio::test]
async fn test_publisher_interface() {
    let manager = AsyncNotificationManager::<ScanEvent>::new();
    let mut manager_clone = manager.clone();
    let subscriber = Arc::new(MockSubscriber::new("test_subscriber"));
    
    manager_clone.subscribe(subscriber.clone()).await.unwrap();
    
    let publisher = MockPublisher::new("test_publisher", manager);
    
    // Publish through publisher interface
    let event = ScanEvent::started("scan_001".to_string(), ScanMode::FILES);
    let result = publisher.publish(event).await;
    assert!(result.is_ok());
    
    // Check that subscriber received the event
    tokio::time::sleep(Duration::from_millis(10)).await;
    assert_eq!(subscriber.get_received_events().await.len(), 1);
}

#[tokio::test]
async fn test_delivery_failure_handling() {
    let mut manager = AsyncNotificationManager::<ScanEvent>::new();
    let failing_subscriber = Arc::new(MockSubscriber::new_failing("failing_subscriber"));
    let normal_subscriber = Arc::new(MockSubscriber::new("normal_subscriber"));
    
    manager.subscribe(failing_subscriber.clone()).await.unwrap();
    manager.subscribe(normal_subscriber.clone()).await.unwrap();
    
    // Publish an event - should succeed for normal subscriber despite failing subscriber
    let event = ScanEvent::started("scan_001".to_string(), ScanMode::FILES);
    let result = manager.publish(event).await;
    assert!(result.is_ok()); // Manager doesn't fail if some subscribers fail
    
    // Check that normal subscriber still received the event
    tokio::time::sleep(Duration::from_millis(10)).await;
    assert_eq!(normal_subscriber.get_received_events().await.len(), 1);
    assert_eq!(failing_subscriber.get_received_events().await.len(), 0);
}

#[tokio::test]
async fn test_shutdown() {
    let mut manager = AsyncNotificationManager::<ScanEvent>::new();
    let subscriber = Arc::new(MockSubscriber::new("test_subscriber"));
    
    manager.subscribe(subscriber).await.unwrap();
    assert_eq!(manager.subscriber_count().await, 1);
    
    // Shutdown
    let result = manager.shutdown().await;
    assert!(result.is_ok());
    assert_eq!(manager.subscriber_count().await, 0);
    
    // Publishing after shutdown should fail
    let event = ScanEvent::started("scan_001".to_string(), ScanMode::FILES);
    let result = manager.publish(event).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_statistics() {
    let mut manager = AsyncNotificationManager::<ScanEvent>::new();
    let subscriber = Arc::new(MockSubscriber::new("test_subscriber"));
    
    manager.subscribe(subscriber.clone()).await.unwrap();
    
    // Publish some events
    for i in 0..3 {
        let event = ScanEvent::started(format!("scan_{:03}", i), ScanMode::FILES);
        manager.publish(event).await.unwrap();
    }
    
    tokio::time::sleep(Duration::from_millis(50)).await; // Give time for delivery
    
    // Check statistics
    let stats = manager.get_stats().await;
    assert_eq!(stats.events_published, 3);
    assert_eq!(stats.events_delivered, 3);
    assert_eq!(stats.delivery_failures, 0);
    
    // Check subscriber received all events
    assert_eq!(subscriber.get_received_events().await.len(), 3);
}

#[tokio::test]
async fn test_event_helper_functions() {
    // Test ScanEvent helper functions
    let started_event = ScanEvent::started("scan_001".to_string(), ScanMode::FILES);
    match started_event {
        ScanEvent::ScanStarted { scan_id, modes, .. } => {
            assert_eq!(scan_id, "scan_001");
            assert_eq!(modes, ScanMode::FILES);
        }
        _ => panic!("Expected ScanStarted event"),
    }
    
    let completed_event = ScanEvent::completed(
        "scan_001".to_string(),
        Duration::from_secs(10),
        vec!["Warning 1".to_string(), "Warning 2".to_string()]
    );
    match completed_event {
        ScanEvent::ScanCompleted { scan_id, duration, warnings, .. } => {
            assert_eq!(scan_id, "scan_001");
            assert_eq!(duration, Duration::from_secs(10));
            assert_eq!(warnings.len(), 2);
            assert_eq!(warnings[0], "Warning 1");
        }
        _ => panic!("Expected ScanCompleted event"),
    }
    
    let data_ready_event = ScanEvent::data_ready(
        "scan_001".to_string(),
        "commits_plugin".to_string(),
        "commit_analysis".to_string()
    );
    match data_ready_event {
        ScanEvent::DataReady { scan_id, plugin_id, data_type, .. } => {
            assert_eq!(scan_id, "scan_001");
            assert_eq!(plugin_id, "commits_plugin");
            assert_eq!(data_type, "commit_analysis");
        }
        _ => panic!("Expected DataReady event"),
    }
}

#[tokio::test]
async fn test_multiple_event_types() {
    // Test that we can create managers for different event types
    let scan_manager = AsyncNotificationManager::<ScanEvent>::new();
    let queue_manager = AsyncNotificationManager::<QueueEvent>::new();
    let plugin_manager = AsyncNotificationManager::<PluginEvent>::new();
    
    assert_eq!(scan_manager.subscriber_count().await, 0);
    assert_eq!(queue_manager.subscriber_count().await, 0);
    assert_eq!(plugin_manager.subscriber_count().await, 0);
}
