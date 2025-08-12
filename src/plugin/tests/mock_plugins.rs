//! Mock Plugin Implementations for Testing
//! 
//! Provides comprehensive mock implementations for testing the plugin system.

use std::sync::{Arc, Mutex};
use std::time::Instant;
use async_trait::async_trait;
use crate::plugin::traits::*;
use crate::plugin::error::{PluginError, PluginResult};
use crate::plugin::context::{PluginContext, PluginRequest, PluginResponse, ExecutionMetadata};
use crate::scanner::messages::{ScanMessage, MessageHeader, MessageData};

/// Mock plugin for testing basic plugin functionality
pub struct MockPlugin {
    info: PluginInfo,
    state: Arc<Mutex<PluginState>>,
    execution_count: Arc<Mutex<u32>>,
    should_fail: bool,
}

impl MockPlugin {
    /// Create a new mock plugin
    pub fn new(name: &str, should_fail: bool) -> Self {
        let info = PluginInfo::new(
            name.to_string(),
            "1.0.0".to_string(),
            20250727,
            "Mock plugin for testing".to_string(),
            "Test Author".to_string(),
            PluginType::Scanner,
        );

        Self {
            info,
            state: Arc::new(Mutex::new(PluginState::Unloaded)),
            execution_count: Arc::new(Mutex::new(0)),
            should_fail,
        }
    }

    /// Get execution count
    pub fn execution_count(&self) -> u32 {
        *self.execution_count.lock().unwrap()
    }

    /// Reset execution count
    pub fn reset_execution_count(&self) {
        *self.execution_count.lock().unwrap() = 0;
    }
    
    /// Set the plugin priority
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.info.priority = priority;
        self
    }
    
    /// Add a capability to this plugin
    pub fn with_capability(mut self, name: &str, description: &str) -> Self {
        self.info.capabilities.push(PluginCapability {
            name: name.to_string(),
            description: description.to_string(),
            version: "1.0".to_string(),
            function_name: None,
            aliases: Vec::new(),
            is_default: false,
        });
        self
    }
    
    /// Add a capability to the plugin
    pub fn add_capability(&mut self, name: String, description: String, version: String) {
        self.info.capabilities.push(crate::plugin::traits::PluginCapability {
            name,
            description,
            version,
            function_name: None,
            aliases: Vec::new(),
            is_default: false,
        });
    }
}

#[async_trait]
impl Plugin for MockPlugin {
    fn plugin_info(&self) -> &PluginInfo {
        &self.info
    }

    async fn initialize(&mut self, _context: &PluginContext) -> PluginResult<()> {
        if self.should_fail {
            return Err(PluginError::initialization_failed("Mock initialization failure"));
        }

        *self.state.lock().unwrap() = PluginState::Initialized;
        Ok(())
    }

    async fn execute(&self, request: PluginRequest) -> PluginResult<PluginResponse> {
        if self.should_fail {
            return Err(PluginError::execution_failed("Mock execution failure"));
        }

        let start_time = Instant::now();
        *self.execution_count.lock().unwrap() += 1;
        *self.state.lock().unwrap() = PluginState::Running;

        // Simulate some work
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let duration = start_time.elapsed();
        let metadata = ExecutionMetadata::new(
            duration.as_millis() as u64,
            1024, // Mock memory usage
            1,    // Mock items processed
            self.info.version.clone(),
        );

        *self.state.lock().unwrap() = PluginState::Initialized;

        Ok(PluginResponse::success(
            request.request_id().unwrap_or("unknown").to_string(),
            serde_json::json!({"result": "mock_success"}),
            metadata,
        ))
    }

    async fn cleanup(&mut self) -> PluginResult<()> {
        if self.should_fail {
            return Err(PluginError::generic("Mock cleanup failure"));
        }

        *self.state.lock().unwrap() = PluginState::Unloaded;
        Ok(())
    }

    fn plugin_state(&self) -> PluginState {
        self.state.lock().unwrap().clone()
    }
}

