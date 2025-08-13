//! Scanner Shutdown Coordination Tests
//! 
//! Tests for integrating plugin coordination with scanner shutdown logic.

use std::sync::Arc;
use std::time::Duration;
use crate::scanner::async_engine::engine::AsyncScannerEngine;
use crate::scanner::config::ScannerConfig;
use crate::scanner::CallbackMessageProducer;
use crate::plugin::{SharedPluginRegistry, traits::PluginState};
use crate::plugin::tests::mock_plugins::MockPlugin;

/// Test basic coordination functionality
#[tokio::test]
async fn test_basic_coordination() {
    // Create plugin registry
    let registry = SharedPluginRegistry::new();
    
    // Register a mock plugin
    let plugin = MockPlugin::new("test-plugin", false);
    registry.register_plugin(Box::new(plugin)).await.unwrap();
    
    // Check initial state
    {
        let reg = registry.inner().read().await;
        let active_plugins = reg.get_active_plugins();
        println!("Active plugins: {:?}", active_plugins);
        assert!(!active_plugins.is_empty(), "Should have active plugins");
        assert!(reg.are_all_active_plugins_idle(), "Should initially be idle");
    }
    
    // Put plugin into processing state
    {
        let mut reg = registry.inner().write().await;
        reg.transition_plugin_state("test-plugin", PluginState::Processing).await.unwrap();
    }
    
    // Check state - should not be idle
    {
        let reg = registry.inner().read().await;
        assert!(!reg.are_all_active_plugins_idle(), "Should not be idle when processing");
    }
    
    // Return plugin to idle state
    {
        let mut reg = registry.inner().write().await;
        reg.transition_plugin_state("test-plugin", PluginState::Initialized).await.unwrap();
    }
    
    // Check final state - should be idle again
    {
        let reg = registry.inner().read().await;
        assert!(reg.are_all_active_plugins_idle(), "Should be idle after returning to Initialized");
    }
    
    // Test wait functionality with short timeout - should succeed immediately
    {
        let reg = registry.inner().read().await;
        let result = reg.wait_for_all_plugins_idle(std::time::Duration::from_millis(10)).await;
        assert!(result.is_ok(), "Wait should succeed when plugins are already idle");
    }
}

/// Test scanner graceful shutdown without plugins first
#[tokio::test]
async fn test_scanner_shutdown_no_plugins() {
    let config = ScannerConfig::default();
    let producer = Arc::new(CallbackMessageProducer::new("test".to_string()));
    let repo_path = std::env::current_dir().unwrap();
    
    let engine = AsyncScannerEngine::new_for_test(repo_path, config, producer).unwrap();
    // No plugin registry set
    
    // Should complete immediately
    let shutdown_result = engine.graceful_shutdown(Duration::from_millis(100)).await;
    assert!(shutdown_result.is_ok(), "Scanner shutdown without plugins should succeed: {:?}", shutdown_result);
}

/// Test scanner graceful shutdown with idle plugins
#[tokio::test]
async fn test_scanner_shutdown_idle_plugins() {
    // Create plugin registry
    let registry = SharedPluginRegistry::new();
    
    // Register a mock plugin (starts in idle state)
    let plugin = MockPlugin::new("test-plugin", false);
    registry.register_plugin(Box::new(plugin)).await.unwrap();
    
    // Create scanner
    let config = ScannerConfig::default();
    let producer = Arc::new(CallbackMessageProducer::new("test".to_string()));
    let repo_path = std::env::current_dir().unwrap();
    
    let mut engine = AsyncScannerEngine::new_for_test(repo_path, config, producer).unwrap();
    engine.set_plugin_registry(registry.clone());
    
    // Should complete immediately since plugin is already idle
    let shutdown_result = engine.graceful_shutdown(Duration::from_millis(100)).await;
    assert!(shutdown_result.is_ok(), "Scanner shutdown with idle plugins should succeed: {:?}", shutdown_result);
}

/// Test that scanner waits for all plugins to be idle before shutdown
#[tokio::test]
async fn test_scanner_waits_for_plugin_coordination() {
    // Create plugin registry
    let registry = SharedPluginRegistry::new();
    
    // Register a mock plugin
    let plugin = MockPlugin::new("test-plugin", false);
    registry.register_plugin(Box::new(plugin)).await.unwrap();
    
    // Create scanner with plugin registry integration
    let config = ScannerConfig::default();
    let producer = Arc::new(CallbackMessageProducer::new("test".to_string()));
    let repo_path = std::env::current_dir().unwrap();
    
    let mut engine = AsyncScannerEngine::new_for_test(repo_path, config, producer).unwrap();
    engine.set_plugin_registry(registry.clone());
    
    // Test: Start with plugin in idle state, then transition to processing, then back to idle
    
    // Verify plugin starts idle
    {
        let reg = registry.inner().read().await;
        assert!(reg.are_all_active_plugins_idle(), "Plugin should start idle");
    }
    
    // Put plugin into processing state
    {
        let mut reg = registry.inner().write().await;
        reg.transition_plugin_state("test-plugin", PluginState::Processing).await.unwrap();
    }
    
    // Verify plugin is now processing
    {
        let reg = registry.inner().read().await;
        assert!(!reg.are_all_active_plugins_idle(), "Plugin should be processing");
    }
    
    // Start shutdown process in background
    let shutdown_task = tokio::spawn(async move {
        engine.graceful_shutdown(Duration::from_millis(500)).await
    });
    
    // Return plugin to idle state after short delay to simulate work completion
    tokio::time::sleep(Duration::from_millis(50)).await;
    {
        let mut reg = registry.inner().write().await;
        reg.transition_plugin_state("test-plugin", PluginState::Initialized).await.unwrap();
    }
    
    // Verify plugin is idle again
    {
        let reg = registry.inner().read().await;
        assert!(reg.are_all_active_plugins_idle(), "Plugin should be idle again");
    }
    
    // Scanner graceful shutdown should complete successfully
    let shutdown_result = shutdown_task.await.unwrap();
    assert!(shutdown_result.is_ok(), "Scanner should wait for plugin coordination: {:?}", shutdown_result);
}

