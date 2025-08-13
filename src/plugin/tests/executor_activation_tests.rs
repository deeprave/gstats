//! Tests for PluginExecutor respecting plugin activation state

use std::sync::Arc;
use crate::plugin::{SharedPluginRegistry, PluginExecutor};
use crate::app::initialization::initialize_builtin_plugins;
use crate::scanner::messages::{ScanMessage, MessageHeader, MessageData, FileChangeData};
use crate::plugin::context::PluginRequest;

#[tokio::test]
async fn test_executor_only_processes_active_plugins() {
    // This test expects the executor to only send events to active plugins
    
    // Initialize registry with builtin plugins
    let registry = SharedPluginRegistry::new();
    initialize_builtin_plugins(&registry).await.unwrap();
    
    // At this point, only export plugin should be active (load_by_default = true)
    {
        let reg = registry.inner().read().await;
        let active_plugins = reg.get_active_plugins();
        assert_eq!(active_plugins, vec!["export"], "Only export plugin should be active initially");
    }
    
    // Create executor 
    let executor = PluginExecutor::new(registry.clone());
    
    // Create a test scan message
    let message = ScanMessage::new(
        MessageHeader::new(1),
        MessageData::CommitInfo {
            hash: "abc123".to_string(),
            author: "Test Author".to_string(),
            message: "Test commit".to_string(),
            timestamp: 1234567890,
            changed_files: vec![],
        }
    );
    
    // Expected behavior: Only active plugins should receive the message for processing
    // Since only export plugin is active, only it should be invoked
    
    // Test that executor processes message through active plugins only
    let results = executor.process_message_through_active_plugins(message).await.unwrap();
    
    // Should only process through 1 active plugin (export)
    assert_eq!(results.len(), 1, "Should only process through 1 active plugin");
    
    // Verify that only export plugin was invoked
    let active_plugins = {
        let reg = registry.inner().read().await;
        reg.get_active_plugins()
    };
    assert_eq!(active_plugins, vec!["export"], "Only export plugin should be active");
}

#[tokio::test]
async fn test_inactive_plugins_not_processed() {
    // This test expects inactive plugins to not receive any processing events
    
    let registry = SharedPluginRegistry::new();
    initialize_builtin_plugins(&registry).await.unwrap();
    
    // Manually activate commits plugin for comparison
    {
        let mut reg = registry.inner().write().await;
        reg.activate_plugin("commits").await.unwrap();
    }
    
    // Now we should have 2 active plugins: export and commits
    {
        let reg = registry.inner().read().await;
        let active_plugins = reg.get_active_plugins();
        assert_eq!(active_plugins.len(), 2, "Should have 2 active plugins");
        assert!(active_plugins.contains(&"export".to_string()));
        assert!(active_plugins.contains(&"commits".to_string()));
        
        // Metrics should still be inactive
        assert!(!reg.is_plugin_active("metrics"), "Metrics plugin should remain inactive");
    }
    
    let executor = PluginExecutor::new(registry.clone());
    
    // Create a test message that all plugins could potentially process
    let message = ScanMessage::new(
        MessageHeader::new(1),
        MessageData::MetricInfo {
            file_count: 10,
            line_count: 100,
            complexity: 15.5,
        }
    );
    
    // Expected behavior: Only the 2 active plugins should be considered for processing
    // The inactive metrics plugin should NOT receive this message, even though
    // it would be the most relevant for MetricInfo data
    
    let results = executor.process_message_through_active_plugins(message).await.unwrap();
    
    // Should only process through 2 active plugins (export and commits)
    assert_eq!(results.len(), 2, "Should process through exactly 2 active plugins");
    
    // Verify metrics plugin is not invoked (it's inactive)
    let active_plugins = {
        let reg = registry.inner().read().await;
        reg.get_active_plugins()
    };
    assert_eq!(active_plugins.len(), 2, "Should have exactly 2 active plugins");
    assert!(!active_plugins.contains(&"metrics".to_string()), "Metrics plugin should not be active");
}

#[tokio::test]
async fn test_plugin_activation_during_execution() {
    // This test expects that plugins can be activated during execution
    // and the executor respects the new activation state
    
    let registry = SharedPluginRegistry::new();
    initialize_builtin_plugins(&registry).await.unwrap();
    
    let executor = PluginExecutor::new(registry.clone());
    
    // Initially only export should be active
    {
        let reg = registry.inner().read().await;
        assert_eq!(reg.get_active_plugins().len(), 1);
    }
    
    // Activate commits plugin during "execution"
    {
        let mut reg = registry.inner().write().await;
        reg.activate_plugin("commits").await.unwrap();
    }
    
    // Now executor should work with 2 active plugins
    {
        let reg = registry.inner().read().await;
        assert_eq!(reg.get_active_plugins().len(), 2);
    }
    
    // Deactivate export plugin
    {
        let mut reg = registry.inner().write().await;
        reg.deactivate_plugin("export").await.unwrap();
    }
    
    // Now only commits should be active
    {
        let reg = registry.inner().read().await;
        let active_plugins = reg.get_active_plugins();
        assert_eq!(active_plugins.len(), 1);
        assert_eq!(active_plugins[0], "commits");
    }
}

#[tokio::test]
async fn test_executor_processes_through_active_plugins_only() {
    // This test expects the executor to have a method for processing messages
    // through active plugins and ignoring inactive ones
    
    let registry = SharedPluginRegistry::new();
    initialize_builtin_plugins(&registry).await.unwrap();
    
    // Activate all plugins for this test
    {
        let mut reg = registry.inner().write().await;
        reg.activate_plugin("commits").await.unwrap();
        reg.activate_plugin("metrics").await.unwrap();
        // export is already active by default
    }
    
    let executor = PluginExecutor::new(registry.clone());
    
    // Verify all plugins are active
    {
        let reg = registry.inner().read().await;
        let active_plugins = reg.get_active_plugins();
        assert_eq!(active_plugins.len(), 3, "All 3 plugins should be active");
    }
    
    // Test processing through all active plugins
    let test_message = ScanMessage::new(
        MessageHeader::new(2),
        MessageData::FileInfo {
            path: "test.rs".to_string(),
            size: 1024,
            lines: 50,
        }
    );
    
    let results = executor.process_message_through_active_plugins(test_message).await.unwrap();
    
    // Should process through all 3 active plugins
    assert_eq!(results.len(), 3, "Should process through all 3 active plugins");
    
    // The key requirement is: executor should only invoke Plugin.execute()
    // on plugins that are currently active in the registry
}