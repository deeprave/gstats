# Plugin Coordination Integration Guide

This guide demonstrates how to integrate the plugin coordination system with scanner shutdown for graceful system termination.

## Overview

The plugin coordination system provides lifecycle management that ensures:
- Plugins can signal when they're actively processing work
- Scanner waits for plugins to complete before shutdown  
- Proper error handling and timeout management
- Backward compatibility with existing code

## Basic Integration

### 1. Scanner Engine Setup

```rust
use std::sync::Arc;
use std::time::Duration;
use gstats::scanner::{AsyncScannerEngineBuilder, ScannerConfig, CallbackMessageProducer};
use gstats::plugin::SharedPluginRegistry;

// Create plugin registry and register plugins
let plugin_registry = SharedPluginRegistry::new();

// Register your plugins
plugin_registry.register_plugin(Box::new(MyPlugin::new())).await?;

// Create scanner engine with plugin coordination
let engine = AsyncScannerEngineBuilder::new()
    .repository_path("/path/to/repo")
    .config(ScannerConfig::default())
    .message_producer(Arc::new(CallbackMessageProducer::new("scanner".to_string())))
    .plugin_registry(plugin_registry.clone())  // Enable coordination
    .build()?;
```

### 2. Graceful Shutdown

```rust
// Perform coordinated shutdown
match engine.graceful_shutdown(Duration::from_secs(30)).await {
    Ok(()) => {
        log::info!("Scanner shutdown completed - all plugins finished work");
    }
    Err(e) => {
        log::warn!("Scanner shutdown timeout: {}", e);
        // System will exit anyway, but some plugin work may be lost
    }
}
```

## Plugin State Management

### Plugin Implementation

Plugins should transition states during their execution:

```rust
use gstats::plugin::traits::{Plugin, PluginState};

impl Plugin for MyPlugin {
    async fn execute(&self, request: PluginRequest) -> PluginResult<PluginResponse> {
        // Signal that we're starting work
        registry.transition_plugin_state(&self.name(), PluginState::Processing).await?;
        
        // Perform the actual work
        let result = self.process_data(&request).await;
        
        // Signal that we're done (whether success or failure)
        match result {
            Ok(data) => {
                registry.transition_plugin_state(&self.name(), PluginState::Initialized).await?;
                Ok(PluginResponse::success(data))
            }
            Err(e) => {
                registry.transition_plugin_state(&self.name(), PluginState::Error(e.to_string())).await?;
                Err(e)
            }
        }
    }
}
```

### State Transitions

- **`Initialized`** - Plugin is idle, ready for shutdown
- **`Processing`** - Plugin is actively working, blocks shutdown  
- **`Error(msg)`** - Plugin failed, treated as idle (doesn't block shutdown)

## Integration with Application

### Complete Example

```rust
pub async fn run_scanner_with_coordination(
    repo_path: PathBuf,
    args: &Args,
    config: &ConfigManager,
) -> Result<()> {
    // Create plugin registry
    let plugin_registry = SharedPluginRegistry::new();
    
    // Initialize plugins
    initialize_builtin_plugins(&plugin_registry).await?;
    
    // Create scanner with coordination
    let mut engine_builder = AsyncScannerEngineBuilder::new()
        .repository_path(repo_path)
        .config(scanner_config)
        .message_producer(message_producer)
        .plugin_registry(plugin_registry.clone()); // Enable coordination
    
    let engine = engine_builder.build()?;
    
    // Run scanning
    match engine.scan().await {
        Ok(()) => log::info!("Scanning completed successfully"),
        Err(e) => {
            log::error!("Scanning failed: {}", e);
            return Err(e.into());
        }
    }
    
    // Perform coordinated shutdown
    log::info!("Starting coordinated shutdown...");
    match engine.graceful_shutdown(Duration::from_secs(30)).await {
        Ok(()) => log::info!("Graceful shutdown completed"),
        Err(e) => log::warn!("Shutdown coordination timeout: {}", e),
    }
    
    Ok(())
}
```

## Configuration Options

### Timeout Configuration

Choose appropriate timeouts based on your plugin workloads:

```rust
// Quick shutdown for lightweight plugins
engine.graceful_shutdown(Duration::from_secs(5)).await?;

// Extended timeout for complex processing
engine.graceful_shutdown(Duration::from_secs(60)).await?;

// Development/testing with short timeout
engine.graceful_shutdown(Duration::from_millis(100)).await?;
```

### Error Handling Strategies

```rust
match engine.graceful_shutdown(timeout).await {
    Ok(()) => {
        log::info!("Clean shutdown - all plugin work completed");
    }
    Err(e) => {
        log::warn!("Coordination timeout: {}", e);
        
        // Option 1: Proceed anyway (may lose some work)
        log::warn!("Proceeding with shutdown despite timeout");
        
        // Option 2: Force immediate shutdown
        log::warn!("Forcing immediate shutdown");
        engine.cancel().await;
        
        // Option 3: Return error to caller
        return Err(e.into());
    }
}
```

## Backward Compatibility

The coordination system is fully backward compatible:

```rust
// Without plugin registry - immediate shutdown (existing behavior)
let engine = AsyncScannerEngineBuilder::new()
    .repository_path("/path/to/repo")
    .message_producer(producer)
    // No .plugin_registry() call
    .build()?;

// graceful_shutdown() still works, just completes immediately
engine.graceful_shutdown(Duration::from_secs(30)).await?; // Returns Ok(()) immediately
```

## Best Practices

1. **State Transitions**: Always transition plugins back to `Initialized` or `Error` after completing work
2. **Timeout Selection**: Use reasonable timeouts based on expected plugin processing times
3. **Error Handling**: Handle coordination timeouts gracefully in your application
4. **Logging**: Enable debug logging to troubleshoot coordination issues
5. **Testing**: Test both successful coordination and timeout scenarios

## Troubleshooting

### Common Issues

1. **Plugins stuck in Processing state**: Check that plugins properly transition back to `Initialized`
2. **Immediate timeouts**: Verify plugin registry is set on scanner engine
3. **Coordination not working**: Ensure plugins are registered and activated properly

### Debug Logging

Enable debug logging to see coordination details:

```rust
log::debug!("Plugin '{}' transitioning to: {:?}", name, state);
log::debug!("All active plugins are idle: {}", registry.are_all_active_plugins_idle());
```

This integration provides robust, coordinated shutdown that ensures plugin work is completed properly while maintaining system responsiveness through configurable timeouts.