/// Mock scanner plugin for testing scanner-specific functionality
pub struct MockScannerPlugin {
    base: MockPlugin,
}

impl MockScannerPlugin {
    /// Create a new mock scanner plugin
    pub fn new(name: &str, should_fail: bool) -> Self {
        Self {
            base: MockPlugin::new(name, should_fail),
        }
    }

    /// Get the base plugin for accessing shared functionality
    pub fn base(&self) -> &MockPlugin {
        &self.base
    }
}

#[async_trait]
impl Plugin for MockScannerPlugin {
    fn plugin_info(&self) -> &PluginInfo {
        self.base.plugin_info()
    }

    async fn initialize(&mut self, context: &PluginContext) -> PluginResult<()> {
        self.base.initialize(context).await
    }

    async fn execute(&self, request: PluginRequest) -> PluginResult<PluginResponse> {
        self.base.execute(request).await
    }

    async fn cleanup(&mut self) -> PluginResult<()> {
        self.base.cleanup().await
    }

    fn plugin_state(&self) -> PluginState {
        self.base.plugin_state()
    }

    /// Override to provide ScannerPlugin access
    fn as_scanner_plugin(&self) -> Option<&dyn ScannerPlugin> {
        Some(self)
    }
}

#[async_trait]
impl ScannerPlugin for MockScannerPlugin {

    async fn process_scan_data(&self, data: &ScanMessage) -> PluginResult<Vec<ScanMessage>> {
        if self.base.should_fail {
            return Err(PluginError::execution_failed("Mock scan processing failure"));
        }

        // Mock processing - just return the input data with modified timestamp
        let mut processed = data.clone();
        if let Ok(header) = bincode::deserialize::<MessageHeader>(&bincode::serialize(&processed.header).unwrap()) {
            let new_header = MessageHeader::new(header.sequence + 1);
            processed.header = new_header;
        }

        Ok(vec![processed])
    }

    async fn aggregate_results(&self, results: Vec<ScanMessage>) -> PluginResult<ScanMessage> {
        if self.base.should_fail {
            return Err(PluginError::execution_failed("Mock aggregation failure"));
        }

        // Mock aggregation - create a summary message
        let header = MessageHeader::new(0);
        let data = MessageData::MetricInfo {
            file_count: results.len() as u32,
            line_count: results.len() as u64 * 100, // Mock line count
            complexity: results.len() as f64 * 1.5,  // Mock complexity
        };

        Ok(ScanMessage::new(header, data))
    }

    fn estimate_processing_time(&self, item_count: usize) -> Option<std::time::Duration> {
        // Mock estimation: 1ms per item
        Some(std::time::Duration::from_millis(item_count as u64))
    }

    fn config_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "mock_setting": {
                    "type": "string",
                    "description": "Mock configuration setting"
                }
            }
        })
    }
}

/// Mock notification plugin for testing notification functionality
pub struct MockNotificationPlugin {
    base: MockPlugin,
    notifications_received: Arc<Mutex<Vec<String>>>,
    pub preferences: NotificationPreferences,
}

impl MockNotificationPlugin {
    /// Create a new mock notification plugin
    pub fn new(name: &str, should_fail: bool) -> Self {
        let mut base = MockPlugin::new(name, should_fail);
        // Change plugin type to Notification
        base.info.plugin_type = PluginType::Notification;
        Self {
            base,
            notifications_received: Arc::new(Mutex::new(Vec::new())),
            preferences: NotificationPreferences {
                queue_updates: true,
                scan_progress: true,
                error_notifications: true,
                system_events: vec![
                    SystemEventType::SystemStartup,
                    SystemEventType::SystemShutdown,
                    SystemEventType::ConfigurationChanged,
                    SystemEventType::MemoryWarning,
                    SystemEventType::PerformanceAlert,
                ],
                max_frequency: Some(100), // High frequency for testing
            },
        }
    }

