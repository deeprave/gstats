#[cfg(test)]
mod tests {
    // Use crate-relative imports
    use crate::notifications::{AsyncNotificationManager, ScanEvent, NotificationManager};
    use crate::scanner::ScannerPublisher;
    use crate::plugin::{PluginExecutor, SharedPluginRegistry};
    use crate::scanner::messages::{ScanMessage, MessageHeader, MessageData};
    use crate::plugin::tests::mock_plugins::MockScannerPlugin;
    
    // Import specific traits needed
    use crate::notifications::traits::Subscriber;
    use crate::notifications::NotificationResult;
    use async_trait::async_trait;

    #[tokio::test]
    async fn test_scan_data_ready_events_emission() {
        // Create notification manager
        let mut notification_manager = AsyncNotificationManager::<ScanEvent>::new();
        
        // Mock subscriber to capture events
        let events = std::sync::Arc::new(tokio::sync::Mutex::new(Vec::<ScanEvent>::new()));
        let events_clone = events.clone();
        
        // Create a mock subscriber
        struct MockSubscriber {
            events: std::sync::Arc<tokio::sync::Mutex<Vec<ScanEvent>>>,
        }
        
        #[async_trait]
        impl Subscriber<ScanEvent> for MockSubscriber {
            async fn handle_event(&self, event: ScanEvent) -> NotificationResult<()> {
                self.events.lock().await.push(event);
                Ok(())
            }
            
            fn subscriber_id(&self) -> &str {
                "test_subscriber"
            }
        }
        
        let subscriber = std::sync::Arc::new(MockSubscriber { events: events_clone });
        
        // Subscribe to events
        notification_manager.subscribe(subscriber).await.expect("Failed to subscribe");
        
        // Create scanner publisher
        let scanner_publisher = ScannerPublisher::new(std::sync::Arc::new(notification_manager));
        
        // Create plugin registry and add a mock plugin
        let plugin_registry = SharedPluginRegistry::new();
        
        // Add a mock scanner plugin that will process messages
        {
            let mock_plugin = MockScannerPlugin::new("test-scanner", false);
            let mut registry = plugin_registry.inner().write().await;
            registry.register_plugin(Box::new(mock_plugin)).await.expect("Failed to register mock plugin");
        }
        
        let plugin_executor = PluginExecutor::with_scanner_publisher(
            plugin_registry,
            scanner_publisher.clone(),
            "test_scan_789".to_string(),
        );
        
        // Create test messages for different data types
        let commit_message = ScanMessage {
            header: MessageHeader::new(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            ),
            data: MessageData::CommitInfo {
                hash: "abc123".to_string(),
                author: "Test Author".to_string(),
                message: "Test commit".to_string(),
                timestamp: 1234567890,
                changed_files: vec![],
            },
        };
        
        let file_message = ScanMessage {
            header: MessageHeader::new(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            ),
            data: MessageData::FileInfo {
                path: "src/main.rs".to_string(),
                size: 1024,
                lines: 50,
            },
        };
        
        // Process messages through plugin executor (should emit ScanDataReady events)
        let _commit_results = plugin_executor.process_message(commit_message).await;
        let _file_results = plugin_executor.process_message(file_message).await;
        
        // Allow events to be processed
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        
        // Verify ScanDataReady events were emitted
        let captured_events = events.lock().await;
        
        // Filter for ScanDataReady events
        let data_ready_events: Vec<_> = captured_events.iter()
            .filter_map(|e| match e {
                ScanEvent::ScanDataReady { data_type, message_count, .. } => Some((data_type, message_count)),
                _ => None,
            })
            .collect();
        
        // Should have events for commits and files data types
        assert!(data_ready_events.len() >= 1, "Should have at least 1 ScanDataReady event, got {}", data_ready_events.len());
        
        // Verify scan_id consistency
        for event in captured_events.iter() {
            match event {
                ScanEvent::ScanDataReady { scan_id: event_scan_id, .. } => {
                    assert_eq!(event_scan_id, "test_scan_789", "Scan ID should be consistent");
                }
                _ => {} // Ignore other event types
            }
        }
    }

