# gstats Architecture Documentation

## System Overview

gstats is a high-performance Git repository analytics tool built with Rust, designed for local-first analysis with extensible plugin architecture. The system processes Git repositories through an async pipeline, providing real-time analysis with memory-conscious operations.

## Core Architectural Principles

### Async-First Design
- **Non-blocking Operations**: All I/O operations use async/await with tokio runtime
- **Concurrent Processing**: Multi-task execution with configurable resource constraints  
- **Stream Processing**: Memory-efficient data processing with backpressure handling
- **Task Coordination**: Centralised task management with priority scheduling

### Memory-Conscious Operations
- **Real-time Monitoring**: Continuous memory usage tracking with leak detection
- **Adaptive Backoff**: Pressure-responsive queue management
- **Resource Constraints**: Configurable limits with graceful degradation
- **Zero-Copy Processing**: Efficient data structures minimising allocations

### Plugin Extensibility
- **Trait-Based Architecture**: Clean interfaces enabling external plugins
- **Version Compatibility**: API safety with semantic versioning
- **Async Notifications**: Real-time event broadcasting to plugins
- **Lifecycle Management**: Automatic registration, initialisation, and cleanup

## System Components

### 1. CLI System (`src/cli/`)

The command-line interface provides user interaction and system configuration.

#### Components:
- **Args Parser** (`args.rs`): Clap-based argument parsing with validation
- **Configuration Manager** (`converter.rs`): TOML configuration with CLI overrides
- **Plugin Handler** (`plugin_handler.rs`): Plugin management operations

#### Key Features:
- **Hierarchical Configuration**: Auto-discovery with precedence rules
- **Plugin Management**: List, validate, and execute plugins
- **Logging Configuration**: Structured output with multiple destinations
- **Memory Controls**: Performance mode presets and custom limits

#### Configuration hierarchy (highest to lowest precedence)
1. CLI arguments (--verbose, --config-file, etc.)
2. Environment variables ($GSTATS_CONFIG)
3. User config (~/.config/gstats/config.toml)
4. Project config (./.gstats.toml)
5. Default values

### 2. Scanner Engine (`src/scanner/`)

High-performance async scanning system for repository analysis using event-driven single-pass architecture with enhanced file tracking and conditional checkout capabilities.

#### Async Engine (`async_engine/`):
- **Engine** (`engine.rs`): Orchestrates scanning operations with plugin integration
- **EventDrivenScanner** (`scanners.rs`): Single-pass repository traversal with gitoxide and enhanced helper functions
- **FileTracker** (`file_tracker.rs`): Backwards history traversal with accurate line counting and lifecycle analysis
- **CheckoutManager** (`checkout_manager.rs`): Conditional file checkout based on plugin requirements
- **DiffAnalyzer** (`diff_analyzer.rs`): Smart diff parsing for precise change analysis
- **Processors** (`processors/`): Event-driven data processing components
- **Messages** (`../messages.rs`): Structured data flow with enhanced FileChange messages

#### Enhanced Features (Post-Refactoring):
- **Smart Diff Analysis**: Parses git diff output directly for accurate line count changes
- **Conditional File Checkout**: Only creates temporary files when plugins require file content
- **Backwards File Tracking**: Maintains accurate file states working backwards through git history
- **Binary File Support**: Handles both text files (line counts) and binary files (byte sizes)
- **File Lifecycle Analysis**: Detects deletion, resurrection, and rename patterns
- **Plugin Data Requirements**: PluginDataRequirements trait for declaring file access needs
- **Memory Optimization**: Avoids unnecessary file operations when plugins only need metadata

#### Plugin Integration:
- **PluginScanner** (`plugin_scanner.rs`): Wraps scanners with plugin processing capabilities
- **PluginScannerBuilder**: Creates plugin-enabled scanners with registry integration
- **PluginDataRequirements**: Trait for plugins to declare their data access requirements

#### Core Features:
- **Repository-Owning Pattern**: Each scanner creates its own repository access using spawn_blocking
- **Single-Pass Scanning**: EventDrivenScanner processes all repository data in one traversal
- **Helper Function Architecture**: Main scanning loop uses helper functions with max 2 levels of nesting
- **Event-Driven Processing**: Streaming data processing through EventProcessor trait
- **Gitoxide Integration**: Uses latest gitoxide (0.73) for Git operations without Send/Sync issues
- **Comprehensive Analysis**: Scanner extracts all repository data (files, history, metrics) without filtering
- **No Rough Estimates**: All data is calculated precisely from git diff analysis

