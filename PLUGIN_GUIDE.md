# gstats Plugin Development Guide

## Overview

This guide provides comprehensive documentation for developing plugins for gstats, a high-performance Git repository analytics tool. The plugin system is built on async Rust traits, providing extensible functionality with strong type safety and performance guarantees.

## Plugin System Architecture

### Core Concepts

gstats plugins are async Rust components that extend the core functionality through a trait-based architecture. The system supports three main plugin types:

1. **Scanner Plugins**: Process repository data (files, commits, metrics)
2. **Notification Plugins**: Handle system events and progress updates  
3. **Output Plugins**: Format and export analysis results

### Plugin Lifecycle

Every plugin follows a structured lifecycle:

1. **Discovery**: Automatic detection and metadata parsing
2. **Registration**: Version compatibility and dependency validation
3. **Initialization**: Context setup and configuration
4. **Execution**: Async processing with error handling
5. **Cleanup**: Resource deallocation and state reset

## Plugin Traits

### Base Plugin Trait

All plugins must implement the core `Plugin` trait:

```rust
use async_trait::async_trait;
use crate::plugin::{Plugin, PluginInfo, PluginContext, PluginRequest, PluginResponse, PluginResult};

#[async_trait]
pub trait Plugin: Send + Sync {
    /// Returns plugin metadata and capabilities
    fn plugin_info(&self) -> &PluginInfo;
    
    /// Initialize plugin with system context
    async fn initialize(&mut self, context: &PluginContext) -> PluginResult<()>;
    
    /// Execute plugin request asynchronously
    async fn execute(&self, request: PluginRequest) -> PluginResult<PluginResponse>;
    
    /// Clean up plugin resources
    async fn cleanup(&mut self) -> PluginResult<()>;
}
```

### Scanner Plugin Trait

For plugins that process repository data:

```rust
#[async_trait]
pub trait ScannerPlugin: Plugin {
    /// Declare which scan modes this plugin supports
    fn supported_modes(&self) -> ScanMode;
    
    /// Process individual scan messages
    async fn process_scan_data(&self, data: &ScanMessage) -> PluginResult<Vec<ScanMessage>>;
    
    /// Aggregate multiple scan results
    async fn aggregate_results(&self, results: Vec<ScanMessage>) -> PluginResult<ScanMessage>;
    
    /// Estimate processing time for performance planning
    fn estimate_processing_time(&self, modes: ScanMode, item_count: usize) -> Option<Duration>;
    
    /// Provide JSON schema for plugin configuration
    fn config_schema(&self) -> serde_json::Value;
}
```

### Notification Plugin Trait

For plugins that handle system events:

```rust
#[async_trait]
pub trait NotificationPlugin: Plugin {
    /// Handle queue update notifications
    async fn on_queue_update(&self, update: QueueUpdate) -> PluginResult<()>;
    
    /// Handle scan progress notifications
    async fn on_scan_progress(&self, progress: ScanProgress) -> PluginResult<()>;
    
    /// Handle error notifications
    async fn on_error(&self, error: PluginError) -> PluginResult<()>;
    
    /// Handle system event notifications
    async fn on_system_event(&self, event: SystemEvent) -> PluginResult<()>;
    
    /// Declare notification preferences
    fn notification_preferences(&self) -> NotificationPreferences;
}
```

## Building Your First Plugin

### 1. Basic Plugin Structure

Create a new plugin by implementing the base `Plugin` trait:

```rust
use async_trait::async_trait;
use crate::plugin::*;
use crate::scanner::modes::ScanMode;
use std::collections::HashMap;

pub struct MyPlugin {
    info: PluginInfo,
    initialized: bool,
    data: HashMap<String, String>,
}

impl MyPlugin {
    pub fn new() -> Self {
        let info = PluginInfo::new(
            "my-plugin".to_string(),
            "1.0.0".to_string(),
            20250727, // API version
            "My custom plugin for gstats".to_string(),
            "Your Name".to_string(),
            PluginType::Processing,
        );
        
        Self {
            info,
            initialized: false,
            data: HashMap::new(),
        }
    }
}

#[async_trait]
impl Plugin for MyPlugin {
    fn plugin_info(&self) -> &PluginInfo {
        &self.info
    }
    
    async fn initialize(&mut self, context: &PluginContext) -> PluginResult<()> {
        if self.initialized {
            return Err(PluginError::initialization_failed("Already initialized"));
        }
        
        // Initialize your plugin state here
        self.data.clear();
        self.initialized = true;
        
        Ok(())
    }
    
    async fn execute(&self, request: PluginRequest) -> PluginResult<PluginResponse> {
        match request {
            PluginRequest::GetCapabilities => {
                Ok(PluginResponse::Capabilities(self.info.capabilities.clone()))
            },
            PluginRequest::GetStatistics => {
                // Return plugin statistics
                let data = MessageData::MetricInfo {
                    file_count: self.data.len() as u32,
                    line_count: 0,
                    complexity: 0.0,
                };
                
                let header = MessageHeader::new(ScanMode::FILES, 
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs()
                );
                
                Ok(PluginResponse::Statistics(ScanMessage::new(header, data)))
            },
            _ => Err(PluginError::execution_failed("Unsupported request type")),
        }
    }
    
    async fn cleanup(&mut self) -> PluginResult<()> {
        self.initialized = false;
        self.data.clear();
        Ok(())
    }
}
```

