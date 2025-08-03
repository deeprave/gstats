# gstats Development Guide

## Overview
gstats is a high-performance Git repository analytics tool built with Rust, focusing on local-first analysis with comprehensive plugin architecture and async processing capabilities.

## Development Philosophy

### Test-Driven Development (TDD)
All development follows strict TDD practices with the Red-Green-Refactor cycle:
1. **Red**: Write failing tests first to define expected behaviour
2. **Green**: Implement minimal code to make tests pass
3. **Refactor**: Improve code quality whilst maintaining test coverage

### Clean Architecture
- **SOLID Principles**: Single responsibility, open/closed, Liskov substitution, interface segregation, dependency inversion
- **Trait-Based Design**: Extensible interfaces with clear contracts
- **Memory Safety**: Leveraging Rust's ownership system for zero-cost abstractions
- **Async-First**: Built on tokio runtime with non-blocking operations

## Issue Management

### Issue Lifecycle
Issues follow a structured workflow through YouTrack (project GS):

1. **Backlog**: Issues available for selection and planning
2. **Open**: Issue selected but implementation not started
3. **In Progress**: Active TDD implementation following plan
4. **Queued**: Implementation complete, tests passing, ready for release
5. **Done**: Released with all queued issues

### Issue Types
- **Feature**: New functionality with complete TDD cycle
- **Enhancement**: Improvements to existing features
- **Bug**: Defect fixes with regression tests
- **Technical Debt**: Refactoring and code quality improvements

### Planning Process
Each issue requires:
- **Requirements Analysis**: Understanding scope and integration points
- **Implementation Plan**: Step-by-step TDD breakdown in `.claude/issues/<issue-id>/implementation-plan.md`
- **Acceptance Criteria**: Clear success metrics and test requirements
- **Dependencies**: Integration points with existing architecture

## Architecture Overview

### Core Components

#### Async Scanner Engine (GS-27)
High-performance async scanning system:
- **Task Management**: Concurrent task execution with resource constraints
- **Streaming Producer**: Memory-efficient data streaming
- **Repository Interface**: Async Git operations
- **Multi-mode Scanning**: Files, history, metrics, security, dependencies

#### Memory-Conscious Queue System (GS-26)
Advanced message queue with memory management:
- **Memory Tracking**: Real-time usage monitoring with leak detection
- **Backoff Algorithm**: Adaptive pressure response system
- **Versioned Messages**: Forward/backward compatibility
- **Listener System**: Event-driven notifications

#### Plugin Communication Interface (GS-28)
Extensible plugin architecture:
- **Trait Hierarchy**: Core Plugin, ScannerPlugin, NotificationPlugin
- **Async Notifications**: Real-time event broadcasting
- **Plugin Registry**: Lifecycle management and discovery
- **Version Compatibility**: API safety and dependency validation
- **Built-in Plugins**: Reference implementations (commits, metrics, export)

#### CLI System
Comprehensive command-line interface:
- **Argument Parsing**: Structured CLI with validation
- **Configuration Management**: TOML-based with discovery hierarchy
- **Plugin Management**: Discovery, validation, execution
- **Logging System**: Structured output with multiple destinations

