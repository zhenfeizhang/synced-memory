use std::{
    cell::UnsafeCell,
    sync::Arc,
    time::{Duration, Instant},
};

/// A wrapper around UnsafeCell that implements Send and Sync for thread-safe access
/// where each thread writes to its own dedicated slot.
pub(crate) struct ThreadSafeUnsafeCell<T>(UnsafeCell<T>);

unsafe impl<T: Send> Send for ThreadSafeUnsafeCell<T> {}
unsafe impl<T: Send> Sync for ThreadSafeUnsafeCell<T> {}

impl<T> ThreadSafeUnsafeCell<T> {
    #[inline(always)]
    pub(crate) fn new(value: T) -> Self {
        Self(UnsafeCell::new(value))
    }

    #[inline(always)]
    pub(crate) fn get(&self) -> *mut T {
        self.0.get()
    }
}

/// Comprehensive timing statistics for the SynchedMemory instance.
#[derive(Clone)]
pub struct Timer {
    /// Number of threads using this timer.
    pub(crate) num_threads: usize,

    /// SynchedMemory initialization timestamp.
    pub(crate) init_timestamp: Instant,

    /// SynchedMemory completion timestamp.
    pub(crate) completion_timestamp: Arc<ThreadSafeUnsafeCell<Instant>>,

    /// Timestamps for the starting time of current block's computation.
    /// Each thread writes to its own slot: computation_start_time_for_each_block\[thread_id\]
    /// Gets overwritten for each new block.
    pub(crate) computation_start_time_for_each_block: Arc<ThreadSafeUnsafeCell<Vec<Instant>>>,

    /// Timestamps for the end time of current block's computation.
    /// Each thread writes to its own slot: computation_end_time_for_each_block\[thread_id\]
    /// Gets overwritten for each new block.
    pub(crate) computation_end_time_for_each_block: Arc<ThreadSafeUnsafeCell<Vec<Instant>>>,

    /// Computation time for each thread and each block
    /// Each thread writes to its own vector: computation_time_for_each_thread_and_block\[thread_id\]\[block_id\]
    pub(crate) computation_time_for_each_thread_and_block:
        Arc<ThreadSafeUnsafeCell<Vec<Vec<Duration>>>>,
}

// SAFETY: Timer is safe to share between threads because:
// 1. Each thread only writes to its own dedicated slots (no data races)
// 2. The ThreadSafeUnsafeCell wrapper properly implements Send + Sync
// 3. Only one thread accesses each thread_id slot at a time
unsafe impl Send for Timer {}
unsafe impl Sync for Timer {}

impl Timer {
    /// Create a new Timer instance with the current time as the initialization timestamp.
    #[inline]
    pub fn new(num_threads: usize) -> Self {
        let init_timestamp = Instant::now();
        Self {
            num_threads,
            init_timestamp,
            completion_timestamp: Arc::new(ThreadSafeUnsafeCell::new(Instant::now())),
            computation_start_time_for_each_block: Arc::new(ThreadSafeUnsafeCell::new(vec![
                Instant::now();
                num_threads
            ])),
            computation_end_time_for_each_block: Arc::new(ThreadSafeUnsafeCell::new(vec![
                Instant::now();
                num_threads
            ])),
            computation_time_for_each_thread_and_block: Arc::new(ThreadSafeUnsafeCell::new(vec![
                vec![];
                num_threads
            ])),
        }
    }

    /// Record the start time for a specific thread's computation block.
    /// Each thread should call this when it starts processing a new block.
    #[inline]
    pub fn start_computation(&self, thread_id: usize) {
        assert!(thread_id < self.num_threads, "Thread ID out of bounds");

        unsafe {
            let start_times = &mut *self.computation_start_time_for_each_block.get();
            start_times[thread_id] = Instant::now();
        }
    }

    /// Record the end time for a specific thread's computation block and store the duration.
    /// Each thread should call this when it finishes processing a block.
    #[inline]
    pub fn end_computation(&self, thread_id: usize) {
        assert!(thread_id < self.num_threads, "Thread ID out of bounds");

        let end_time = Instant::now();

        unsafe {
            let start_times = &*self.computation_start_time_for_each_block.get();
            let end_times = &mut *self.computation_end_time_for_each_block.get();
            let comp_times = &mut *self.computation_time_for_each_thread_and_block.get();

            let start_time = start_times[thread_id];
            let computation_duration = end_time.duration_since(start_time);

            end_times[thread_id] = end_time;
            comp_times[thread_id].push(computation_duration);
        }
    }

    /// Get timing statistics for analysis and plotting.
    #[inline]
    pub fn get_timing_stats(&self) -> TimingStats {
        unsafe {
            let computation_times =
                (*self.computation_time_for_each_thread_and_block.get()).clone();
            let completion_timestamp = *self.completion_timestamp.get();

            TimingStats {
                init_timestamp: self.init_timestamp,
                completion_timestamp,
                computation_times_per_thread: computation_times,
            }
        }
    }

    /// Finalize the timer by recording the completion timestamp and final computation times.
    /// This should be called after all threads have completed their processing.
    #[inline]
    pub fn finalize(&self) {
        let now = Instant::now();

        // Record completion timestamp
        unsafe {
            let completion_time = &mut *self.completion_timestamp.get();
            *completion_time = now;
        }

        // Record final computation time for each thread
        unsafe {
            let start_times = &mut *self.computation_start_time_for_each_block.get();
            let comp_times = &mut *self.computation_time_for_each_thread_and_block.get();

            for thread_id in 0..self.num_threads {
                let start_time = start_times[thread_id];
                let computation_time = now.duration_since(start_time);
                comp_times[thread_id].push(computation_time);

                // Update start time to now for consistency
                start_times[thread_id] = now;
            }
        }
    }
}