### 2. Scanner Plugin Example

Here's a complete example of a scanner plugin that analyzes file extensions:

```rust
use async_trait::async_trait;
use crate::plugin::*;
use crate::scanner::{modes::ScanMode, messages::*};
use std::collections::HashMap;

pub struct FileExtensionPlugin {
    info: PluginInfo,
    initialized: bool,
    extension_counts: HashMap<String, u32>,
}

impl FileExtensionPlugin {
    pub fn new() -> Self {
        let info = PluginInfo::new(
            "file-extensions".to_string(),
            "1.0.0".to_string(),
            20250727,
            "Analyzes file extensions in repository".to_string(),
            "gstats team".to_string(),
            PluginType::Scanner,
        )
        .with_capability(
            "extension_analysis".to_string(),
            "Counts file extensions and sizes".to_string(),
            "1.0.0".to_string(),
        );
        
        Self {
            info,
            initialized: false,
            extension_counts: HashMap::new(),
        }
    }
}

#[async_trait]
impl Plugin for FileExtensionPlugin {
    fn plugin_info(&self) -> &PluginInfo {
        &self.info
    }
    
    async fn initialize(&mut self, _context: &PluginContext) -> PluginResult<()> {
        self.extension_counts.clear();
        self.initialized = true;
        Ok(())
    }
    
    async fn execute(&self, request: PluginRequest) -> PluginResult<PluginResponse> {
        match request {
            PluginRequest::GetStatistics => {
                let total_files = self.extension_counts.values().sum::<u32>();
                let data = MessageData::MetricInfo {
                    file_count: total_files,
                    line_count: 0,
                    complexity: self.extension_counts.len() as f64,
                };
                
                let header = MessageHeader::new(ScanMode::FILES, 
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs()
                );
                
                Ok(PluginResponse::Statistics(ScanMessage::new(header, data)))
            },
            _ => Err(PluginError::execution_failed("Unsupported request")),
        }
    }
    
    async fn cleanup(&mut self) -> PluginResult<()> {
        self.initialized = false;
        self.extension_counts.clear();
        Ok(())
    }
}

#[async_trait]
impl ScannerPlugin for FileExtensionPlugin {
    fn supported_modes(&self) -> ScanMode {
        ScanMode::FILES
    }
    
    async fn process_scan_data(&self, data: &ScanMessage) -> PluginResult<Vec<ScanMessage>> {
        if let MessageData::FileInfo { path, .. } = &data.data {
            // Extract file extension
            let extension = std::path::Path::new(path)
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("no_extension")
                .to_string();
            
            // Create a modified copy (plugins should be immutable)
            let mut plugin_copy = self.clone_for_processing();
            *plugin_copy.extension_counts.entry(extension).or_insert(0) += 1;
        }
        
        // Return original message plus any additional analysis
        Ok(vec![data.clone()])
    }
    
    async fn aggregate_results(&self, results: Vec<ScanMessage>) -> PluginResult<ScanMessage> {
        let total_files = results.len() as u32;
        
        let data = MessageData::MetricInfo {
            file_count: total_files,
            line_count: 0,
            complexity: self.extension_counts.len() as f64,
        };
        
        let header = MessageHeader::new(ScanMode::FILES, 
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        );
        
        Ok(ScanMessage::new(header, data))
    }
    
    fn estimate_processing_time(&self, modes: ScanMode, item_count: usize) -> Option<Duration> {
        if modes.contains(ScanMode::FILES) {
            // Estimate 1ms per file
            Some(Duration::from_millis(item_count as u64))
        } else {
            None
        }
    }
    
    fn config_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "include_hidden": {
                    "type": "boolean",
                    "description": "Include hidden files in analysis",
                    "default": false
                },
                "max_files": {
                    "type": "integer",
                    "description": "Maximum files to process",
                    "default": 10000
                }
            }
        })
    }
}

// Helper method for immutable processing
impl FileExtensionPlugin {
    fn clone_for_processing(&self) -> Self {
        Self {
            info: self.info.clone(),
            initialized: self.initialized,
            extension_counts: self.extension_counts.clone(),
        }
    }
}
```

