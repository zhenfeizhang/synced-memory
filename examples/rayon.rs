use rayon::prelude::*;
use std::sync::Arc;
use synched_memory::SynchedMemory;

#[derive(Clone, Copy, Default, Debug)]
pub struct FibonacciResult {
    pub iteration: usize,
    pub fibonacci_input: u64,
    pub fibonacci_output: u64,
    pub thread_id: usize,
}

// Simple iterative Fibonacci calculation
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

fn main() {
    println!("🚀 Fibonacci with Rayon + SynchedMemory Integration");
    println!("==================================================\n");

    // Configuration
    const NUM_THREADS: usize = 4;
    const MAX_RESULTS_PER_THREAD: usize = 50;
    const NUM_ITERATIONS: usize = 10;
    const FIBONACCI_INPUTS: [u64; NUM_THREADS] = [35, 36, 37, 38]; // Different workloads per thread

    // Allocate shared memory that will be used by all threads
    let shared_memory = Arc::new(SynchedMemory::<FibonacciResult>::new(
        NUM_THREADS,
        MAX_RESULTS_PER_THREAD,
    ));

    println!("📊 Configuration:");
    println!("  Threads: {}", NUM_THREADS);
    println!("  Iterations per thread: {}", NUM_ITERATIONS);
    println!("  Fibonacci inputs: {:?}", FIBONACCI_INPUTS);
    println!("  Max results per thread: {}\n", MAX_RESULTS_PER_THREAD);

    let start_time = std::time::Instant::now();

    // Use Rayon's parallel iterator interface
    let results: Vec<Vec<FibonacciResult>> = (0..NUM_THREADS)
        .into_par_iter()
        .map(|thread_id| {
            println!("🧵 Thread {} starting...", thread_id);

            // Each thread gets its own local handler for the shared memory
            let local_handler = shared_memory.build_local_handler(thread_id);
            let fibonacci_input = FIBONACCI_INPUTS[thread_id];
            let mut thread_results = Vec::new();

            // Repeat the computation and sync cycle 10 times
            for iteration in 0..NUM_ITERATIONS {
                // 1. Calculate Fibonacci for specified number of times
                let start_fib = std::time::Instant::now();
                let fibonacci_output = fibonacci(fibonacci_input);
                let fib_duration = start_fib.elapsed();

                println!(
                    "  Thread {}, Iteration {}: fib({}) = {} (computed in {:?})",
                    thread_id, iteration, fibonacci_input, fibonacci_output, fib_duration
                );

                // Prepare data to write to shared memory
                let local_data = vec![
                    FibonacciResult {
                        iteration,
                        fibonacci_input,
                        fibonacci_output,
                        thread_id,
                    };
                    1
                ]; // Writing 1 result per iteration

                // 2. Sync up memory - this blocks until all threads reach this point
                let global_data =
                    shared_memory.write_local_memory_and_sync_read(&local_handler, &local_data);

                // Store this iteration's global state
                let valid_results: Vec<_> = global_data
                    .into_iter()
                    .filter(|r| r.fibonacci_input != 0 || r.fibonacci_output != 0) // Filter out default values
                    .collect();

                thread_results.push(valid_results);

                println!(
                    "  Thread {} synchronized at iteration {} (saw {} total results)",
                    thread_id,
                    iteration,
                    thread_results.last().unwrap().len()
                );
            }

            println!("✅ Thread {} completed all iterations", thread_id);

            // Return flattened results from this thread's perspective
            thread_results.into_iter().flatten().collect()
        })
        .collect();

    let total_duration = start_time.elapsed();

    // Finalize timing collection
    shared_memory.finalize();

    // Print results summary
    println!("\n📈 Results Summary:");
    println!("==================");

    for (thread_id, thread_results) in results.iter().enumerate() {
        println!(
            "Thread {} processed {} results:",
            thread_id,
            thread_results.len()
        );

        // Group by iteration to show synchronization points
        let mut by_iteration: std::collections::HashMap<usize, Vec<&FibonacciResult>> =
            std::collections::HashMap::new();

        for result in thread_results {
            by_iteration
                .entry(result.iteration)
                .or_default()
                .push(result);
        }

        for iteration in 0..NUM_ITERATIONS {
            if let Some(iteration_results) = by_iteration.get(&iteration) {
                let thread_outputs: Vec<_> = iteration_results
                    .iter()
                    .map(|r| {
                        format!(
                            "T{}:fib({})={}",
                            r.thread_id, r.fibonacci_input, r.fibonacci_output
                        )
                    })
                    .collect();
                println!("  Iteration {}: [{}]", iteration, thread_outputs.join(", "));
            }
        }
        println!();
    }

    println!("⏱️  Total execution time: {:?}", total_duration);

    // Display timing statistics if profiler is enabled
    #[cfg(feature = "profiler")]
    {
        println!("\n📊 Detailed Timing Analysis:");
        let stats = shared_memory.get_all_timing_stats();
        stats.plot();

        println!("\nPer-thread timing breakdown:");
        for thread_id in 0..NUM_THREADS {
            if let Some(total_comp_time) = stats.thread_total_computation_time(thread_id) {
                println!(
                    "  Thread {}: {:?} total computation time",
                    thread_id, total_comp_time
                );
            }
        }
    }

    #[cfg(not(feature = "profiler"))]
    {
        println!("\n💡 Tip: Enable the 'profiler' feature for detailed timing analysis!");
        println!("   Add 'features = [\"profiler\"]' to your Cargo.toml dependency");
    }

    println!("\n✅ Rayon + SynchedMemory integration completed successfully!");
}