#### Enhanced Architecture Flow:
```
Repository Path → EventDrivenScanner → 
├── determine_target_commit() → Branch Detection
├── get_commit_file_changes() → Smart Diff Analysis
│   ├── DiffAnalyzer → Precise Line Counting
│   ├── FileTracker → Backwards State Tracking
│   └── CheckoutManager → Conditional File Access
└── Message Builders → Enhanced FileChange Messages → PluginScanner → Plugins
```

### 3. Message Queue System (`src/queue/`)

Advanced message queue with memory management and event notification.

#### Core Components:
- **Memory Queue** (`memory_queue.rs`): Main queue implementation
- **Memory Tracker** (`memory_tracker.rs`): Real-time usage monitoring
- **Consumer** (`consumer.rs`): Message processing with batching
- **Backoff Algorithm** (`backoff.rs`): Adaptive pressure response

#### Advanced Features:
- **Versioned Messages** (`versioned_message.rs`): Forward/backward compatibility
- **Listener System** (`listener.rs`): Event-driven notifications
- **Memory Pressure Detection**: Automatic scaling and backoff
- **Performance Monitoring**: Throughput and latency metrics

```rust
// Message structure
pub struct ScanMessage {
    pub header: MessageHeader,  // Fixed-size metadata
    pub data: MessageData,      // Variable-size content
}

// Memory tracking
- Real-time usage monitoring
- Leak detection with history
- Pressure level calculation
- Automatic garbage collection triggers
```

### 4. Plugin System (`src/plugin/`)

Extensible plugin architecture with async communication and lifecycle management.

#### Core Traits:
- **Plugin** (`traits.rs`): Base plugin interface with lifecycle methods
- **ScannerPlugin**: Repository data processing capabilities
- **NotificationPlugin**: Event handling and system notifications

#### Management Components:
- **Registry** (`registry.rs`): Plugin lifecycle and discovery
- **Discovery** (`discovery.rs`): Automatic plugin detection and loading
- **Notification Manager** (`notification.rs`): Async event broadcasting
- **Compatibility Checker** (`compatibility.rs`): Version validation

#### Integration Components:
- **PluginExecutor** (`executor.rs`): Processes messages through registered plugins in real-time
- **PluginScanner** (`plugin_scanner.rs`): Wraps base scanners to add plugin processing capabilities
- **SharedPluginRegistry**: Thread-safe plugin registry wrapper with Arc<RwLock<>>

#### Built-in Plugins (`builtin/`):
- **CommitsPlugin** (`commits/`): Git history analysis with statistics
- **MetricsPlugin** (`metrics/`): Code quality and complexity metrics with comprehensive processors
- **ExportPlugin** (`export/`): Multi-format output (JSON, CSV, XML, YAML, HTML)

#### Comprehensive Processors (`processors/`):
- **ChangeFrequencyProcessor**: File change frequency analysis with time windows
- **ComplexityProcessor**: Language-specific complexity metrics (cyclomatic, cognitive, structural)
- **HotspotProcessor**: Risk assessment combining complexity and change frequency
- **DebtAssessmentProcessor**: Technical debt scoring with configurable factors
- **FormatDetectionProcessor**: File format classification with confidence scoring
- **DuplicationDetectorProcessor**: Code similarity analysis with impact assessment

```rust
// Plugin communication
pub enum PluginRequest {
    Execute { config: serde_json::Value },
    GetStatistics,
    GetCapabilities,
    Export,
    ProcessData { data: serde_json::Value },
}

pub enum PluginResponse {
    Success { data: serde_json::Value, metadata: ExecutionMetadata },
    Error { error: PluginError },
    Statistics(ScanMessage),
    Capabilities(Vec<PluginCapability>),
    Data(String),
}
```

## Data Flow Architecture

### 1. Initialisation Flow
```
CLI Args → Configuration → Repository Validation → Plugin Discovery → EventDrivenScanner Setup
```

### 2. Event-Driven Scanning Pipeline
```
Repository Path → EventDrivenScanner → spawn_blocking(gitoxide) → 
Event Stream → EventProcessor → ScanMessage → PluginScanner → 
Comprehensive Processors → Plugin Processing → Output
```