    /// Get received notifications
    pub fn received_notifications(&self) -> Vec<String> {
        self.notifications_received.lock().unwrap().clone()
    }

    /// Clear received notifications
    pub fn clear_notifications(&self) {
        self.notifications_received.lock().unwrap().clear();
    }

    /// Get the base plugin for accessing shared functionality
    pub fn base(&self) -> &MockPlugin {
        &self.base
    }
}

#[async_trait]
impl Plugin for MockNotificationPlugin {
    fn plugin_info(&self) -> &PluginInfo {
        self.base.plugin_info()
    }

    async fn initialize(&mut self, context: &PluginContext) -> PluginResult<()> {
        self.base.initialize(context).await
    }

    async fn execute(&self, request: PluginRequest) -> PluginResult<PluginResponse> {
        self.base.execute(request).await
    }

    async fn cleanup(&mut self) -> PluginResult<()> {
        self.base.cleanup().await
    }

    fn plugin_state(&self) -> PluginState {
        self.base.plugin_state()
    }
}

#[async_trait]
impl NotificationPlugin for MockNotificationPlugin {
    async fn on_queue_update(&self, update: QueueUpdate) -> PluginResult<()> {
        if self.base.should_fail {
            return Err(PluginError::notification_failed("Mock queue update failure"));
        }

        self.notifications_received.lock().unwrap().push(
            format!("queue_update:{}:{:?}", update.queue_id, update.update_type)
        );
        Ok(())
    }

    async fn on_scan_progress(&self, progress: ScanProgress) -> PluginResult<()> {
        if self.base.should_fail {
            return Err(PluginError::notification_failed("Mock scan progress failure"));
        }

        self.notifications_received.lock().unwrap().push(
            format!("scan_progress:{}:{}", progress.scan_id, progress.entries_processed)
        );
        Ok(())
    }

    async fn on_error(&self, error: PluginError) -> PluginResult<()> {
        if self.base.should_fail {
            return Err(PluginError::notification_failed("Mock error notification failure"));
        }

        self.notifications_received.lock().unwrap().push(
            format!("error:{}", error.to_string())
        );
        Ok(())
    }

    async fn on_system_event(&self, event: SystemEvent) -> PluginResult<()> {
        if self.base.should_fail {
            return Err(PluginError::notification_failed("Mock system event failure"));
        }

        self.notifications_received.lock().unwrap().push(
            format!("system_event:{:?}", event.event_type)
        );
        Ok(())
    }

    fn notification_preferences(&self) -> NotificationPreferences {
        self.preferences.clone()
    }
}

/// Helper function to create a test plugin context
pub fn create_test_context() -> PluginContext {
    use crate::scanner::{ScannerConfig, QueryParams};
    // Removed unused import: crate::git
    use std::sync::Arc;

    let scanner_config = Arc::new(ScannerConfig::default());
    let query_params = Arc::new(QueryParams::default());

    PluginContext::new(scanner_config, query_params)
}