### Data Flow Architecture

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   CLI Interface │────│  Configuration   │────│ Plugin Registry │
└─────────────────┘    └──────────────────┘    └─────────────────┘
         │                       │                       │
         ▼                       ▼                       ▼
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│ Scanner Engine  │────│  Message Queue   │────│ Plugin System   │
└─────────────────┘    └──────────────────┘    └─────────────────┘
         │                       │                       │
         ▼                       ▼                       ▼
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│ Git Repository  │    │  Notifications   │    │ Output Formats  │
└─────────────────┘    └──────────────────┘    └─────────────────┘
```

## Development Workflow

### Phase-Based Implementation
Major features are implemented in phases:

#### Phase Planning
1. **Requirements Gathering**: Analyse integration points and dependencies
2. **Architecture Design**: Define traits, structures, and interfaces
3. **Test Strategy**: Plan comprehensive test coverage
4. **Implementation Breakdown**: Create detailed task checklist

#### TDD Implementation
1. **Write Tests**: Start with comprehensive test cases
2. **Implement Features**: Minimal code to pass tests
3. **Refactor**: Improve code quality and performance
4. **Validate**: Ensure all tests pass and coverage requirements met

#### Quality Gates
- **100% Test Coverage**: All new code must have comprehensive tests
- **Zero Failing Tests**: No broken tests allowed in any commit
- **Memory Safety**: Rust compiler checks plus runtime monitoring
- **Performance**: Benchmarks for critical paths

### Git Workflow

#### Commit Standards
- **Descriptive Messages**: Clear purpose and scope
- **Issue References**: Link to YouTrack issues (GS-XX format)  
- **Generated Attribution**: Include Claude Code attribution for AI-assisted work
- **No Secrets**: Never commit API keys, passwords, or sensitive data

#### Branch Strategy
- **Main Branch**: Production-ready code with all tests passing
- **Feature Branches**: Individual issue implementation
- **Release Tags**: Semantic versioning for stable releases

## Testing Strategy

### Test Pyramid
1. **Unit Tests**: Individual components with mock dependencies
2. **Integration Tests**: Component interaction and data flow
3. **End-to-End Tests**: Complete workflows with real Git repositories
4. **Performance Tests**: Benchmarks and resource usage validation

### Mock Infrastructure
- **MockPlugin**: Comprehensive plugin testing without external dependencies
- **MockRepository**: Git operations testing without real repositories
- **MockQueues**: Message processing testing with controlled scenarios
- **MockNotifications**: Event system testing with configurable behaviour

### Test Categories
- **Happy Path**: Expected behaviour with valid inputs
- **Edge Cases**: Boundary conditions and unusual inputs
- **Error Handling**: Failure scenarios and recovery
- **Concurrency**: Multi-threaded operations and race conditions
- **Performance**: Resource usage and execution time

## Code Standards

### Rust Best Practices
- **Ownership Clarity**: Clear lifetimes and borrowing patterns
- **Error Handling**: Comprehensive Result types with context
- **Documentation**: Rustdoc for all public APIs
- **Clippy Compliance**: All linting warnings addressed
- **fmt Consistency**: Automated formatting with rustfmt

### Naming Conventions
- **Modules**: Snake_case reflecting functionality
- **Traits**: PascalCase describing capability
- **Structs**: PascalCase representing entities
- **Functions**: Snake_case describing action
- **Constants**: SCREAMING_SNAKE_CASE

### Memory Management
- **Minimal Allocations**: Efficient data structures and algorithms
- **Memory Monitoring**: Real-time tracking and leak detection
- **Resource Cleanup**: Proper Drop implementations
- **Async Safety**: Send + Sync bounds where required

## Plugin Development

### Plugin Types
1. **Scanner Plugins**: Process repository data (files, history, metrics)
2. **Notification Plugins**: Handle system events and progress updates
3. **Output Plugins**: Format and export analysis results

### Plugin Lifecycle
1. **Discovery**: Automatic detection and metadata parsing
2. **Registration**: Version compatibility and dependency validation
3. **Initialization**: Context setup and configuration
4. **Execution**: Async processing with error handling
5. **Cleanup**: Resource deallocation and state reset

### Built-in Examples
- **CommitsPlugin**: Git history analysis with issue tracking
- **MetricsPlugin**: Code quality assessment and complexity analysis
- **ExportPlugin**: Multi-format output (JSON, CSV, XML, YAML, HTML)

## Performance Characteristics

### Benchmarking
- **Scanner Performance**: Files/second processing rates
- **Memory Efficiency**: Peak usage and allocation patterns
- **Queue Throughput**: Messages/second with backpressure
- **Plugin Overhead**: Execution time and resource impact

### Optimization Targets
- **Large Repositories**: 100k+ files, 10k+ commits
- **Memory Constraints**: Configurable limits with graceful degradation
- **Concurrent Processing**: Multi-core utilization
- **Network Efficiency**: Minimal remote Git operations

## Contributing Guidelines

### Getting Started
1. **Environment Setup**: Rust toolchain, development dependencies
2. **Issue Selection**: Choose from Backlog in YouTrack
3. **Implementation Planning**: Create detailed phase breakdown
4. **TDD Cycle**: Red-Green-Refactor with comprehensive tests

### Code Review
- **Self-Review**: Check tests, documentation, performance
- **Peer Review**: Code quality, architecture alignment
- **Integration Testing**: Full system validation
- **Performance Validation**: Benchmark regression testing

### Release Process
1. **Feature Complete**: All planned functionality implemented
2. **Quality Gates**: 100% test pass rate, performance targets met
3. **Documentation**: API docs, user guides, examples
4. **Versioning**: Semantic version bump with changelog

## Troubleshooting

### Common Issues
- **Memory Pressure**: Monitor queue metrics and adjust limits
- **Plugin Failures**: Check version compatibility and dependencies
- **Performance Degradation**: Profile critical paths and optimize
- **Test Failures**: Ensure clean state and proper mocking

### Debugging Tools
- **Logging**: Structured output with configurable levels
- **Memory Tracking**: Real-time usage monitoring
- **Async Debugging**: Task coordination and deadlock detection
- **Plugin Diagnostics**: Execution tracing and error reporting

## Future Roadmap

### Planned Enhancements
- **Distributed Processing**: Multi-node repository analysis
- **Web Interface**: Browser-based repository visualization
- **AI Integration**: Intelligent code analysis and recommendations
- **Cloud Export**: Integration with external analytics platforms

### Architecture Evolution
- **Microservices**: Decomposition for scalability
- **Event Sourcing**: Complete audit trail and replay capability
- **Plugin Marketplace**: External plugin distribution and updates
- **API Gateway**: RESTful interface for external integrations

---

This development guide reflects the current state of the gstats codebase and serves as a reference for contributors and maintainers. The architecture and practices documented here have evolved through rigorous TDD implementation and performance optimization.