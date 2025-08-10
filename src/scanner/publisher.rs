use std::sync::Arc;
use crate::notifications::traits::{Publisher, NotificationManager};
use crate::notifications::manager::AsyncNotificationManager;
use crate::notifications::events::ScanEvent;
use crate::notifications::error::NotificationResult;

/// Publisher wrapper for scanner components to emit scan lifecycle events
#[derive(Clone)]
pub struct ScannerPublisher {
    notification_manager: Arc<AsyncNotificationManager<ScanEvent>>,
    publisher_id: String,
}

impl ScannerPublisher {
    /// Create a new scanner publisher
    pub fn new(notification_manager: Arc<AsyncNotificationManager<ScanEvent>>) -> Self {
        Self {
            notification_manager,
            publisher_id: "scanner".to_string(),
        }
    }
    
    /// Create a new scanner publisher with custom ID
    pub fn with_id(notification_manager: Arc<AsyncNotificationManager<ScanEvent>>, publisher_id: String) -> Self {
        Self {
            notification_manager,
            publisher_id,
        }
    }
}

#[async_trait::async_trait]
impl Publisher<ScanEvent> for ScannerPublisher {
    async fn publish(&self, event: ScanEvent) -> NotificationResult<()> {
        self.notification_manager.publish(event).await
    }
    
    async fn publish_to(&self, event: ScanEvent, subscriber_id: &str) -> NotificationResult<()> {
        self.notification_manager.publish_to(event, subscriber_id).await
    }
    
    fn publisher_id(&self) -> &str {
        &self.publisher_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::modes::ScanMode;
    use std::time::Duration;

    #[tokio::test]
    async fn test_scanner_publisher_creation() {
        let notification_manager = Arc::new(AsyncNotificationManager::new());
        let scanner_publisher = ScannerPublisher::new(notification_manager);
        
        // Should have default publisher ID
        assert_eq!(scanner_publisher.publisher_id(), "scanner");
    }

    #[tokio::test]
    async fn test_scanner_publisher_with_custom_id() {
        let notification_manager = Arc::new(AsyncNotificationManager::new());
        let scanner_publisher = ScannerPublisher::with_id(notification_manager, "custom-scanner".to_string());
        
        // Should have custom publisher ID
        assert_eq!(scanner_publisher.publisher_id(), "custom-scanner");
    }

    #[tokio::test]
    async fn test_scanner_publisher_scan_started_event() {
        let notification_manager = Arc::new(AsyncNotificationManager::new());
        let scanner_publisher = ScannerPublisher::new(notification_manager);
        
        let event = ScanEvent::ScanStarted {
            scan_id: "test-scan-1".to_string(),
            modes: ScanMode::HISTORY,
        };
        
        // Should publish without error
        let result = scanner_publisher.publish(event).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_scanner_publisher_scan_completed_event() {
        let notification_manager = Arc::new(AsyncNotificationManager::new());
        let scanner_publisher = ScannerPublisher::new(notification_manager);
        
        let event = ScanEvent::ScanCompleted {
            scan_id: "test-scan-1".to_string(),
            duration: Duration::from_secs(10),
            warnings: vec!["Test warning".to_string()],
        };
        
        // Should publish without error
        let result = scanner_publisher.publish(event).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_scanner_publisher_scan_data_ready_event() {
        let notification_manager = Arc::new(AsyncNotificationManager::new());
        let scanner_publisher = ScannerPublisher::new(notification_manager);
        
        let event = ScanEvent::ScanDataReady {
            scan_id: "test-scan-1".to_string(),
            data_type: "commits".to_string(),
            message_count: 42,
        };
        
        // Should publish without error
        let result = scanner_publisher.publish(event).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_scanner_publisher_scan_warning_event() {
        let notification_manager = Arc::new(AsyncNotificationManager::new());
        let scanner_publisher = ScannerPublisher::new(notification_manager);
        
        let event = ScanEvent::ScanWarning {
            scan_id: "test-scan-1".to_string(),
            warning: "Missing file detected".to_string(),
            recoverable: true,
        };
        
        // Should publish without error
        let result = scanner_publisher.publish(event).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_scanner_publisher_scan_error_event() {
        let notification_manager = Arc::new(AsyncNotificationManager::new());
        let scanner_publisher = ScannerPublisher::new(notification_manager);
        
        let event = ScanEvent::ScanError {
            scan_id: "test-scan-1".to_string(),
            error: "Repository corrupted".to_string(),
            fatal: true,
        };
        
        // Should publish without error
        let result = scanner_publisher.publish(event).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_scanner_publisher_publish_to_specific_subscriber() {
        let notification_manager = Arc::new(AsyncNotificationManager::new());
        let scanner_publisher = ScannerPublisher::new(notification_manager);
        
        let event = ScanEvent::ScanStarted {
            scan_id: "test-scan-1".to_string(),
            modes: ScanMode::FILES,
        };
        
        // Should handle publishing to non-existent subscriber gracefully
        // (The notification manager should handle this case appropriately)
        let result = scanner_publisher.publish_to(event, "test-subscriber").await;
        // The result depends on the notification manager's implementation
        // For now, we just verify the method can be called without panicking
        let _ = result; // Don't assert on the result since subscriber doesn't exist
    }
}