/// Comprehensive timing statistics for the SynchedMemory instance.
#[derive(Debug, Clone)]
pub struct TimingStats {
    /// SynchedMemory initialization timestamp
    pub init_timestamp: Instant,
    /// SynchedMemory completion timestamp
    pub completion_timestamp: Instant,
    /// Computation times organized by \[thread_id\]\[block_id\]
    pub computation_times_per_thread: Vec<Vec<Duration>>,
}

impl TimingStats {
    /// Calculate the total runtime from init to completion.
    #[inline]
    pub fn total_runtime(&self) -> Duration {
        self.completion_timestamp
            .duration_since(self.init_timestamp)
    }

    /// Calculate the total computation time for a specific thread across all blocks.
    #[inline]
    pub fn thread_total_computation_time(&self, thread_id: usize) -> Option<Duration> {
        let thread_times = self.computation_times_per_thread.get(thread_id)?;
        let total = thread_times.iter().sum();
        Some(total)
    }

    /// Get the number of blocks processed by a specific thread.
    #[inline]
    pub fn thread_block_count(&self, thread_id: usize) -> usize {
        self.computation_times_per_thread
            .get(thread_id)
            .map(|times| times.len())
            .unwrap_or(0)
    }

    /// Get the average computation time per block for a specific thread.
    #[inline]
    pub fn thread_average_computation_time(&self, thread_id: usize) -> Option<Duration> {
        let thread_times = self.computation_times_per_thread.get(thread_id)?;
        if thread_times.is_empty() {
            return None;
        }
        let total: Duration = thread_times.iter().sum();
        Some(total / thread_times.len() as u32)
    }

    /// Prints a formatted table showing computation times per thread and block.
    ///
    /// Table format:
    /// - Rows: Block IDs (0, 1, 2, ...)
    /// - Columns: Thread IDs (0, 1, 2, ...)
    /// - Data: Computation time in milliseconds with microsecond precision
    ///
    /// The last three rows show aggregate statistics:
    /// - Total Computation Time: Sum of all block computation times per thread
    /// - Total Time: Total runtime from init to completion (same for all threads)
    /// - Computation Ratio: Total computation time / Total time (efficiency ratio)
    pub fn plot(&self) {
        // Find the maximum number of blocks across all threads
        let max_blocks = self
            .computation_times_per_thread
            .iter()
            .map(|thread_times| thread_times.len())
            .max()
            .unwrap_or(0);

        let num_threads = self.computation_times_per_thread.len();

        if max_blocks == 0 || num_threads == 0 {
            println!("No timing data available to plot.");
            return;
        }

        // Calculate total runtime
        let total_runtime = self.total_runtime();

        // Print title
        println!("\n📊 COMPUTATION TIME TABLE");
        println!("Time format: milliseconds (ms) with microsecond precision");
        println!("Total Runtime: {:.3} ms", duration_to_ms(total_runtime));
        println!("{}", "=".repeat(80));

        // Print header row (Thread IDs)
        print!("{:<12}", "Block");
        for thread_id in 0..num_threads {
            print!(" {:<12}", format!("Thread {}", thread_id));
        }
        println!();

        // Print separator
        print!("{}", "-".repeat(12));
        for _ in 0..num_threads {
            print!(" {}", "-".repeat(12));
        }
        println!();

        // Print data rows (one per block)
        for block_id in 0..max_blocks {
            print!("{:<12}", block_id);

            for thread_id in 0..num_threads {
                let thread_times = &self.computation_times_per_thread[thread_id];
                if block_id < thread_times.len() {
                    let time_ms = duration_to_ms(thread_times[block_id]);
                    print!(" {:<12.3}", time_ms);
                } else {
                    print!(" {:<12}", "—");
                }
            }
            println!();
        }

        // Print separator before summary rows
        println!("{}", "=".repeat(12 + num_threads * 13));

        // Summary rows - now showing data for each thread

        // Total Computation Time row
        print!("{:<12}", "Total Comp");
        for thread_id in 0..num_threads {
            if let Some(total_comp) = self.thread_total_computation_time(thread_id) {
                let time_ms = duration_to_ms(total_comp);
                print!(" {:<12.3}", time_ms);
            } else {
                print!(" {:<12}", "—");
            }
        }
        println!();

        // Total Time row (same for all threads)
        print!("{:<12}", "Total Time");
        for _thread_id in 0..num_threads {
            let time_ms = duration_to_ms(total_runtime);
            print!(" {:<12.3}", time_ms);
        }
        println!();

        // Computation Ratio row
        print!("{:<12}", "Comp Ratio%");
        for thread_id in 0..num_threads {
            if let Some(comp_time) = self.thread_total_computation_time(thread_id) {
                if total_runtime.as_nanos() > 0 {
                    let ratio = comp_time.as_nanos() as f64 / total_runtime.as_nanos() as f64;
                    print!(" {:<12.1}", ratio * 100.0);
                } else {
                    print!(" {:<12}", "0.0");
                }
            } else {
                print!(" {:<12}", "—");
            }
        }
        println!();

        println!("{}", "=".repeat(80));

        // Print legend
        println!("\nLegend:");
        println!("  • Total Comp: Total computation time per thread (ms)");
        println!("  • Total Time: Total runtime from init to completion (ms)");
        println!("  • Comp Ratio: Computation time as percentage of total runtime (%)");
        println!("  • —: No data available for this block");
    }
}

/// Convert Duration to milliseconds as f64
#[inline]
fn duration_to_ms(duration: Duration) -> f64 {
    duration.as_nanos() as f64 / 1_000_000.0
}