## Message Types and Data Structures

### ScanMessage Structure

All data flows through the system as `ScanMessage` instances:

```rust
pub struct ScanMessage {
    pub header: MessageHeader,
    pub data: MessageData,
}

pub struct MessageHeader {
    pub scan_mode: ScanMode,
    pub timestamp: u64,
}
```

### MessageData Variants

The system supports various data types:

```rust
pub enum MessageData {
    FileInfo {
        path: String,
        size: u64, 
        lines: u32,
    },
    CommitInfo {
        hash: String,
        author: String,
        message: String,
        timestamp: i64,
    },
    MetricInfo {
        file_count: u32,
        line_count: u64,
        complexity: f64,
    },
    SecurityInfo {
        vulnerability: String,
        severity: String,
        location: String,
    },
    DependencyInfo {
        name: String,
        version: String,
        license: Option<String>,
    },
    PerformanceInfo {
        function: String,
        execution_time: f64,
        memory_usage: u64,
    },
    None,
}
```

### Plugin Communication

Plugins communicate using structured request/response enums:

```rust
pub enum PluginRequest {
    Execute {
        request_id: String,
        scan_modes: ScanMode,
        parameters: HashMap<String, serde_json::Value>,
        priority: RequestPriority,
        timeout_ms: Option<u64>,
    },
    GetStatistics,
    GetCapabilities,
    Export,
    ProcessData {
        data: serde_json::Value,
    },
}

pub enum PluginResponse {
    Success {
        request_id: String,
        data: serde_json::Value,
        metadata: ExecutionMetadata,
    },
    Error {
        request_id: String,
        error: PluginError,
    },
    Statistics(ScanMessage),
    Capabilities(Vec<PluginCapability>),
    Data(String),
}
```

## Configuration and Context

### Plugin Context

The `PluginContext` provides access to system resources:

```rust
pub struct PluginContext {
    pub scanner_config: Arc<ScannerConfig>,
    pub repository: Arc<RepositoryHandle>,
    pub query_params: Arc<QueryParams>,
    pub plugin_config: HashMap<String, serde_json::Value>,
    pub runtime_info: RuntimeInfo,
}
```

### Configuration Schema

Define configuration schemas for type-safe settings:

```rust
fn config_schema(&self) -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "enabled": {
                "type": "boolean",
                "description": "Enable this plugin",
                "default": true
            },
            "batch_size": {
                "type": "integer",
                "description": "Processing batch size",
                "default": 1000,
                "minimum": 1
            },
            "output_format": {
                "type": "string",
                "enum": ["json", "csv", "xml"],
                "description": "Output format",
                "default": "json"
            }
        }
    })
}
```

## Error Handling

### Plugin Error Types

Use structured error types for better error handling:

```rust
pub enum PluginError {
    InitializationFailed { message: String },
    ExecutionFailed { message: String },
    InvalidState { message: String },
    NotificationFailed { message: String },
    Generic { message: String },
}

// Convenience constructors
impl PluginError {
    pub fn initialization_failed(message: &str) -> Self {
        Self::InitializationFailed { message: message.to_string() }
    }
    
    pub fn execution_failed(message: &str) -> Self {
        Self::ExecutionFailed { message: message.to_string() }
    }
    
    pub fn invalid_state(message: &str) -> Self {
        Self::InvalidState { message: message.to_string() }
    }
}
```

### Error Handling Best Practices

1. **Graceful Degradation**: Handle errors without crashing
2. **Detailed Messages**: Provide helpful error context
3. **Resource Cleanup**: Ensure cleanup on error paths
4. **Error Isolation**: Don't let plugin errors affect the system