/// Helper function to create a test plugin request
pub fn create_test_request() -> PluginRequest {
    use crate::plugin::context::{PluginRequest, RequestPriority};

    PluginRequest::new()
        .with_priority(RequestPriority::Normal)
        .with_timeout(5000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_plugin_basic_functionality() {
        let mut plugin = MockPlugin::new("test-plugin", false);
        let context = create_test_context();

        // Test plugin info
        assert_eq!(plugin.plugin_info().name, "test-plugin");
        assert_eq!(plugin.plugin_info().version, "1.0.0");

        // Test initialization
        assert_eq!(plugin.plugin_state(), PluginState::Unloaded);
        plugin.initialize(&context).await.unwrap();
        assert_eq!(plugin.plugin_state(), PluginState::Initialized);

        // Test execution
        let request = create_test_request();
        let response = plugin.execute(request).await.unwrap();
        assert!(response.is_success());
        assert_eq!(plugin.execution_count(), 1);

        // Test cleanup
        plugin.cleanup().await.unwrap();
        assert_eq!(plugin.plugin_state(), PluginState::Unloaded);
    }

    #[tokio::test]
    async fn test_mock_plugin_failure_scenarios() {
        let mut plugin = MockPlugin::new("failing-plugin", true);
        let context = create_test_context();

        // Test initialization failure
        let result = plugin.initialize(&context).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PluginError::InitializationFailed { .. }));

        // Force initialize for further tests
        plugin.should_fail = false;
        plugin.initialize(&context).await.unwrap();
        plugin.should_fail = true;

        // Test execution failure
        let request = create_test_request();
        let result = plugin.execute(request).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PluginError::ExecutionFailed { .. }));

        // Test cleanup failure
        let result = plugin.cleanup().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mock_scanner_plugin() {
        let mut plugin = MockScannerPlugin::new("scanner-plugin", false);
        let context = create_test_context();

        plugin.initialize(&context).await.unwrap();

        // Test supported modes
        // Plugin no longer advertises supported modes

        // Test processing time estimation
        let estimate = plugin.estimate_processing_time(100);
        assert!(estimate.is_some());
        assert_eq!(estimate.unwrap().as_millis(), 100);

        // Test unsupported mode estimation

        // Test config schema
        let schema = plugin.config_schema();
        assert!(schema.is_object());
    }

    #[tokio::test]
    async fn test_mock_notification_plugin() {
        let mut plugin = MockNotificationPlugin::new("notification-plugin", false);
        let context = create_test_context();

        plugin.initialize(&context).await.unwrap();

        // Test queue update notification
        let queue_update = QueueUpdate::new(
            "test-queue".to_string(),
            QueueUpdateType::MessageEnqueued,
            10,
            1024
        );
        plugin.on_queue_update(queue_update).await.unwrap();

        // Test scan progress notification
        let scan_progress = ScanProgress::new(
            "test-scan".to_string(),
            5,
            "processing".to_string()
        );
        plugin.on_scan_progress(scan_progress).await.unwrap();

        // Test error notification
        let error = PluginError::generic("Test error");
        plugin.on_error(error).await.unwrap();

        // Test system event notification
        let event = SystemEvent::new(
            SystemEventType::SystemStartup,
            serde_json::json!({"timestamp": "now"})
        );
        plugin.on_system_event(event).await.unwrap();

        // Verify notifications were received
        let notifications = plugin.received_notifications();
        assert_eq!(notifications.len(), 4);
        assert!(notifications[0].starts_with("queue_update:test-queue"));
        assert!(notifications[1].starts_with("scan_progress:test-scan"));
        assert!(notifications[2].starts_with("error:"));
        assert!(notifications[3].starts_with("system_event:"));

        // Test clearing notifications
        plugin.clear_notifications();
        assert_eq!(plugin.received_notifications().len(), 0);
    }

    #[tokio::test]
    async fn test_concurrent_plugin_execution() {
        let plugin = Arc::new(MockPlugin::new("concurrent-plugin", false));
        let context = create_test_context();

        // Initialize the plugin (need mutable reference for this)
        {
            let mut plugin_mut = MockPlugin::new("concurrent-plugin", false);
            plugin_mut.initialize(&context).await.unwrap();
        }

        // Create multiple concurrent requests
        let mut handles = Vec::new();
        for i in 0..10 {
            let plugin_clone = Arc::clone(&plugin);
            let handle = tokio::spawn(async move {
                let request = create_test_request()
                    .with_parameter("index".to_string(), i);
                plugin_clone.execute(request).await
            });
            handles.push(handle);
        }

        // Wait for all requests to complete
        let mut success_count = 0;
        for handle in handles {
            match handle.await.unwrap() {
                Ok(response) => {
                    assert!(response.is_success());
                    success_count += 1;
                }
                Err(_) => {}
            }
        }

        assert_eq!(success_count, 10);
    }
}