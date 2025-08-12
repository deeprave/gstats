# Scanner-Plugin Coordination via Notification System

This document provides comprehensive documentation for using the notification system to coordinate between the scanner and plugins in gstats.

## Overview

The notification system enables event-driven communication between the scanner engine and plugins, allowing for:
- Real-time progress updates
- Data availability notifications
- Error and warning propagation
- Plugin lifecycle coordination
- Export synchronization

## Architecture

```
Scanner Engine → AsyncNotificationManager → PluginSubscriber → Plugin Event Handlers
```

### Key Components

1. **AsyncNotificationManager**: Central event distribution hub
2. **PluginSubscriber**: Wrapper that routes events to appropriate plugin handlers
3. **ScanEvent**: Enum defining all possible scanner events
4. **Plugin Event Handlers**: Methods in plugins that respond to specific events

## Event Types

### Core Scanner Events

#### ScanStarted
Emitted when a scan begins.
```rust
ScanEvent::ScanStarted {
    scan_id: String,
}
```

#### ScanDataReady
Emitted when scanner has processed data and it's ready for plugin consumption.
```rust
ScanEvent::ScanDataReady {
    scan_id: String,
    data_type: String,    // "commits", "files", "metrics", etc.
    message_count: usize,
}
```

#### ScanCompleted
Emitted when scanning is complete.
```rust
ScanEvent::ScanCompleted {
    scan_id: String,
    duration: Duration,
    warnings: Vec<String>,
}
```

### Plugin Coordination Events

#### DataReady
Emitted by analysis plugins when their processing is complete.
```rust
ScanEvent::DataReady {
    scan_id: String,
    plugin_id: String,    // "commits", "metrics", etc.
    data_type: String,
}
```

### Error and Warning Events

#### ScanError
Emitted for fatal and non-fatal errors.
```rust
ScanEvent::ScanError {
    scan_id: String,
    error: String,
    fatal: bool,
}
```

#### ScanWarning
Emitted for recoverable warnings.
```rust
ScanEvent::ScanWarning {
    scan_id: String,
    warning: String,
    recoverable: bool,
}
```

## Plugin Integration

### Setting Up Plugin Subscription

```rust
// Create notification manager
let notification_manager = Arc::new(AsyncNotificationManager::new());

// Create plugin registry with notification support
let mut registry = PluginRegistry::with_notification_manager(notification_manager.clone());

// Register plugins
registry.register_plugin(Box::new(CommitsPlugin::new())).await?;
registry.register_plugin(Box::new(MetricsPlugin::new())).await?;
registry.register_plugin(Box::new(ExportPlugin::new())).await?;

// Subscribe all plugins to events
registry.subscribe_all_plugins().await?;
```

### Implementing Event Handlers in Plugins

#### ScanDataReady Handler
```rust
impl CommitsPlugin {
    pub async fn handle_scan_data_ready(&mut self, event: ScanEvent) -> PluginResult<()> {
        match event {
            ScanEvent::ScanDataReady { scan_id, data_type, message_count } => {
                if data_type == "commits" {
                    log::info!("Processing {} commit messages for scan {}", message_count, scan_id);
                    // TODO: Fetch and process commit data from queue
                    // TODO: Emit DataReady event when processing complete
                }
                Ok(())
            }
            _ => Err(PluginError::ExecutionFailed { 
                message: "Invalid event type for handle_scan_data_ready".to_string() 
            })
        }
    }
}
```

#### Error and Warning Handlers
```rust
impl Plugin {
    pub async fn handle_scan_error(&mut self, event: ScanEvent) -> PluginResult<()> {
        match event {
            ScanEvent::ScanError { scan_id, error, fatal } => {
                if fatal {
                    log::error!("Fatal error in scan {}: {}", scan_id, error);
                    // Cleanup resources and abort processing
                    self.cleanup_resources().await?;
                } else {
                    log::warn!("Non-fatal error in scan {}: {}", scan_id, error);
                    // Continue with degraded functionality
                }
                Ok(())
            }
            _ => Err(PluginError::ExecutionFailed { 
                message: "Invalid event type for handle_scan_error".to_string() 
            })
        }
    }
}
```

## Export Plugin Coordination

The export plugin uses a sophisticated coordination system to wait for all analysis plugins before rendering results.

### Coordination Flow

1. **Data Collection**: Export plugin receives `DataReady` events from analysis plugins
2. **Readiness Check**: Verifies all expected plugins have reported completion
3. **Automatic Export**: Triggers export rendering when all plugins are ready
4. **Completion Notification**: Signals export completion for cleanup coordination

### Implementation Example