### 3. Plugin Integration Flow
```
CLI Args → Plugin Registry → Scanner Engine → EventDrivenScanner → 
PluginScanner → Event Processing → Comprehensive Processors → Results
```

### 4. Plugin Communication
```
Event Trigger → Notification Manager → Plugin Filtering → Async Delivery → Response
```

### 5. Memory Management
```
Usage Monitoring → Pressure Detection → Backoff Algorithm → Resource Adjustment
```

## Message Structures

### Core Message Types

#### ScanMessage
```rust
pub struct ScanMessage {
    pub header: MessageHeader,
    pub data: MessageData,
}

pub struct MessageHeader {
    pub sequence: u64,
    pub timestamp: u64,
}
```

#### MessageData Variants
```rust
pub enum MessageData {
    FileInfo { path: String, size: u64, lines: u32 },
    CommitInfo { hash: String, author: String, message: String, timestamp: i64 },
    FileChange {
        path: String,
        change_type: ChangeType,
        old_path: Option<String>,
        insertions: usize,
        deletions: usize,
        is_binary: bool,
        binary_size: Option<u64>,
        checkout_path: Option<PathBuf>,  // Enhanced for conditional checkout
    },
    MetricInfo { file_count: u32, line_count: u64, complexity: f64 },
    SecurityInfo { vulnerability: String, severity: String, location: String },
    DependencyInfo { name: String, version: String, license: Option<String> },
    PerformanceInfo { function: String, execution_time: f64, memory_usage: u64 },
    None,
}
```

### Queue Messages
- **Versioned Wrapper**: Forward/backward compatibility
- **Memory Estimation**: Accurate size calculation for tracking
- **Serialization**: Efficient binary format with bincode

## Enhanced File Tracking System

### Overview

The enhanced file tracking system represents a major architectural improvement that provides accurate file state tracking working backwards through git history, with conditional file checkout capabilities based on plugin requirements.

### Core Components

#### FileTracker (`file_tracker.rs`)
Maintains accurate file states as the scanner traverses git history backwards:

```rust
pub struct FileTracker {
    file_states: HashMap<String, FileState>,
}

pub struct FileState {
    pub line_count: Option<usize>,      // Precise line count from diff analysis
    pub is_binary: bool,                // Binary file detection
    pub binary_size: Option<u64>,       // Size for binary files
    pub exists: bool,                   // File existence at this point in history
    pub current_path: String,           // Track renames and moves
}
```

**Key Features:**
- **Backwards Processing**: Works backwards through git history for accurate state reconstruction
- **Precise Line Counting**: No estimates - all counts derived from actual diff analysis
- **Binary File Support**: Handles both text files (line counts) and binary files (byte sizes)
- **Rename Detection**: Tracks file path changes through git history
- **Lifecycle Analysis**: Detects file deletion, resurrection, and stability patterns

#### CheckoutManager (`checkout_manager.rs`)
Provides conditional file checkout capabilities based on plugin requirements:

```rust
pub struct CheckoutManager {
    base_checkout_dir: PathBuf,
    checkout_dirs: HashMap<String, PathBuf>,
    checkout_required: bool,             // Derived from plugin analysis
}
```

**Conditional Operations:**
- **Plugin-Driven Checkout**: Only creates files when plugins require file content
- **Commit-Scoped Directories**: Organizes checkouts by commit hash for isolation
- **Automatic Cleanup**: Implements Drop trait for guaranteed cleanup
- **Memory Efficiency**: Avoids unnecessary disk operations

#### DiffAnalyzer (`diff_analyzer.rs`)
Provides smart parsing of git diff output for accurate change analysis:

```rust
pub struct DiffLineAnalyzer;

impl DiffLineAnalyzer {
    pub fn analyze_diff_content(diff_content: &str) -> Result<FileChangeAnalysis, ScanError>
    pub fn calculate_line_changes(diff_hunks: &[DiffHunk]) -> (usize, usize)
}
```

**Smart Analysis:**
- **Direct Diff Parsing**: Processes git diff output directly for accuracy
- **Hunk-Level Analysis**: Analyzes individual diff hunks for precise line counting
- **Binary Detection**: Identifies binary files from diff output
- **Change Type Classification**: Accurately categorizes file changes (added, deleted, modified, renamed)

### Plugin Data Requirements Architecture

#### PluginDataRequirements Trait
Enables plugins to declare their data access requirements:

