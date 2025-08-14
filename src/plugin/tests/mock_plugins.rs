//! Mock Plugin Implementations for Testing
//! 
//! Provides comprehensive mock implementations for testing the plugin system.

use std::sync::{Arc, Mutex};
use std::time::Instant;
use async_trait::async_trait;
use crate::plugin::traits::*;
use crate::plugin::error::{PluginError, PluginResult};
use crate::plugin::context::{PluginContext, PluginRequest, PluginResponse, ExecutionMetadata};

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
            PluginType::Processing,
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
    
    /// Set whether plugin should be activated by default
    pub fn with_load_by_default(mut self, load_by_default: bool) -> Self {
        self.info.load_by_default = load_by_default;
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

/// Data requirements implementation for MockPlugin
impl PluginDataRequirements for MockPlugin {
    fn requires_current_file_content(&self) -> bool {
        false // Mock plugin doesn't need file content
    }
    
    fn requires_historical_file_content(&self) -> bool {
        false // Mock plugin for testing only
    }
    
    fn preferred_buffer_size(&self) -> usize {
        4096 // Small buffer for testing
    }
    
    fn max_file_size(&self) -> Option<usize> {
        Some(1024 * 1024) // 1MB limit for testing
    }
    
    fn handles_binary_files(&self) -> bool {
        false // Text files only for testing
    }
}

/// Mock processing plugin for testing processing-specific functionality
pub struct MockProcessingPlugin {
    base: MockPlugin,
}

impl MockProcessingPlugin {
    /// Create a new mock processing plugin
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
impl Plugin for MockProcessingPlugin {
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

/// Data requirements implementation for MockProcessingPlugin
impl PluginDataRequirements for MockProcessingPlugin {
    fn requires_current_file_content(&self) -> bool {
        true // Processing plugin mock might need file content for testing
    }
    
    fn requires_historical_file_content(&self) -> bool {
        false // Still mock, keep it simple
    }
    
    fn preferred_buffer_size(&self) -> usize {
        8192 // Slightly larger buffer for processing testing
    }
    
    fn max_file_size(&self) -> Option<usize> {
        Some(2 * 1024 * 1024) // 2MB limit for processing tests
    }
    
    fn handles_binary_files(&self) -> bool {
        true // Processing plugin mock can handle binary for testing
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

/// Data requirements implementation for MockNotificationPlugin
impl PluginDataRequirements for MockNotificationPlugin {
    fn requires_current_file_content(&self) -> bool {
        false // Notification plugin doesn't need file content
    }
    
    fn requires_historical_file_content(&self) -> bool {
        false // Only handles notifications, not file analysis
    }
    
    fn preferred_buffer_size(&self) -> usize {
        4096 // Small buffer for notifications
    }
    
    fn max_file_size(&self) -> Option<usize> {
        None // N/A - doesn't process files
    }
    
    fn handles_binary_files(&self) -> bool {
        false // N/A - doesn't process files
    }
}

/// Helper function to create a test plugin context
pub fn create_test_context() -> PluginContext {
    use crate::scanner::{ScannerConfig, QueryParams};
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
    async fn test_mock_processing_plugin() {
        let mut plugin = MockProcessingPlugin::new("processing-plugin", false);
        let context = create_test_context();

        plugin.initialize(&context).await.unwrap();

        // Test basic plugin functionality
        assert_eq!(plugin.plugin_info().name, "processing-plugin");
        assert_eq!(plugin.plugin_info().plugin_type, PluginType::Processing);
        
        // Test execution
        let request = create_test_request();
        let response = plugin.execute(request).await.unwrap();
        assert!(response.is_success());
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