# SynchedMemory

[![CI](https://github.com/zhenfeizhang/synced-memory/workflows/CI/badge.svg)](https://github.com/zhenfeizhang/synced-memory/actions)

A high-performance, lock-free data structure for coordinating memory access between threads in a write-then-read pattern. Perfect for scenarios where multiple threads need to perform coordinated computation cycles with synchronized data sharing.

## 🚀 Features

- **Lock-Free Performance**: Uses atomic operations and barriers instead of mutexes
- **Segmented Memory**: Each thread has exclusive access to its own memory segment
- **Automatic Synchronization**: Built-in barriers coordinate write-then-read cycles
- **Memory Safety**: Prevents data races through careful design and `UnsafeCell`
- **Zero-Copy Reads**: Direct memory access for maximum performance
- **Flexible Data Types**: Generic over any `Default + Clone + Copy` type
- **Optional Profiling**: Feature-gated timing analytics with detailed performance plots

## 🏗️ Architecture

```
Thread 0: [data...] | Thread 1: [data...] | Thread 2: [data...] | Thread N: [data...]
          ^                      ^                      ^                      ^
          |                      |                      |                      |
 LocalMemHandler(0)     LocalMemHandler(1)     LocalMemHandler(2)     LocalMemHandler(N)
```

### How It Works

1. **Segmented Memory**: Total memory is divided into `num_threads × segment_size` elements
2. **Exclusive Writes**: Each thread writes only to its own segment (no contention)
3. **Barrier Synchronization**: All threads coordinate at write barriers
4. **Synchronized Reads**: Complete global state available after synchronization

## 📦 Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
synched-memory = { git = "https://github.com/zhenfeizhang/synced-memory" }

# Or with profiling support
synched-memory = { git = "https://github.com/zhenfeizhang/synced-memory", features = ["profiler"] }
```

## 🔧 Quick Start

### Basic Usage

```rust
use std::sync::Arc;
use std::thread;
use synched_memory::SynchedMemory;

fn main() {
    // Create synchronized memory for 4 threads, 10 elements per thread
    let memory = Arc::new(SynchedMemory::<i32>::new(4, 10));

    let handles: Vec<_> = (0..4).map(|thread_id| {
        let mem = Arc::clone(&memory);
        thread::spawn(move || {
            // Each thread gets its own local memory handler
            let local_handler = mem.build_local_handler(thread_id);
            
            // Write data and automatically sync with other threads
            let data = vec![thread_id as i32; 5];
            let global_data = mem.write_local_memory_and_sync_read(&local_handler, &data);
            
            // All threads have now written and read the synchronized state
            println!("Thread {} sees {} total elements", thread_id, global_data.len());
            
            // Verify our data is in the global state
            let start_idx = thread_id * 10;
            assert_eq!(&global_data[start_idx..start_idx + 5], &data);
        })
    }).collect();

    for handle in handles {
        handle.join().unwrap();
    }
}
```

### With Performance Profiling

```rust
use synched_memory::SynchedMemory;

fn main() {
    let memory = Arc::new(SynchedMemory::<i32>::new(4, 10));

    // ... spawn threads and do work ...

    // Finalize timing collection
    memory.finalize();

    // Get detailed timing analysis
    #[cfg(feature = "profiler")]
    {
        let timing_stats = memory.get_all_timing_stats();
        timing_stats.plot(); // Prints detailed timing table
    }
}
```

## 💡 Use Cases

### Iterative Simulations
Perfect for simulations where threads compute local state and need global visibility:

```rust
// Game of Life, physics simulations, cellular automata
let grid = Arc::new(SynchedMemory::<Cell>::new(num_workers, cells_per_worker));

for iteration in 0..1000 {
    // Each worker computes its portion of the simulation
    let local_cells = compute_local_simulation(&previous_state, worker_id);
    
    // Sync with all threads and get complete global state
    let global_state = grid.write_local_memory_and_sync_read(&local_handler, &local_cells);
    
    // Use global state for boundary conditions in next iteration
    previous_state = global_state;
}
```

### Parallel Algorithms
Coordinate distributed computations with synchronized checkpoints:

```rust
// Map-reduce operations, iterative algorithms
let shared_results = Arc::new(SynchedMemory::<ComputeResult>::new(workers, chunk_size));

for iteration in 0..max_iterations {
    // Each thread processes its data chunk
    let local_results = process_data_chunk(thread_id, &input_data);
    
    // Synchronize and check for convergence
    let all_results = shared_results.write_local_memory_and_sync_read(&handler, &local_results);
    
    if has_converged(&all_results) {
        break;
    }
}
```

### Real-Time Processing
High-performance coordination for time-sensitive applications:

```rust
// Audio processing, real-time control systems
let audio_buffers = Arc::new(SynchedMemory::<f32>::new(channels, samples_per_channel));

loop {
    // Each channel processes audio independently
    let processed_samples = process_audio_channel(channel_id, &input_audio);
    
    // Sync for mixing and output
    let all_channels = audio_buffers.write_local_memory_and_sync_read(&handler, &processed_samples);
    
    // Mix and output combined audio
    output_mixed_audio(&all_channels);
}
```

## 🔄 Core Pattern: Write-Then-Read

The fundamental coordination pattern is simple and powerful:

```rust
// 1. Each thread writes to its own segment (no contention)
let data = compute_my_portion();

// 2. Synchronize with all threads and read global state (one atomic operation)
let global_data = memory.write_local_memory_and_sync_read(&local_handler, &data);

// 3. Process complete global state for next iteration
prepare_next_iteration(&global_data);
```

This pattern ensures:
- **No write contention**: Each thread has exclusive access to its segment
- **Coordinated synchronization**: All threads see the same consistent global state
- **Efficient memory access**: Direct memory operations without copying overhead

## 🚀 Performance

### Benchmark Results (4 threads, 10 rounds, Variable Fibonacci workload)

| Metric | Value |
|--------|-------|
| Total Operations | 40 sync cycles |
| Total Execution Time | 4.596ms |
| Average Operation Time | ~0.115ms |
| Throughput | ~8,704 ops/sec |
| Memory Efficiency | Zero allocations during operation |
| Lock Contention | None (lock-free) |
| Fibonacci Iterations | [50K, 900K, 950K, 1M] per thread |

### Performance Characteristics

- **O(1) Write Complexity**: Each thread writes to its own segment
- **O(n) Read Complexity**: Global state read (unavoidable for coordination)
- **Memory Usage**: `num_threads × segment_size × sizeof(T)` (allocated once)
- **Cache Efficiency**: Each thread works on its own cache lines
- **No Dynamic Allocation**: All memory pre-allocated at creation

## 📊 Profiling & Analysis

Enable the `profiler` feature for detailed performance analysis:

```toml
[dependencies]
synched-memory = { git = "https://github.com/zhenfeizhang/synced-memory", features = ["profiler"] }
```

### Timing Analysis

```rust
#[cfg(feature = "profiler")]
{
    // Get comprehensive timing statistics
    let stats = memory.get_all_timing_stats();
    
    // Print detailed timing table
    stats.plot();
    
    // Access individual metrics
    let total_runtime = stats.total_runtime();
    let thread_0_compute_time = stats.thread_total_computation_time(0);
    let average_block_time = stats.thread_average_computation_time(0);
}
```

### Sample Timing Output

```
📊 COMPUTATION TIME TABLE
Time format: milliseconds (ms) with microsecond precision
Total Runtime: 4.597 ms
================================================================================
Block        Thread 0     Thread 1     Thread 2     Thread 3    
------------ ------------ ------------ ------------ ------------
0            0.025        0.437        0.394        0.518       
1            0.026        0.379        0.380        0.460       
2            0.025        0.395        0.380        0.408       
3            0.025        0.359        0.376        0.402       
4            0.025        0.356        0.376        0.402       
5            0.028        0.372        0.386        0.407       
6            0.025        0.383        0.405        0.428       
7            0.023        0.406        0.428        0.478       
8            0.022        0.396        0.421        0.436       
9            0.020        0.366        0.424        0.412       
10           0.044        0.045        0.046        0.039       
================================================================
Total Comp   0.289        3.892        4.017        4.391       
Total Time   4.597        4.597        4.597        4.597       
Comp Ratio%  6.3          84.7         87.4         95.5        
================================================================================

Legend:
  • Total Comp: Total computation time per thread (ms)
  • Total Time: Total runtime from init to completion (ms)
  • Comp Ratio: Computation time as percentage of total runtime (%)
  • —: No data available for this block
```

This example demonstrates:
- **Variable Workloads**: Different Fibonacci counts per thread (50K to 1M iterations)
- **Load Balancing**: Thread 0 has much lighter work (6.3% CPU) vs Thread 3 (95.5% CPU)
- **Synchronization Overhead**: Final block shows sync overhead (~0.04ms per thread)
- **Consistent Timing**: Similar computation times per thread across blocks 0-9

## 🛡️ Safety Guarantees

### Memory Safety
- **No Data Races**: Each thread has exclusive write access to its segment
- **Barrier Synchronization**: Coordinated write-then-read phases prevent inconsistency
- **Bounds Checking**: All memory access is validated and bounded
- **Type Safety**: Generic design ensures type consistency across threads

### Thread Safety
- **Segmented Access**: Threads never write to overlapping memory locations
- **Atomic Coordination**: All synchronization uses lock-free atomic operations
- **Validated Handles**: Thread IDs are validated to prevent out-of-bounds access
- **Safe Interior Mutability**: `UnsafeCell` used correctly with external coordination

## 📋 API Reference

### Core Types

```rust
// Main synchronized memory structure
pub struct SynchedMemory<T: Default + Clone + Copy>

// Handle for thread-local memory access
pub struct LocalMemHandler<T>

// Timing statistics (profiler feature only)
#[cfg(feature = "profiler")]
pub struct TimingStats
```

### Essential Methods

```rust
// Construction
SynchedMemory::new(num_threads: usize, segment_size: usize) -> Self

// Thread setup
build_local_handler(&self, thread_id: usize) -> LocalMemHandler<T>

// Core coordination (recommended)
write_local_memory_and_sync_read(&self, handler: &LocalMemHandler<T>, data: &[T]) -> Vec<T>

// Finalization
finalize(&self)

// Profiling (feature = "profiler" only)
get_all_timing_stats(&self) -> TimingStats
get_thread_computation_times(&self, thread_id: usize) -> Vec<Duration>
```

### Advanced Methods

```rust
// Low-level operations
write_local_memory(&self, handler: &LocalMemHandler<T>, data: &[T])
read_global(&self) -> Vec<T>
wait_write_barrier(&self)

// Inspection
num_threads(&self) -> usize
data_len(&self) -> usize
```

## 🔧 Advanced Usage

### Multiple Computation Rounds

```rust
let memory = Arc::new(SynchedMemory::<f64>::new(threads, segment_size));

for round in 0..num_rounds {
    let local_handler = memory.build_local_handler(thread_id);
    
    // Compute based on previous round's global state
    let new_data = compute_round(round, &previous_global_state);
    
    // Synchronize and get updated global state
    let global_state = memory.write_local_memory_and_sync_read(&local_handler, &new_data);
    
    previous_global_state = global_state;
}
```

### Manual Synchronization Control

```rust
// For advanced users who need fine-grained control
let local_handler = memory.build_local_handler(thread_id);

// Write phase
memory.write_local_memory(&local_handler, &my_data);

// Manual barrier synchronization
memory.wait_write_barrier();

// Read phase
let global_data = memory.read_global();
```

### Error Handling

```rust
// API uses panics for programming errors (bounds violations, etc.)
// Use catch_unwind for graceful error recovery if needed
let result = std::panic::catch_unwind(|| {
    memory.write_local_memory_and_sync_read(&local_handler, &data)
});

match result {
    Ok(global_data) => process_success(global_data),
    Err(_) => handle_coordination_error(),
}
```

## 🧪 Testing

```bash
# Run basic test suite
cargo test

# Test with profiler features
cargo test --features profiler

# Run with output for timing analysis
cargo test --features profiler -- --nocapture

# Performance benchmarks
cargo test bench --release
```

Test coverage includes:
- Multi-threaded coordination correctness
- Memory safety under high contention
- Performance regression detection
- Edge case validation (empty data, single thread, etc.)
- Profiler accuracy verification

## 📈 Performance Optimization

### Memory Layout Optimization
```rust
// Align segment sizes to cache lines (64 bytes typically)
let cache_line_size = 64 / std::mem::size_of::<T>();
let aligned_segment_size = ((desired_size + cache_line_size - 1) / cache_line_size) * cache_line_size;
let memory = SynchedMemory::new(threads, aligned_segment_size);
```

### Thread Affinity
```rust
// Pin threads to specific cores for consistent performance
use core_affinity;

thread::spawn(move || {
    core_affinity::set_for_current(core_affinity::CoreId { id: thread_id });
    
    let local_handler = memory.build_local_handler(thread_id);
    // ... computation work ...
});
```

### Load Balancing
```rust
// Monitor timing stats to identify imbalanced workloads
#[cfg(feature = "profiler")]
{
    let stats = memory.get_all_timing_stats();
    for thread_id in 0..num_threads {
        let avg_time = stats.thread_average_computation_time(thread_id);
        println!("Thread {} average: {:?}", thread_id, avg_time);
    }
}
```

## 🤝 Contributing

We welcome contributions! Please see our [contribution guidelines](CONTRIBUTING.md).

### Development Setup
```bash
git clone https://github.com/zhenfeizhang/synced-memory
cd synced-memory

# Development build
cargo build

# Run tests
cargo test --all-features

# Check formatting and lints
cargo fmt --check
cargo clippy --all-features
```

### Performance Testing
```bash
# Run benchmark suite
cargo test bench --release --features profiler -- --nocapture
```
---

**Questions?** Open an issue at https://github.com/zhenfeizhang/synced-memory/issues

**Using in production?** We'd love to hear about your use case and performance results!