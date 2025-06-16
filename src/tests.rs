#![allow(clippy::needless_range_loop)]

use super::*;
use rand::{RngCore, thread_rng};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[test]
fn test_basic_memory_creation() {
    let memory: SynchedMemory<i32> = SynchedMemory::new(3, 10);
    assert_eq!(memory.num_threads(), 3);
    assert_eq!(memory.data_len(), 30); // 3 threads * 10 elements each
}

#[test]
fn test_read_global_returns_data() {
    let memory: SynchedMemory<i32> = SynchedMemory::new(2, 5);

    // read_global now always returns data (no longer returns Option)
    let result = memory.read_global();
    assert_eq!(result.len(), 10); // 2 threads * 5 size each

    // Initially all elements should be default (0 for i32)
    assert!(result.iter().all(|&x| x == 0));
}

#[test]
fn test_coordinated_write_read_cycle() {
    const NUM_THREADS: usize = 4;
    const SEGMENT_SIZE: usize = 5;

    let memory = Arc::new(SynchedMemory::<i32>::new(NUM_THREADS, SEGMENT_SIZE));

    println!(
        "Starting coordinated write-read test with {} threads",
        NUM_THREADS
    );

    // Spawn threads that will coordinate their writes
    let handles: Vec<_> = (0..NUM_THREADS)
        .map(|thread_id| {
            let mem = Arc::clone(&memory);

            std::thread::spawn(move || {
                let local_handler = mem.build_local_handler(thread_id);

                // Each thread writes its ID repeated
                let data = vec![(thread_id + 1) as i32; 3]; // +1 to avoid zeros

                println!("Thread {} writing data: {:?}", thread_id, data);

                // This will coordinate with all other threads
                let global_data = mem.write_local_memory_and_sync_read(&local_handler, &data);

                println!(
                    "Thread {} read global data (length: {})",
                    thread_id,
                    global_data.len()
                );

                // Verify this thread's data in the global memory
                let start_idx = thread_id * SEGMENT_SIZE;
                let thread_segment = &global_data[start_idx..start_idx + SEGMENT_SIZE];

                // Check that our data is there (first 3 elements should be our data)
                for i in 0..3 {
                    assert_eq!(
                        thread_segment[i],
                        (thread_id + 1) as i32,
                        "Thread {}: expected {} at position {}, got {}",
                        thread_id,
                        thread_id + 1,
                        i,
                        thread_segment[i]
                    );
                }

                // Remaining elements should be default (0)
                for i in 3..SEGMENT_SIZE {
                    assert_eq!(
                        thread_segment[i], 0,
                        "Thread {}: expected 0 at position {}, got {}",
                        thread_id, i, thread_segment[i]
                    );
                }

                println!("Thread {} verification completed successfully", thread_id);
                global_data
            })
        })
        .collect();

    // Collect results from all threads
    let mut all_results = Vec::new();
    for (i, handle) in handles.into_iter().enumerate() {
        match handle.join() {
            Ok(result) => {
                println!("Thread {} joined successfully", i);
                all_results.push(result);
            }
            Err(e) => {
                println!("Thread {} panicked: {:?}", i, e);
                panic!("Thread {} panicked", i);
            }
        }
    }

    // All threads should have seen the same global data
    let first_result = &all_results[0];
    for (i, result) in all_results.iter().enumerate() {
        assert_eq!(
            result, first_result,
            "Thread {} saw different global data than thread 0",
            i
        );
    }

    println!("Coordinated write-read test completed successfully!");
}