```rust
pub trait PluginDataRequirements {
    fn requires_current_file_content(&self) -> bool { false }
    fn requires_historical_file_content(&self) -> bool { false }
    fn preferred_buffer_size(&self) -> usize { 8192 }
    fn max_file_size(&self) -> Option<usize> { None }
    fn handles_binary_files(&self) -> bool { false }
}
```

**Plugin Categories:**
- **Metadata-Only Plugins**: Only need file paths, sizes, change counts (no checkout)
- **Content-Requiring Plugins**: Need actual file content (conditional checkout)
- **Binary-Aware Plugins**: Can process binary files alongside text files
- **Size-Limited Plugins**: Have specific file size constraints

#### Runtime Configuration Analysis
Dynamic configuration derived from active plugins:

```rust
pub struct RuntimeScannerConfig {
    pub requires_checkout: bool,          // Any plugins need file content
    pub requires_current_content: bool,   // Plugins need current file state
    pub requires_historical_content: bool, // Plugins need historical content
    pub base_config: ScannerConfig,
    pub effective_checkout_dir: Option<PathBuf>,
}

impl ScannerConfig {
    pub fn analyze_plugins(&self, plugins: &[Box<dyn PluginDataRequirements>]) -> RuntimeScannerConfig
}
```

### Enhanced Message Flow

#### FileChange Messages
Enhanced with conditional checkout support:

```rust
pub struct FileChange {
    pub path: String,
    pub change_type: ChangeType,
    pub old_path: Option<String>,         // For renames
    pub insertions: usize,                // Precise from diff analysis
    pub deletions: usize,                 // Precise from diff analysis
    pub is_binary: bool,
    pub binary_size: Option<u64>,
    pub checkout_path: Option<PathBuf>,   // Only populated when needed
}
```

#### Processing Flow
1. **Plugin Analysis**: Determine which plugins require file content
2. **Runtime Configuration**: Create optimized configuration based on requirements
3. **Conditional Checkout**: Setup CheckoutManager only if needed
4. **Smart Diff Processing**: Analyze git diffs for precise change data
5. **Backwards File Tracking**: Maintain accurate file states through history
6. **Message Enhancement**: Include checkout paths only when files are actually checked out

### Performance Optimizations

#### Memory Efficiency
- **Lazy Checkout**: Files only created when plugins require content
- **Selective Processing**: Skip unnecessary operations for metadata-only plugins
- **Efficient Tracking**: Only track files that appear in git diffs
- **Automatic Cleanup**: Immediate cleanup of temporary checkout directories

#### Processing Efficiency
- **Single-Pass Analysis**: All diff information extracted in one pass
- **Helper Function Architecture**: Main scanning loop complexity reduced to max 2 levels
- **Backwards Traversal**: Efficient file state reconstruction
- **Binary Detection**: Quick identification to avoid unnecessary text processing

### Integration with Existing Architecture

The enhanced file tracking system integrates seamlessly with the existing plugin architecture:

- **Transparent Operation**: Existing plugins continue to work without modification
- **Progressive Enhancement**: Plugins can opt-in to enhanced features via PluginDataRequirements
- **Backward Compatibility**: Legacy plugins receive metadata as before
- **Performance Benefits**: Metadata-only plugins get better performance due to reduced I/O

## Plugin Architecture

### Plugin Trait Hierarchy

```rust
// Base plugin interface
#[async_trait]
pub trait Plugin: Send + Sync {
    fn plugin_info(&self) -> &PluginInfo;
    async fn initialize(&mut self, context: &PluginContext) -> PluginResult<()>;
    async fn execute(&self, request: PluginRequest) -> PluginResult<PluginResponse>;
    async fn cleanup(&mut self) -> PluginResult<()>;
}

// Data requirements interface (NEW)
pub trait PluginDataRequirements {
    fn requires_current_file_content(&self) -> bool { false }
    fn requires_historical_file_content(&self) -> bool { false }
    fn preferred_buffer_size(&self) -> usize { 8192 }
    fn max_file_size(&self) -> Option<usize> { None }
    fn handles_binary_files(&self) -> bool { false }
}

// Scanner-specific capabilities
#[async_trait]
pub trait ScannerPlugin: Plugin {
    async fn process_scan_data(&self, data: &ScanMessage) -> PluginResult<Vec<ScanMessage>>;
    async fn aggregate_results(&self, results: Vec<ScanMessage>) -> PluginResult<ScanMessage>;
    fn estimate_processing_time(&self, item_count: usize) -> Option<Duration>;
    fn config_schema(&self) -> serde_json::Value;
}

// Notification capabilities
#[async_trait]
pub trait NotificationPlugin: Plugin {
    async fn on_queue_update(&self, update: QueueUpdate) -> PluginResult<()>;
    async fn on_scan_progress(&self, progress: ScanProgress) -> PluginResult<()>;
    async fn on_error(&self, error: PluginError) -> PluginResult<()>;
    async fn on_system_event(&self, event: SystemEvent) -> PluginResult<()>;
    fn notification_preferences(&self) -> NotificationPreferences;
}
```