```rust
async fn safe_operation(&self) -> PluginResult<String> {
    // Use ? operator for error propagation
    let data = self.load_data()
        .await
        .map_err(|e| PluginError::execution_failed(&format!("Failed to load data: {}", e)))?;
    
    // Validate input
    if data.is_empty() {
        return Err(PluginError::invalid_state("No data available"));
    }
    
    // Process data safely
    Ok(process_data(data))
}
```

## Testing Your Plugin

### Unit Testing

Create comprehensive tests for your plugin:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::context::PluginContext;
    
    fn create_test_context() -> PluginContext {
        let repo = crate::git::resolve_repository_handle(None).unwrap();
        let scanner_config = std::sync::Arc::new(crate::scanner::ScannerConfig::default());
        let query_params = std::sync::Arc::new(crate::scanner::QueryParams::default());
        
        PluginContext::new(scanner_config, Arc::new(repo), query_params)
    }
    
    #[tokio::test]
    async fn test_plugin_initialization() {
        let mut plugin = MyPlugin::new();
        let context = create_test_context();
        
        assert!(plugin.initialize(&context).await.is_ok());
        assert!(plugin.initialized);
    }
    
    #[tokio::test] 
    async fn test_plugin_execution() {
        let mut plugin = MyPlugin::new();
        let context = create_test_context();
        plugin.initialize(&context).await.unwrap();
        
        let request = PluginRequest::GetCapabilities;
        let response = plugin.execute(request).await.unwrap();
        
        match response {
            PluginResponse::Capabilities(caps) => {
                assert!(!caps.is_empty());
            },
            _ => panic!("Expected capabilities response"),
        }
    }
    
    #[tokio::test]
    async fn test_error_handling() {
        let plugin = MyPlugin::new();
        let request = PluginRequest::GetStatistics; // Plugin not initialized
        
        let result = plugin.execute(request).await;
        assert!(result.is_err());
    }
}
```

### Integration Testing

Test your plugin with the full system:

```rust
#[tokio::test]
async fn test_plugin_integration() {
    let mut registry = PluginRegistry::new();
    let mut plugin = MyPlugin::new();
    
    // Register plugin
    registry.register_plugin("my-plugin", Box::new(plugin)).await.unwrap();
    
    // Test discovery
    let plugins = registry.list_plugins().await;
    assert!(plugins.contains(&"my-plugin".to_string()));
    
    // Test execution
    let result = registry.execute_plugin("my-plugin", PluginRequest::GetCapabilities).await;
    assert!(result.is_ok());
}
```

## Performance Optimization

### Memory Management

Keep memory usage minimal and predictable:

```rust
impl MyPlugin {
    // Use bounded collections
    fn new() -> Self {
        Self {
            cache: HashMap::with_capacity(1000), // Pre-allocate
            buffer: Vec::with_capacity(512),     // Avoid reallocations
        }
    }
    
    // Implement cleanup
    async fn cleanup(&mut self) -> PluginResult<()> {
        self.cache.clear();
        self.cache.shrink_to_fit(); // Release memory
        self.buffer.clear();
        self.buffer.shrink_to_fit();
        Ok(())
    }
}
```

### Async Best Practices

Write efficient async code:

```rust
// Use streams for large data sets
async fn process_large_dataset(&self, data: Vec<ScanMessage>) -> PluginResult<Vec<ScanMessage>> {
    use futures::stream::{self, StreamExt};
    
    let results: Vec<ScanMessage> = stream::iter(data)
        .map(|message| async move { self.process_message(message).await })
        .buffer_unordered(10) // Process 10 items concurrently
        .collect()
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    
    Ok(results)
}

// Avoid blocking operations
async fn non_blocking_operation(&self) -> PluginResult<String> {
    // Use tokio for I/O
    let content = tokio::fs::read_to_string("data.txt")
        .await
        .map_err(|e| PluginError::execution_failed(&e.to_string()))?;
    
    // Use spawn_blocking for CPU-intensive work
    let processed = tokio::task::spawn_blocking(move || {
        expensive_computation(content)
    })
    .await
    .map_err(|e| PluginError::execution_failed(&e.to_string()))?;
    
    Ok(processed)
}
```

## Plugin Distribution

### Plugin Metadata

Create a plugin descriptor file:

```yaml
# my-plugin.yaml
name: my-plugin
version: 1.0.0
api_version: 20250727
description: "My custom gstats plugin"
author: "Your Name"
plugin_type: Scanner

capabilities:
  - name: file_analysis
    description: "Analyzes file patterns"
    version: 1.0.0

dependencies:
  - name: core
    version: ">=1.0.0"
    optional: false