#[test]
fn test_multiple_rounds_coordination() {
    const NUM_THREADS: usize = 3;
    const SEGMENT_SIZE: usize = 4;
    const NUM_ROUNDS: usize = 5;

    let memory = Arc::new(SynchedMemory::<i32>::new(NUM_THREADS, SEGMENT_SIZE));

    println!(
        "Starting multi-round coordination test: {} threads, {} rounds",
        NUM_THREADS, NUM_ROUNDS
    );

    let handles: Vec<_> = (0..NUM_THREADS)
        .map(|thread_id| {
            let mem = Arc::clone(&memory);

            std::thread::spawn(move || {
                let local_handler = mem.build_local_handler(thread_id);
                let mut round_results = Vec::new();

                for round in 0..NUM_ROUNDS {
                    println!("Thread {} starting round {}", thread_id, round);

                    // Create round-specific data
                    let data = vec![(thread_id * 10 + round + 1) as i32; 2];

                    // Coordinate write and read
                    let global_data = mem.write_local_memory_and_sync_read(&local_handler, &data);

                    // Store the result
                    round_results.push(global_data.clone());

                    // Verify our data is in the global state
                    let start_idx = thread_id * SEGMENT_SIZE;
                    let thread_segment = &global_data[start_idx..start_idx + SEGMENT_SIZE];

                    let expected_value = (thread_id * 10 + round + 1) as i32;
                    assert_eq!(thread_segment[0], expected_value);
                    assert_eq!(thread_segment[1], expected_value);

                    println!(
                        "Thread {} completed round {} successfully",
                        thread_id, round
                    );

                    // Small delay between rounds
                    std::thread::sleep(Duration::from_millis(10));
                }

                println!("Thread {} completed all {} rounds", thread_id, NUM_ROUNDS);
                round_results
            })
        })
        .collect();

    // Wait for all threads
    let all_thread_results: Vec<_> = handles
        .into_iter()
        .enumerate()
        .map(|(i, handle)| match handle.join() {
            Ok(results) => {
                println!("Thread {} finished all rounds successfully", i);
                results
            }
            Err(e) => {
                panic!("Thread {} panicked: {:?}", i, e);
            }
        })
        .collect();

    memory.finalize();

    // Verify all threads saw consistent data in each round
    for round in 0..NUM_ROUNDS {
        let first_thread_data = &all_thread_results[0][round];
        for thread_id in 1..NUM_THREADS {
            assert_eq!(
                &all_thread_results[thread_id][round], first_thread_data,
                "Round {}: Thread {} saw different data than thread 0",
                round, thread_id
            );
        }
        println!("Round {} data consistency verified", round);
    }

    println!("Multi-round coordination test completed successfully!");

    #[cfg(feature = "profiler")]
    {
        let time_stat = memory.get_all_timing_stats();
        time_stat.plot();
    }
}

#[test]
#[should_panic(expected = "Thread ID 3 out of bounds")]
fn test_build_local_handler_bounds_checking() {
    let memory: SynchedMemory<i32> = SynchedMemory::new(3, 10);

    let _local_handler_0 = memory.build_local_handler(0);
    let _local_handler_1 = memory.build_local_handler(1);
    let _local_handler_2 = memory.build_local_handler(2);

    // This should panic
    let _local_handler_3 = memory.build_local_handler(3);
}

#[test]
fn test_write_local_memory_basic() {
    let memory: SynchedMemory<i32> = SynchedMemory::new(2, 5);

    let local_handler_0 = memory.build_local_handler(0);
    let local_handler_1 = memory.build_local_handler(1);

    // Write data to local segments
    memory.write_local_memory(&local_handler_0, &[1, 2, 3]);
    memory.write_local_memory(&local_handler_1, &[4, 5, 6, 7]);

    // Read global memory directly
    let global_data = memory.read_global();

    // Verify the data was written correctly to the appropriate segments
    assert_eq!(&global_data[0..3], &[1, 2, 3]); // Thread 0's data
    assert_eq!(&global_data[3], &0); // Thread 0's remaining space (cleared)
    assert_eq!(&global_data[4], &0); // Thread 0's remaining space (cleared)
    assert_eq!(&global_data[5..9], &[4, 5, 6, 7]); // Thread 1's data
    assert_eq!(&global_data[9], &0); // Thread 1's remaining space (cleared)

    println!("Basic write test completed successfully");
}

#[test]
#[should_panic(expected = "Data length 11 exceeds maximum size per thread 10")]
fn test_write_local_memory_size_validation() {
    let memory: SynchedMemory<i32> = SynchedMemory::new(2, 10);
    let local_handler = memory.build_local_handler(0);

    // Try to write more data than the segment can hold
    let too_much_data = vec![1; 11];
    memory.write_local_memory(&local_handler, &too_much_data);
}

#[test]
fn test_performance_simple() {
    const NUM_THREADS: usize = 4;
    const SEGMENT_SIZE: usize = 100;
    const NUM_ITERATIONS: usize = 100;

    let memory = Arc::new(SynchedMemory::<u64>::new(NUM_THREADS, SEGMENT_SIZE));

    let start_time = Instant::now();

    let handles: Vec<_> = (0..NUM_THREADS)
        .map(|thread_id| {
            let mem = Arc::clone(&memory);

            std::thread::spawn(move || {
                let local_handler = mem.build_local_handler(thread_id);
                let mut rng = thread_rng();

                for iteration in 0..NUM_ITERATIONS {
                    // Generate some random data
                    let data_size = 10 + (rng.next_u32() as usize) % 20; // 10-29 elements
                    let data: Vec<u64> = (0..data_size)
                        .map(|i| (thread_id as u64 * 1000 + iteration as u64 * 100 + i as u64))
                        .collect();

                    // Write and sync
                    let _global_data = mem.write_local_memory_and_sync_read(&local_handler, &data);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let total_time = start_time.elapsed();
    let total_operations = NUM_THREADS * NUM_ITERATIONS;

    println!("Performance test completed:");
    println!("  Total time: {:?}", total_time);
    println!("  Total operations: {}", total_operations);
    println!(
        "  Average time per operation: {:?}",
        total_time / total_operations as u32
    );
    println!(
        "  Operations per second: {:.0}",
        total_operations as f64 / total_time.as_secs_f64()
    );
}
