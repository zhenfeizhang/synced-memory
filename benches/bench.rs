use std::sync::Arc;
use std::thread;
use std::time::Instant;
use synched_memory::SynchedMemory;

// Simple iterative Fibonacci calculation for benchmarking
fn fibonacci(n: u64) -> u64 {
    if n <= 1 {
        return n;
    }

    let mut a = 0;
    let mut b = 1;

    for _ in 2..=n {
        let temp = a + b;
        a = b;
        b = temp;
    }

    b
}

// Benchmark configuration
const NUM_THREADS: usize = 4;
const MAX_SIZE_PER_THREAD: usize = 100;
const NUM_ROUNDS: usize = 10;

// Thread-specific Fibonacci iteration counts
const THREAD_FIBONACCI_COUNTS: [u64; NUM_THREADS] = [50000, 900000, 950000, 1000000];

#[derive(Clone, Copy, Default)]
struct BenchmarkData {
    fibonacci_result: u64,
}

fn main() {
    // Create synchronized memory
    let memory = Arc::new(SynchedMemory::<BenchmarkData>::new(
        NUM_THREADS,
        MAX_SIZE_PER_THREAD,
    ));

    // Start total benchmark timer
    let total_start = Instant::now();

    // Spawn benchmark threads
    let handles: Vec<_> = (0..NUM_THREADS)
        .map(|thread_id| {
            let mem = Arc::clone(&memory);

            thread::spawn(move || {
                let local_handler = mem.build_local_handler(thread_id);
                let fibonacci_count = THREAD_FIBONACCI_COUNTS[thread_id];

                for _round in 0..NUM_ROUNDS {
                    // Computation phase - measure Fibonacci calculation time
                    let fib_result = fibonacci(fibonacci_count);

                    // Prepare data to write to shared memory
                    let benchmark_data = BenchmarkData {
                        fibonacci_result: fib_result,
                    };

                    // Create data array (using some of the segment for demonstration)
                    let data_size = std::cmp::min(10, MAX_SIZE_PER_THREAD);
                    let mut data_to_write = vec![benchmark_data; data_size];

                    // Add some variation to the data
                    for (i, item) in data_to_write.iter_mut().enumerate() {
                        item.fibonacci_result = fib_result + i as u64;
                    }

                    // Synchronization phase - write and sync with other threads
                    let _global_data =
                        mem.write_local_memory_and_sync_read(&local_handler, &data_to_write);
                }
            })
        })
        .collect();

    // Wait for all threads to complete
    for handle in handles.into_iter() {
        handle.join().unwrap();
    }

    let total_duration = total_start.elapsed();

    // Finalize to record completion timestamps
    memory.finalize();

    // Get timing statistics and plot

    #[cfg(feature = "profiler")]
    let timing_stats = memory.get_all_timing_stats();

    // Print basic benchmark info
    println!("SynchedMemory Benchmark Results");
    println!("===============================");
    println!("Threads: {}", NUM_THREADS);
    println!("Rounds: {}", NUM_ROUNDS);
    println!(
        "Fibonacci iterations per thread: {:?}",
        THREAD_FIBONACCI_COUNTS
    );
    println!("Max elements per thread: {}", MAX_SIZE_PER_THREAD);
    println!("Total execution time: {:?}", total_duration);

    // Plot detailed timing analysis
    #[cfg(feature = "profiler")]
    timing_stats.plot();

    println!("\nBenchmark completed successfully!");
}