configuration_schema:
  type: object
  properties:
    batch_size:
      type: integer
      default: 1000
```

### Installation

1. **Built-in Plugins**: Include in `src/plugin/builtin/`
2. **External Plugins**: Place in plugin directories
3. **Dynamic Loading**: Support for shared libraries (future)

### Plugin Discovery

The system automatically discovers plugins in:

- `/usr/local/lib/gstats/plugins/`
- `~/.local/lib/gstats/plugins/`
- `./plugins/`
- Environment variable `GSTATS_PLUGIN_PATH`

## Advanced Features

### Notification Plugins

Handle system events:

```rust
#[async_trait]
impl NotificationPlugin for MyNotificationPlugin {
    async fn on_queue_update(&self, update: QueueUpdate) -> PluginResult<()> {
        println!("Queue {} has {} items", update.queue_id, update.item_count);
        Ok(())
    }
    
    async fn on_scan_progress(&self, progress: ScanProgress) -> PluginResult<()> {
        println!("Scan {} processed {} items", progress.scan_id, progress.items_processed);
        Ok(())
    }
    
    fn notification_preferences(&self) -> NotificationPreferences {
        NotificationPreferences {
            queue_updates: true,
            scan_progress: true,
            error_notifications: true,
            system_events: vec![
                SystemEventType::SystemStartup,
                SystemEventType::ConfigurationChanged,
            ],
            max_frequency: Some(10), // Max 10 notifications per second
        }
    }
}
```

### Plugin Dependencies

Declare dependencies for complex plugins:

```rust
impl PluginInfo {
    fn with_dependency(mut self, name: String, version: String, optional: bool) -> Self {
        self.dependencies.push(PluginDependency { name, version, optional });
        self
    }
}

let info = PluginInfo::new(/* ... */)
    .with_dependency("core".to_string(), ">=1.0.0".to_string(), false)
    .with_dependency("metrics".to_string(), "^2.0".to_string(), true);
```

## Built-in Plugin Examples

Study the built-in plugins for comprehensive examples:

### 1. CommitsPlugin (`src/plugin/builtin/commits.rs`)
- Git history analysis
- Commit statistics and author tracking
- Issue reference extraction
- Demonstrates scanner plugin patterns

### 2. MetricsPlugin (`src/plugin/builtin/metrics.rs`)
- Code complexity analysis
- File statistics and quality metrics
- Multi-language support
- Shows aggregation patterns

### 3. ExportPlugin (`src/plugin/builtin/export.rs`)
- Multi-format output (JSON, CSV, XML, YAML, HTML)
- Data transformation and escaping
- Template-based rendering
- Demonstrates output plugin architecture

## Troubleshooting

### Common Issues

1. **Plugin Not Discovered**
   - Check plugin descriptor syntax
   - Verify file permissions
   - Ensure plugin directory is in search path

2. **Initialization Failures**
   - Validate plugin dependencies
   - Check API version compatibility
   - Review context requirements

3. **Performance Issues**
   - Profile memory usage
   - Optimize async operations
   - Implement efficient data structures

4. **Error Handling**
   - Use structured error types
   - Provide detailed error messages
   - Implement proper cleanup

### Debugging Tools

Enable debug logging:

```bash
RUST_LOG=debug gstats --verbose .
```

Use the plugin diagnostic tools:

```bash
gstats --list-plugins
gstats --plugin-info my-plugin
gstats --check-plugin my-plugin
```

### Best Practices Summary

1. **Follow the Trait Contracts**: Implement all required methods properly
2. **Handle Errors Gracefully**: Don't crash the system on plugin failures
3. **Write Comprehensive Tests**: Unit, integration, and performance tests
4. **Document Your Plugin**: Clear descriptions and configuration schemas
5. **Optimize for Performance**: Memory-conscious and async-first design
6. **Version Compatibility**: Use semantic versioning and API contracts
7. **Security**: Validate inputs and handle sensitive data properly

## Resources

- **Built-in Plugin Examples**: `src/plugin/builtin/`
- **Test Framework**: `src/plugin/tests/mock_plugins.rs`
- **API Documentation**: Generated Rustdoc
- **Architecture Guide**: `ARCHITECTURE.md`
- **Development Guide**: `DEVELOPMENT.md`

---

This plugin guide provides the foundation for building powerful extensions to gstats. The trait-based architecture ensures type safety and performance while enabling rich functionality through async processing and event handling.