    #[tokio::test]
    async fn test_data_ready_events_emission() {
        // Create notification manager
        let mut notification_manager = AsyncNotificationManager::<ScanEvent>::new();
        
        // Mock subscriber to capture events
        let events = std::sync::Arc::new(tokio::sync::Mutex::new(Vec::<ScanEvent>::new()));
        let events_clone = events.clone();
        
        // Create a mock subscriber
        struct MockSubscriber {
            events: std::sync::Arc<tokio::sync::Mutex<Vec<ScanEvent>>>,
        }
        
        #[async_trait]
        impl Subscriber<ScanEvent> for MockSubscriber {
            async fn handle_event(&self, event: ScanEvent) -> NotificationResult<()> {
                self.events.lock().await.push(event);
                Ok(())
            }
            
            fn subscriber_id(&self) -> &str {
                "test_subscriber"
            }
        }
        
        let subscriber = std::sync::Arc::new(MockSubscriber { events: events_clone });
        
        // Subscribe to events
        notification_manager.subscribe(subscriber).await.expect("Failed to subscribe");
        
        // Create scanner publisher
        let scanner_publisher = ScannerPublisher::new(std::sync::Arc::new(notification_manager));
        
        // Create plugin registry and add a mock plugin
        let plugin_registry = SharedPluginRegistry::new();
        
        // Add a mock scanner plugin that will process messages
        {
            let mock_plugin = MockScannerPlugin::new("test-scanner", false);
            let mut registry = plugin_registry.inner().write().await;
            registry.register_plugin(Box::new(mock_plugin)).await.expect("Failed to register mock plugin");
        }
        
        let plugin_executor = PluginExecutor::with_scanner_publisher(
            plugin_registry,
            scanner_publisher.clone(),
            "test_scan_456".to_string(),
        );
        
        // Create test messages for different data types
        let commit_message = ScanMessage {
            header: MessageHeader::new(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            ),
            data: MessageData::CommitInfo {
                hash: "def456".to_string(),
                author: "Test Author 2".to_string(),
                message: "Test commit 2".to_string(),
                timestamp: 1234567891,
                changed_files: vec![],
            },
        };
        
        let file_message = ScanMessage {
            header: MessageHeader::new(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            ),
            data: MessageData::FileInfo {
                path: "src/lib.rs".to_string(),
                size: 2048,
                lines: 100,
            },
        };
        
        // Process messages through plugin executor
        let _commit_results = plugin_executor.process_message(commit_message).await;
        let _file_results = plugin_executor.process_message(file_message).await;
        
        // Finalize scanning (should emit DataReady events)
        let _final_aggregated = plugin_executor.finalize_scanning().await.expect("Failed to finalize scanning");
        
        // Allow events to be processed
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        
        // Verify DataReady events were emitted
        let captured_events = events.lock().await;
        
        // Filter for DataReady events
        let data_ready_events: Vec<_> = captured_events.iter()
            .filter_map(|e| match e {
                ScanEvent::DataReady { plugin_id, data_type, .. } => Some((plugin_id, data_type)),
                _ => None,
            })
            .collect();
        
        // Should have DataReady event for the plugin that processed data
        assert!(data_ready_events.len() >= 1, "Should have at least 1 DataReady event, got {}", data_ready_events.len());
        
        // Verify scan_id consistency and plugin_id
        for event in captured_events.iter() {
            match event {
                ScanEvent::DataReady { scan_id: event_scan_id, plugin_id, .. } => {
                    assert_eq!(event_scan_id, "test_scan_456", "Scan ID should be consistent");
                    assert_eq!(plugin_id, "test-scanner", "Plugin ID should match registered plugin");
                }
                _ => {} // Ignore other event types
            }
        }
    }
}
    #[tokio::test]
    async fn test_scan_warning_events_emission() {
        // Create notification manager
        let mut notification_manager = AsyncNotificationManager::<ScanEvent>::new();
        
        // Mock subscriber to capture events
        let events = std::sync::Arc::new(tokio::sync::Mutex::new(Vec::<ScanEvent>::new()));
        let events_clone = events.clone();
        
        // Create a mock subscriber
        struct MockSubscriber {
            events: std::sync::Arc<tokio::sync::Mutex<Vec<ScanEvent>>>,
        }
        
        #[async_trait]
        impl Subscriber<ScanEvent> for MockSubscriber {
            async fn handle_event(&self, event: ScanEvent) -> NotificationResult<()> {
                self.events.lock().await.push(event);
                Ok(())
            }
            
            fn subscriber_id(&self) -> &str {
                "test_subscriber"
            }
        }
        
        let subscriber = std::sync::Arc::new(MockSubscriber { events: events_clone });
        
        // Subscribe to events
        notification_manager.subscribe(subscriber).await.expect("Failed to subscribe");
        
        // Create scanner publisher
        let scanner_publisher = ScannerPublisher::new(std::sync::Arc::new(notification_manager));
        
        // Create plugin registry (empty to trigger "plugin not found" warning)
        let plugin_registry = SharedPluginRegistry::new();
        
        let plugin_executor = PluginExecutor::with_scanner_publisher(
            plugin_registry,
            scanner_publisher.clone(),
            "test_scan_warning".to_string(),
        );
        
        // Create test message
        let test_message = ScanMessage {
            header: MessageHeader::new(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            ),
            data: MessageData::CommitInfo {
                hash: "warning123".to_string(),
                author: "Test Author".to_string(),
                message: "Test commit for warning".to_string(),
                timestamp: 1234567892,
                changed_files: vec![],
            },
        };
        
        // Process message through plugin executor (should emit ScanWarning for missing plugin)
        let _results = plugin_executor.process_message(test_message).await;
        
        // Allow events to be processed
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        
        // Verify ScanWarning events were emitted
        let captured_events = events.lock().await;
        
        // Filter for ScanWarning events
        let warning_events: Vec<_> = captured_events.iter()
            .filter_map(|e| match e {
                ScanEvent::ScanWarning { warning, recoverable, .. } => Some((warning, recoverable)),
                _ => None,
            })
            .collect();
        
        // Should have ScanWarning event for missing plugin
        assert!(warning_events.len() >= 1, "Should have at least 1 ScanWarning event, got {}", warning_events.len());
        
        // Verify warning content and recoverability
        let found_plugin_warning = warning_events.iter().any(|(warning, recoverable)| {
            warning.contains("Plugin") && warning.contains("not found in registry") && **recoverable
        });
        assert!(found_plugin_warning, "Should have warning about plugin not found in registry");
        
        // Verify scan_id consistency
        for event in captured_events.iter() {
            match event {
                ScanEvent::ScanWarning { scan_id: event_scan_id, .. } => {
                    assert_eq!(event_scan_id, "test_scan_warning", "Scan ID should be consistent");
                }
                _ => {} // Ignore other event types
            }
        }
    }