```rust
impl ExportPlugin {
    pub async fn handle_data_ready(&mut self, event: ScanEvent) -> PluginResult<()> {
        match event {
            ScanEvent::DataReady { scan_id, plugin_id, data_type } => {
                // Track collected plugin data
                self.collected_plugins.insert(plugin_id.clone(), data_type.clone());
                
                // Check if all expected plugins have reported
                if self.all_expected_plugins_ready() {
                    log::info!("All plugins ready for scan {}, triggering export", scan_id);
                    self.trigger_export_if_ready().await?;
                }
                
                Ok(())
            }
            _ => Err(PluginError::ExecutionFailed { 
                message: "Invalid event type for handle_data_ready".to_string() 
            })
        }
    }
    
    fn all_expected_plugins_ready(&self) -> bool {
        for expected_plugin in &self.expected_plugins {
            if !self.collected_plugins.contains_key(expected_plugin) {
                return false;
            }
        }
        true
    }
}
```

## Event Publishing

### From Scanner Engine

```rust
// Emit scan started event
let scan_started = ScanEvent::ScanStarted {
    scan_id: scan_id.clone(),
};
notification_manager.publish(scan_started).await?;

// Emit data ready event
let data_ready = ScanEvent::ScanDataReady {
    scan_id: scan_id.clone(),
    data_type: "commits".to_string(),
    message_count: 150,
};
notification_manager.publish(data_ready).await?;
```

### From Plugins

```rust
// Emit data ready event from analysis plugin
let data_ready = ScanEvent::DataReady {
    scan_id: scan_id.clone(),
    plugin_id: "commits".to_string(),
    data_type: "commits".to_string(),
};
notification_manager.publish(data_ready).await?;
```

## Error Handling

### Graceful Degradation

```rust
impl Plugin {
    pub async fn handle_scan_warning(&mut self, event: ScanEvent) -> PluginResult<()> {
        match event {
            ScanEvent::ScanWarning { scan_id, warning, recoverable } => {
                if recoverable {
                    log::warn!("Recoverable warning for scan {}: {}", scan_id, warning);
                    // Continue processing with reduced functionality
                    self.enable_fallback_mode();
                } else {
                    log::error!("Non-recoverable warning for scan {}: {}", scan_id, warning);
                    // May need to abort certain operations
                }
                Ok(())
            }
            _ => Err(PluginError::ExecutionFailed { 
                message: "Invalid event type for handle_scan_warning".to_string() 
            })
        }
    }
}
```

### Resource Cleanup

```rust
impl Plugin {
    pub async fn handle_scan_completed(&mut self, event: ScanEvent) -> PluginResult<()> {
        match event {
            ScanEvent::ScanCompleted { scan_id, duration, warnings } => {
                log::info!("Scan {} completed in {:?} with {} warnings", 
                          scan_id, duration, warnings.len());
                
                // Finalize processing and cleanup resources
                self.finalize_processing().await?;
                self.cleanup_temporary_data().await?;
                
                Ok(())
            }
            _ => Err(PluginError::ExecutionFailed { 
                message: "Invalid event type for handle_scan_completed".to_string() 
            })
        }
    }
}
```

## Best Practices

### Event Handler Design

1. **Idempotent Operations**: Event handlers should be safe to call multiple times
2. **Fast Processing**: Avoid blocking operations in event handlers
3. **Error Resilience**: Handle errors gracefully without crashing the system
4. **Logging**: Provide detailed logging for debugging and monitoring

### Performance Considerations

1. **Async Processing**: Use async/await for all I/O operations
2. **Memory Management**: Clean up resources promptly after processing
3. **Backpressure**: Handle high event volumes gracefully
4. **Concurrent Processing**: Design for concurrent event processing

### Testing

1. **Mock Events**: Use mock events for unit testing plugin handlers
2. **Integration Tests**: Test complete event flows end-to-end
3. **Error Scenarios**: Test error and warning event handling
4. **Concurrency**: Test concurrent event processing scenarios

## Monitoring and Debugging

### Event Statistics

```rust
// Get notification manager statistics
let stats = notification_manager.get_stats().await;
println!("Events published: {}", stats.events_published);
println!("Events delivered: {}", stats.events_delivered);
println!("Delivery failures: {}", stats.delivery_failures);
```

### Debug Logging

Enable debug logging to trace event flow:

```bash
RUST_LOG=debug gstats /path/to/repository
```

### Common Issues

1. **Missing Event Handlers**: Ensure all plugins implement required event handlers
2. **Subscription Failures**: Verify plugins are properly subscribed to notifications
3. **Event Ordering**: Be aware that events may arrive out of order
4. **Memory Leaks**: Ensure proper cleanup in event handlers

## Examples

See the integration tests in `src/plugin/tests/integration_tests.rs` for comprehensive examples of:
- End-to-end scanner-plugin coordination
- Error scenario handling
- Plugin lifecycle coordination
- Concurrent event processing
- Memory management and cleanup

## Future Enhancements

Planned improvements to the notification system:
- Event filtering and routing
- Priority-based event delivery
- Event persistence and replay
- Custom event types for plugins
- Performance metrics and monitoring