/// Test scanner timeout during plugin coordination
#[tokio::test]
async fn test_scanner_shutdown_timeout() {
    // Create plugin registry
    let registry = SharedPluginRegistry::new();
    
    // Register a mock plugin
    let plugin = MockPlugin::new("stuck-plugin", false);
    registry.register_plugin(Box::new(plugin)).await.unwrap();
    
    // Create scanner
    let config = ScannerConfig::default();
    let producer = Arc::new(CallbackMessageProducer::new("test".to_string()));
    let repo_path = std::env::current_dir().unwrap();
    
    let mut engine = AsyncScannerEngine::new_for_test(repo_path, config, producer).unwrap();
    engine.set_plugin_registry(registry.clone());
    
    // Put plugin into processing state and leave it there
    {
        let mut reg = registry.inner().write().await;
        reg.transition_plugin_state("stuck-plugin", PluginState::Processing).await.unwrap();
    }
    
    // Scanner shutdown should timeout
    let shutdown_result = engine.graceful_shutdown(Duration::from_millis(50)).await;
    assert!(shutdown_result.is_err(), "Scanner should timeout when plugins don't become idle");
}

/// Test graceful shutdown with multiple plugins
#[tokio::test]
async fn test_scanner_shutdown_multiple_plugins() {
    // Create plugin registry
    let registry = SharedPluginRegistry::new();
    
    // Register multiple mock plugins
    let plugin1 = MockPlugin::new("plugin1", false);
    let plugin2 = MockPlugin::new("plugin2", false);
    registry.register_plugin(Box::new(plugin1)).await.unwrap();
    registry.register_plugin(Box::new(plugin2)).await.unwrap();
    
    // Create scanner
    let config = ScannerConfig::default();
    let producer = Arc::new(CallbackMessageProducer::new("test".to_string()));
    let repo_path = std::env::current_dir().unwrap();
    
    let mut engine = AsyncScannerEngine::new_for_test(repo_path, config, producer).unwrap();
    engine.set_plugin_registry(registry.clone());
    
    // Put one plugin into processing state
    {
        let mut reg = registry.inner().write().await;
        reg.transition_plugin_state("plugin1", PluginState::Processing).await.unwrap();
    }
    
    // Start shutdown process in background
    let engine_arc = Arc::new(engine);
    let shutdown_engine = engine_arc.clone();
    let shutdown_task = tokio::spawn(async move {
        shutdown_engine.graceful_shutdown(Duration::from_millis(500)).await  // Increased timeout
    });
    
    // Return plugin to idle state after short delay
    tokio::time::sleep(Duration::from_millis(50)).await;
    {
        let mut reg = registry.inner().write().await;
        reg.transition_plugin_state("plugin1", PluginState::Initialized).await.unwrap();
    }
    
    // Give some time for the transition to be detected
    tokio::time::sleep(Duration::from_millis(20)).await;
    
    // Shutdown should complete successfully
    let shutdown_result = shutdown_task.await.unwrap();
    match &shutdown_result {
        Ok(()) => log::info!("Multi-plugin shutdown successful"),
        Err(e) => log::error!("Multi-plugin shutdown failed: {}", e),
    }
    assert!(shutdown_result.is_ok(), "Scanner should complete graceful shutdown when plugins become idle: {:?}", shutdown_result);
}

/// Test that scanner shutdown handles plugin errors gracefully
#[tokio::test]
async fn test_scanner_shutdown_with_plugin_errors() {
    // Create plugin registry
    let registry = SharedPluginRegistry::new();
    
    // Register mock plugins
    let plugin1 = MockPlugin::new("good-plugin", false);
    let plugin2 = MockPlugin::new("error-plugin", false);
    registry.register_plugin(Box::new(plugin1)).await.unwrap();
    registry.register_plugin(Box::new(plugin2)).await.unwrap();
    
    // Create scanner
    let config = ScannerConfig::default();
    let producer = Arc::new(CallbackMessageProducer::new("test".to_string()));
    let repo_path = std::env::current_dir().unwrap();
    
    let mut engine = AsyncScannerEngine::new_for_test(repo_path, config, producer).unwrap();
    engine.set_plugin_registry(registry.clone());
    
    // Put one plugin into error state
    {
        let mut reg = registry.inner().write().await;
        reg.transition_plugin_state("error-plugin", PluginState::Error("Test error".to_string())).await.unwrap();
    }
    
    // Scanner shutdown should still succeed (error plugins are considered "done")
    let shutdown_result = engine.graceful_shutdown(Duration::from_millis(100)).await;
    assert!(shutdown_result.is_ok(), "Scanner should handle plugin errors gracefully during shutdown");
}