### Plugin Lifecycle

1. **Discovery**: Automatic detection in plugin directories
2. **Registration**: Version compatibility validation
3. **Initialisation**: Context setup and configuration
4. **Execution**: Async processing with error handling
5. **Notification**: Event delivery and response
6. **Cleanup**: Resource deallocation and state reset

### Plugin Communication

#### Request/Response Pattern
- **Enum-based Messages**: Flexible communication protocol
- **Async Execution**: Non-blocking plugin operations
- **Error Isolation**: Plugin failures don't crash system
- **Metadata Tracking**: Execution time and resource usage

#### Notification System
- **Event Broadcasting**: Real-time system events
- **Filtered Delivery**: Plugin preferences and capabilities
- **Rate Limiting**: Frequency controls and backpressure
- **Graceful Shutdown**: Clean termination and cleanup

### Plugin-Scanner Integration

The plugin system is fully integrated with the async scanner engine through several key components:

#### PluginExecutor
```rust
pub struct PluginExecutor {
    registry: Arc<RwLock<PluginRegistry>>,
    metrics: Arc<RwLock<ExecutionMetrics>>,
}
```

**Responsibilities:**
- Processes scan messages through registered plugins in real-time
- Manages plugin execution metrics and performance tracking
- Provides streaming plugin processing with backpressure handling
- Handles plugin errors gracefully without system crashes

#### PluginScanner Adapter
```rust
pub struct PluginScanner {
    inner_scanner: Arc<dyn AsyncScanner>,
    plugin_registry: SharedPluginRegistry,
    name: String,
}
```

**Integration Features:**
- Wraps existing async scanners to add plugin processing capabilities
- Maintains scanner interface compatibility for seamless integration
- Provides transparent plugin execution during scanning operations
- Supports plugin-generated messages and data transformation

#### Plugin Stream Processing
- **Streaming Architecture**: Plugin processing integrated into scanner streams
- **Async Boundaries**: Proper async/sync coordination for plugin execution
- **Backpressure Handling**: Plugin processing respects scanner flow control
- **Message Buffering**: Efficient handling of plugin-generated messages

#### Integration Flow
1. **Scanner Creation**: EventDrivenScanner created with repository-owning pattern
2. **Plugin Wrapping**: PluginScannerBuilder wraps scanner with plugin processing
3. **Engine Integration**: Plugin-wrapped scanner added to AsyncScannerEngine
4. **Stream Processing**: Plugin execution happens during scanning via PluginProcessingStream
5. **Event Processing**: Comprehensive processors handle specialised analysis
6. **Message Flow**: Plugin-processed messages flow through the queue system

## Memory Management

### Tracking System
```rust
pub struct MemoryTracker {
    current_usage: AtomicUsize,
    peak_usage: AtomicUsize,
    memory_limit: usize,
    samples: Vec<MemoryUsageSample>,
    leak_detector: LeakDetector,
}
```

### Pressure Levels
- **Normal**: < 70% of limit, standard operations
- **Moderate**: 70-85% of limit, reduce batch sizes
- **High**: 85-95% of limit, aggressive cleanup
- **Critical**: > 95% of limit, emergency measures

### Adaptive Algorithms
- **Exponential Backoff**: Pressure-responsive delays
- **Batch Size Adjustment**: Dynamic processing windows
- **Garbage Collection**: Triggered cleanup operations
- **Resource Scaling**: Thread pool and queue adjustments

## Performance Characteristics

### Benchmarking Results
- **File Processing**: 10k+ files/second on modern hardware
- **Memory Efficiency**: < 100MB for typical repositories
- **Queue Throughput**: 50k+ messages/second with backpressure
- **Plugin Overhead**: < 5% execution time impact

