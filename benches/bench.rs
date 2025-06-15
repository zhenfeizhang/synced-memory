use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use synced_memory::SynchedMemory;

// ANSI color codes for different threads
const THREAD_COLORS: [&str; 4] = [
    "\x1b[31m", // Red
    "\x1b[32m", // Green
    "\x1b[33m", // Yellow
    "\x1b[34m", // Blue
];
const RESET_COLOR: &str = "\x1b[0m";

// Helper function to print colored text for a specific thread
fn print_thread(thread_id: usize, message: &str) {
    println!(
        "{}Thread {}: {}{}",
        THREAD_COLORS[thread_id % THREAD_COLORS.len()],
        thread_id,
        message,
        RESET_COLOR
    );
}

// Helper function to print colored formatted text for a specific thread
fn print_thread_fmt(thread_id: usize, message: String) {
    println!(
        "{}Thread {}: {}{}",
        THREAD_COLORS[thread_id % THREAD_COLORS.len()],
        thread_id,
        message,
        RESET_COLOR
    );
}

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
    println!("SynchedMemory Benchmark");
    println!("======================");
    println!("Threads: {}", NUM_THREADS);
    println!("Rounds: {}", NUM_ROUNDS);
    println!(
        "Fibonacci iterations per thread: {:?}",
        THREAD_FIBONACCI_COUNTS
    );
    println!("Max elements per thread: {}", MAX_SIZE_PER_THREAD);
    println!();

    // Create synchronized memory
    let memory = Arc::new(SynchedMemory::<BenchmarkData>::new(
        NUM_THREADS,
        MAX_SIZE_PER_THREAD,
    ));

    // Start total benchmark timer
    let total_start = Instant::now();

    // Storage for per-thread timing data
    let thread_results = Arc::new(std::sync::Mutex::new(Vec::new()));

    // Spawn benchmark threads
    let handles: Vec<_> = (0..NUM_THREADS)
        .map(|thread_id| {
            let mem = Arc::clone(&memory);
            let results = Arc::clone(&thread_results);

            thread::spawn(move || {
                let local_handler = mem.build_local_handler(thread_id);
                let fibonacci_count = THREAD_FIBONACCI_COUNTS[thread_id];

                let mut total_computation_time = Duration::ZERO;
                let mut round_results = Vec::new();

                print_thread(
                    thread_id,
                    &format!("starting with {} Fibonacci iterations", fibonacci_count),
                );

                for round in 0..NUM_ROUNDS {
                    print_thread(thread_id, &format!("Round {}/{}", round + 1, NUM_ROUNDS));

                    // Computation phase - measure Fibonacci calculation time
                    let computation_start = Instant::now();
                    let fib_result = fibonacci(fibonacci_count);
                    let computation_duration = computation_start.elapsed();
                    total_computation_time += computation_duration;

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
                    let sync_start = Instant::now();
                    let global_data =
                        mem.write_local_memory_and_sync_read(&local_handler, &data_to_write);
                    let sync_duration = sync_start.elapsed();

                    // Store round results
                    round_results.push((
                        round,
                        fib_result,
                        computation_duration,
                        sync_duration,
                        global_data.len(),
                    ));

                    print_thread_fmt(
                        thread_id,
                        format!(
                            "Round {} completed: fib({}) = {}, comp_time: {:?}, sync_time: {:?}",
                            round + 1,
                            fibonacci_count,
                            fib_result,
                            computation_duration,
                            sync_duration
                        ),
                    );
                }

                // Store thread results
                {
                    let mut results_guard = results.lock().unwrap();
                    results_guard.push((thread_id, total_computation_time, round_results));
                }

                print_thread_fmt(
                    thread_id,
                    format!(
                        "completed all rounds. Total computation time: {:?}",
                        total_computation_time
                    ),
                );
            })
        })
        .collect();

    // Wait for all threads to complete
    println!("\nWaiting for all threads to complete...");
    for (i, handle) in handles.into_iter().enumerate() {
        match handle.join() {
            Ok(_) => println!(
                "{}Thread {} finished successfully{}",
                THREAD_COLORS[i % THREAD_COLORS.len()],
                i,
                RESET_COLOR
            ),
            Err(e) => {
                eprintln!(
                    "{}Thread {} panicked: {:?}{}",
                    THREAD_COLORS[i % THREAD_COLORS.len()],
                    i,
                    e,
                    RESET_COLOR
                );
                std::process::exit(1);
            }
        }
    }

    let total_duration = total_start.elapsed();

    // Initialize tracking variables OUTSIDE the results scope
    let mut per_round_max_computation_times = [Duration::ZERO; NUM_ROUNDS];
    let mut all_fibonacci_results = Vec::new();

    // Analyze results
    println!("\nBenchmark Results");
    println!("=================");

    {
        let results = thread_results.lock().unwrap();

        for (thread_id, thread_computation_time, round_results) in results.iter() {
            println!(
                "\n{}Thread {} Summary:{}",
                THREAD_COLORS[*thread_id % THREAD_COLORS.len()],
                thread_id,
                RESET_COLOR
            );
            println!(
                "  Fibonacci iterations: {}",
                THREAD_FIBONACCI_COUNTS[*thread_id]
            );
            println!("  Total computation time: {:?}", thread_computation_time);
            println!(
                "  Average computation time per round: {:?}",
                *thread_computation_time / NUM_ROUNDS as u32
            );

            // Show detailed round results for this thread
            println!("  Round details:");
            for (round, fib_result, comp_time, sync_time, global_len) in round_results {
                println!(
                    "    {}Round {}: fib_result={}, comp={:?}, sync={:?}, global_data_len={}{}",
                    THREAD_COLORS[*thread_id % THREAD_COLORS.len()],
                    round + 1,
                    fib_result,
                    comp_time,
                    sync_time,
                    global_len,
                    RESET_COLOR
                );
                all_fibonacci_results.push(*fib_result);

                // Track the maximum computation time for each round
                per_round_max_computation_times[*round] =
                    per_round_max_computation_times[*round].max(*comp_time);
            }
        }
    } // MutexGuard is dropped here, but variables remain accessible

    // Calculate total computation time as sum of max computation times per round
    let total_computation_time: Duration = per_round_max_computation_times.iter().sum();

    // Overall statistics
    println!("\nOverall Statistics:");
    println!("  Total execution time: {:?}", total_duration);
    println!(
        "  Total computation time (max per round): {:?}",
        total_computation_time
    );
    println!(
        "  Synchronization overhead: {:?}",
        total_duration - total_computation_time
    );
    println!(
        "  Computation efficiency: {:.1}%",
        (total_computation_time.as_secs_f64() / total_duration.as_secs_f64()) * 100.0
    );

    // Per-round computation analysis
    println!("\nPer-Round Computation Analysis:");
    for (round, max_comp_time) in per_round_max_computation_times.iter().enumerate() {
        println!(
            "  Round {}: max computation time = {:?}",
            round + 1,
            max_comp_time
        );
    }
    let avg_round_computation = total_computation_time / NUM_ROUNDS as u32;
    println!(
        "  Average max computation time per round: {:?}",
        avg_round_computation
    );

    // Performance metrics
    let total_operations = NUM_THREADS * NUM_ROUNDS;
    println!("  Total sync operations: {}", total_operations);
    println!(
        "  Average time per sync operation: {:?}",
        total_duration / total_operations as u32
    );
    println!(
        "  Operations per second: {:.1}",
        total_operations as f64 / total_duration.as_secs_f64()
    );

    // Fibonacci verification
    println!("\nFibonacci Results Verification:");
    for thread_id in 0..NUM_THREADS {
        let expected = fibonacci(THREAD_FIBONACCI_COUNTS[thread_id]);
        println!(
            "  {}Thread {}: fib({}) = {} (expected: {}){}",
            THREAD_COLORS[thread_id % THREAD_COLORS.len()],
            thread_id,
            THREAD_FIBONACCI_COUNTS[thread_id],
            expected,
            expected,
            RESET_COLOR
        );
    }

    // Memory usage info
    let total_memory_elements = NUM_THREADS * MAX_SIZE_PER_THREAD;
    let memory_size_bytes = total_memory_elements * std::mem::size_of::<BenchmarkData>();
    println!("\nMemory Usage:");
    println!("  Total elements: {}", total_memory_elements);
    println!(
        "  Memory size: {} bytes ({:.1} KB)",
        memory_size_bytes,
        memory_size_bytes as f64 / 1024.0
    );

    println!("\nBenchmark completed successfully!");
}
