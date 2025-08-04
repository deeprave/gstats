# Performance Benchmarks

This directory contains comprehensive performance benchmarks for the gstats scanner system, measuring throughput, latency, memory usage, and scalability characteristics.

## Benchmark Categories

### 1. Scanner Benchmarks (`scanner_benchmarks.rs`)
- **Scanner configuration creation and validation**
- **Repository handle operations**
- **Scanner engine creation and setup**
- **Performance with different repository sizes**
- **Scan mode comparison (FILES, HISTORY, ALL)**

**Target Metrics:**
- Configuration creation: < 1ms
- Repository operations: < 10ms
- Scan throughput: > 1000 commits/second

### 2. Memory Benchmarks (`memory_benchmarks.rs`)
- **Memory queue operations (enqueue/dequeue)**
- **Message size impact on performance**
- **Batch processing efficiency**
- **Memory tracking and pressure detection**
- **Backoff algorithm performance**
- **Concurrent memory operations**
- **Memory allocation patterns**

**Target Metrics:**
- Memory usage: < 256MB for typical repositories
- Queue overhead: < 1MB total
- Memory pressure detection: < 1ms

### 3. Queue Benchmarks (`queue_benchmarks.rs`)
- **Single producer throughput (target: >10,000 messages/sec)**
- **Multi-producer concurrent throughput**
- **Consumer thread latency (target: <1ms average)**
- **ScanMode filtering performance (target: <100μs lookup)**
- **Batch processing latency**
- **Message size impact on throughput**
- **Backoff algorithm effectiveness**

**Target Metrics:**
- Queue throughput: > 10,000 messages/second
- Consumer latency: < 1ms average
- ScanMode filtering: < 100μs lookup time
- Multi-producer scaling: Near-linear to 16 threads

### 4. Integration Benchmarks (`integration_benchmarks.rs`)
- **Complete CLI argument parsing**
- **Configuration loading (file vs default)**
- **End-to-end scanning workflow**
- **Scanner configuration integration**
- **Queue integration with realistic workloads**
- **Async engine coordination**
- **Scalability with repository size**

**Target Metrics:**
- CLI parsing: < 1ms
- End-to-end latency: < 100ms for interactive operations
- Scalability: Linear performance scaling with repository size

## Running Benchmarks

### Run All Benchmarks
```bash
cargo bench
```

### Run Specific Benchmark Suite
```bash
cargo bench --bench scanner_benchmarks
cargo bench --bench memory_benchmarks
cargo bench --bench queue_benchmarks
cargo bench --bench integration_benchmarks
```

### Run Specific Benchmark
```bash
cargo bench --bench queue_benchmarks -- single_producer_throughput
```

### Generate HTML Reports
```bash
cargo bench --bench scanner_benchmarks
# Reports generated in target/criterion/
```

## Performance Test Infrastructure

### Performance Validation Tests (`tests/performance/`)
Located in `tests/performance/async_validation.rs`:
- **Async scanner responsiveness**
- **Concurrent scanner load testing**
- **Task coordination validation**
- **Backpressure handling**
- **Cancellation safety**
- **Performance under various load scenarios**

Run performance validation tests:
```bash
cargo test --test async_validation
```

## Benchmark Results Interpretation

### Throughput Benchmarks
- **Elements/second**: Number of operations per second
- **Bytes/second**: Data processing rate
- **Messages/second**: Queue processing rate

### Latency Benchmarks
- **Average latency**: Mean processing time
- **95th percentile**: Response time for 95% of operations
- **Maximum latency**: Worst-case processing time

### Memory Benchmarks
- **Peak memory usage**: Maximum memory consumption
- **Memory overhead**: Additional memory beyond data
- **Allocation rate**: Memory allocations per second

## Performance Targets

### Queue System
- ✅ **Throughput**: > 10,000 messages/second
- ✅ **Latency**: < 1ms average processing
- ✅ **Filtering**: < 100μs ScanMode lookup
- ✅ **Memory**: < 1MB queue overhead
- ✅ **Scaling**: Near-linear to 16 producer threads

### Scanner System
- ✅ **Repository Processing**: > 1000 commits/second
- ✅ **Memory Usage**: < 256MB for typical repositories
- ✅ **Interactive Response**: < 100ms for user operations
- ✅ **Scalability**: Linear scaling with repository size

### Integration System
- ✅ **CLI Parsing**: < 1ms argument processing
- ✅ **Configuration**: < 10ms loading from file
- ✅ **End-to-End**: Complete workflow in reasonable time
- ✅ **Async Coordination**: Efficient task management

## Adding New Benchmarks

### 1. Add to existing benchmark file:
```rust
fn bench_new_feature(c: &mut Criterion) {
    c.bench_function("new_feature_name", |b| {
        b.iter(|| {
            // Benchmark code here
        })
    });
}

// Add to criterion_group!
criterion_group!(
    existing_benches,
    // ... existing benchmarks
    bench_new_feature
);
```

### 2. Create new benchmark file:
```rust
// benches/new_benchmarks.rs
use criterion::{criterion_group, criterion_main, Criterion};

fn bench_something(c: &mut Criterion) {
    // Implementation
}

criterion_group!(new_benches, bench_something);
criterion_main!(new_benches);
```

Add to `Cargo.toml`:
```toml
[[bench]]
name = "new_benchmarks"
harness = false
```

## Continuous Integration

Benchmarks should be run regularly to detect performance regressions:

```bash
# Run benchmarks and save baseline
cargo bench -- --save-baseline main

# Compare against baseline
cargo bench -- --baseline main
```

## Troubleshooting

### Common Issues

1. **"Cannot find bench target"**
   - Ensure benchmark is listed in `Cargo.toml`
   - Check file exists in `benches/` directory

2. **Benchmark takes too long**
   - Reduce iteration count with `--sample-size`
   - Use smaller test datasets

3. **Inconsistent results**
   - Run benchmarks multiple times
   - Check for background processes
   - Use isolated test environment

### Performance Debugging

1. **Profile with perf**:
   ```bash
   perf record --call-graph=dwarf cargo bench --bench scanner_benchmarks
   perf report
   ```

2. **Memory profiling**:
   ```bash
   valgrind --tool=massif cargo bench --bench memory_benchmarks
   ```

3. **Async debugging**:
   ```bash
   TOKIO_CONSOLE=1 cargo bench --bench integration_benchmarks
   ```