### Optimisation Strategies
- **Zero-Copy Operations**: Minimise memory allocations
- **Batch Processing**: Efficient I/O operations
- **Async Coordination**: Non-blocking task execution
- **Resource Pooling**: Reuse expensive resources

### Scalability Targets
- **Large Repositories**: 100k+ files, 100k+ commits
- **Memory Constraints**: Configurable 32MB-2GB limits
- **Concurrent Processing**: Multi-core utilisation
- **Plugin Scaling**: 10+ active plugins simultaneously

## Error Handling

### Error Categories
```rust
pub enum PluginError {
    InitializationFailed { message: String },
    ExecutionFailed { message: String },
    InvalidState { message: String },
    NotificationFailed { message: String },
    Generic { message: String },
}
```

### Recovery Strategies
- **Plugin Isolation**: Failures don't affect other plugins
- **Graceful Degradation**: Continue processing without failed components
- **Retry Logic**: Configurable retry attempts with backoff
- **Circuit Breaker**: Disable failing plugins temporarily

### Monitoring
- **Error Metrics**: Failure rates and patterns
- **Performance Tracking**: Execution time and resource usage
- **Health Checks**: Component status monitoring
- **Alerting**: Critical failure notifications

## Configuration System

### Hierarchy
```toml
# Global settings
[base]
verbose = true
log-format = "json"

# Scanner configuration
[scanner]
max-memory = "256MB"
queue-size = 5000
max-threads = 8

# Plugin settings
[plugins]
enabled = ["commits", "metrics", "export"]
plugin-dir = "/usr/local/lib/gstats/plugins"

# Module-specific configuration
[module.commits]
since = "30d"
include-merges = false

[module.metrics]
max-files = 10000
complexity-analysis = true

[module.export]
format = "json"
output-path = "./output"
```

### Discovery Process
1. CLI arguments (highest precedence)
2. Environment variables
3. User configuration files
4. Project-local configuration
5. System defaults (lowest precedence)

## Testing Architecture

### Test Categories
- **Unit Tests**: Individual component testing with mocks
- **Integration Tests**: Component interaction validation
- **Performance Tests**: Benchmarking and resource usage
- **End-to-End Tests**: Complete workflow validation

### Mock Infrastructure
- **MockPlugin**: Comprehensive plugin testing framework
- **MockRepository**: Git operations without real repositories
- **MockQueue**: Message processing with controlled scenarios
- **MockNotifications**: Event system testing

### Quality Gates
- **100% Test Coverage**: All new code must have tests
- **Zero Failing Tests**: No broken tests in any commit
- **Performance Benchmarks**: Regression detection
- **Memory Safety**: Leak detection and validation

## Deployment Architecture

### Binary Distribution
- **Single Executable**: No external dependencies
- **Cross-Platform**: Linux, macOS, Windows support
- **Plugin Discovery**: Automatic detection in standard locations
- **Configuration**: Hierarchical with sensible defaults

### Plugin Ecosystem
- **Built-in Plugins**: Core functionality included
- **External Plugins**: Dynamic loading from directories
- **Version Compatibility**: API safety with semantic versioning
- **Plugin Marketplace**: Future distribution platform

### Monitoring
- **Metrics Collection**: Performance and usage statistics
- **Log Aggregation**: Structured logging with correlation
- **Health Monitoring**: Component status and alerts
- **Resource Tracking**: Memory and CPU utilisation

## Future Architecture Evolution

### Planned Enhancements
- **Distributed Processing**: Multi-node repository analysis
- **Web Interface**: Browser-based visualization and control
- **API Gateway**: RESTful interface for external integrations
- **Cloud Export**: Integration with external analytics platforms

### Scalability Improvements
- **Microservices**: Component decomposition for scaling
- **Event Sourcing**: Complete audit trail and replay capability
- **Caching Layer**: Results persistence and invalidation
- **Load Balancing**: Request distribution and failover

### Plugin Evolution
- **Hot Reloading**: Runtime plugin updates without restart
- **Sandboxing**: Security isolation for external plugins
- **Resource Quotas**: Per-plugin resource limitations
- **Dependency Management**: Automated plugin ecosystem

---

This architecture documentation reflects the current implementation of gstats and provides a foundation for understanding the system's design principles, component interactions, and future evolution plans.