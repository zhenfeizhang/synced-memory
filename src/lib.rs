//! # SynchedMemory - Lock-Free Thread-Synchronized Memory
//!
//! A high-performance, lock-free data structure for coordinating memory access between threads
//! in a write-then-read pattern. Each thread can write to its own segment of memory, and all
//! threads can read the global state once everyone has completed their writes.
//!
//! ## Key Features
//!
//! - **Lock-Free**: Uses atomic operations and barriers instead of mutexes
//! - **Segmented Access**: Each thread has exclusive access to its own memory segment
//! - **Automatic Synchronization**: Built-in barriers coordinate write-then-read cycles
//! - **Memory Safety**: Prevents data races through careful design and `UnsafeCell`
//! - **High Performance**: Optimized for scenarios with many threads doing coordinated work
//! - **Optional Profiling**: Feature-gated timing analytics with detailed performance analysis
//!
//! ## Architecture
//!
//! ```text
//! Thread 0: [data...] | Thread 1: [data...] | Thread 2: [data...] | Thread N: [data...]
//!           ^                      ^                      ^                      ^
//!           |                      |                      |                      |
//!  LocalMemHandler(0)     LocalMemHandler(1)     LocalMemHandler(2)     LocalMemHandler(N)
//! ```
//!
//! ### Memory Layout
//! - **Total Memory**: `num_threads × max_size_per_thread × size_of::<T>()`
//! - **Segmented Access**: Each thread writes exclusively to its own segment
//! - **Cache Friendly**: Non-overlapping memory regions prevent false sharing
//!
//! ### Synchronization Protocol
//! 1. Each thread writes to its own segment (parallel, no contention)
//! 2. All threads wait at write barrier (coordination point)
//! 3. Global state becomes available for reading (consistent view)
//! 4. Threads process global state and repeat
//!
//! ## Basic Usage Pattern
//!
//! ```rust,no_run
//! use synched_memory::SynchedMemory;
//! use std::sync::Arc;
//! use std::thread;
//!
//! // Create synchronized memory for 4 threads, 10 elements per thread
//! let memory = Arc::new(SynchedMemory::<i32>::new(4, 10));
//!
//! let handles: Vec<_> = (0..4).map(|thread_id| {
//!     let mem = Arc::clone(&memory);
//!     thread::spawn(move || {
//!         // Each thread gets its own local memory handler
//!         let local_handler = mem.build_local_handler(thread_id);
//!         
//!         // Write data and automatically sync with other threads
//!         let data = vec![thread_id as i32; 5];
//!         let global_data = mem.write_local_memory_and_sync_read(&local_handler, &data);
//!         
//!         println!("Thread {} sees {} total elements", thread_id, global_data.len());
//!     })
//! }).collect();
//!
//! for handle in handles {
//!     handle.join().unwrap();
//! }
//! ```
//!
//! ## Iterative Computation Pattern
//!
//! Perfect for simulations, iterative algorithms, and parallel processing:
//!
//! ```rust,no_run
//! use synched_memory::SynchedMemory;
//! use std::sync::Arc;
//! use std::thread;
//!
//! #[derive(Default, Clone, Copy)]
//! struct SimulationCell {
//!     state: f64,
//!     energy: f64,
//! }
//!
//! let simulation = Arc::new(SynchedMemory::<SimulationCell>::new(4, 100));
//!
//! let handles: Vec<_> = (0..4).map(|thread_id| {
//!     let sim = Arc::clone(&simulation);
//!     thread::spawn(move || {
//!         let local_handler = sim.build_local_handler(thread_id);
//!         let mut previous_state = Vec::new();
//!         
//!         for iteration in 0..1000 {
//!             // Compute local portion based on global state from previous iteration
//!             let local_cells = compute_simulation_step(thread_id, &previous_state);
//!             
//!             // Synchronize with all threads and get updated global state
//!             let global_state = sim.write_local_memory_and_sync_read(&local_handler, &local_cells);
//!             
//!             previous_state = global_state;
//!         }
//!     })
//! }).collect();
//!
//! for handle in handles {
//!     handle.join().unwrap();
//! }
//!
//! fn compute_simulation_step(thread_id: usize, global_state: &[SimulationCell]) -> Vec<SimulationCell> {
//!     // Simulate computation based on global state
//!     vec![SimulationCell { state: thread_id as f64, energy: 1.0 }; 10]
//! }
//! ```
//!
//! ## Performance Profiling
//!
//! Enable detailed timing analysis with the `profiler` feature:
//!
//! ```toml
//! [dependencies]
//! synched-memory = { version = "*", features = ["profiler"] }
//! ```
//!
//! ```rust,no_run
//! use synched_memory::SynchedMemory;
//! use std::sync::Arc;
//!
//! let memory = Arc::new(SynchedMemory::<f64>::new(4, 1000));
//!
//! // ... perform computation work ...
//!
//! // Finalize timing collection
//! memory.finalize();
//!
//! // Analyze performance (only available with profiler feature)
//! #[cfg(feature = "profiler")]
//! {
//!     let stats = memory.get_all_timing_stats();
//!     
//!     // Print detailed timing table
//!     stats.plot();
//!     
//!     // Access specific metrics
//!     println!("Total runtime: {:?}", stats.total_runtime());
//!     println!("Thread 0 total computation: {:?}", stats.thread_total_computation_time(0));
//! }
//! ```
//!
//! ## Thread Safety
//!
//! - Each thread only writes to its own segment (no contention)
//! - Barrier-based synchronization ensures proper write-then-read ordering
//! - Built-in validation prevents out-of-bounds access
//! - `UnsafeCell` provides safe interior mutability with external coordination
//!
//! ## Performance Characteristics
//!
//! - **Write Complexity**: O(1) per thread (no contention)
//! - **Read Complexity**: O(n) where n = total elements (unavoidable for global view)
//! - **Memory Usage**: Fixed allocation of `num_threads × segment_size × sizeof(T)`
//! - **Cache Efficiency**: Each thread works on separate cache lines
//! - **Zero Allocation**: No dynamic memory allocation during operation
//!
//! ## Use Cases
//!
//! - **Iterative Simulations**: Game of Life, physics simulations, cellular automata
//! - **Parallel Algorithms**: Map-reduce patterns, distributed computing
//! - **Real-Time Systems**: Audio processing, control systems, game engines
//! - **Scientific Computing**: Numerical methods, optimization algorithms
//! - **Data Processing**: Parallel data transformation with coordination points

#[cfg(test)]
mod tests;

#[cfg(feature = "profiler")]
mod timer;
#[cfg(feature = "profiler")]
use std::time::{Duration, Instant};
#[cfg(feature = "profiler")]
pub use timer::{Timer, TimingStats};

use std::{
    cell::UnsafeCell,
    sync::{Arc, Barrier},
};

/// A lock-free, thread-synchronized memory structure for coordinated data access.
///
/// `SynchedMemory` provides a high-performance way for multiple threads to coordinate
/// memory access in a write-then-read pattern. The memory is divided into segments,
/// with each thread having exclusive write access to its own segment.
///
/// ## Memory Layout
///
/// ```text
/// ┌─────────────┬─────────────┬─────────────┬─────────────┐
/// │  Thread 0   │  Thread 1   │  Thread 2   │  Thread N   │
/// │ Segment     │ Segment     │ Segment     │ Segment     │
/// │ [0..size)   │ [size..2*s) │ [2*s..3*s)  │ [N*s..(N+1)*s) │
/// └─────────────┴─────────────┴─────────────┴─────────────┘
/// ```
///
/// Where `s = max_size_per_thread` and `N = num_threads - 1`.
///
/// ## Synchronization Protocol
///
/// 1. **Write Phase**: Each thread writes to its exclusive segment
/// 2. **Barrier Phase**: All threads synchronize at write barrier
/// 3. **Read Phase**: Complete global state becomes available
/// 4. **Repeat**: Continue to next iteration with updated global state
///
/// ## Thread Requirements
///
/// - Each thread must call `build_local_handler()` exactly once with a unique thread ID
/// - Thread IDs must be in range `[0, num_threads)`
/// - All threads must participate in each synchronization cycle
///
/// ## Memory Requirements
///
/// - `T` must implement `Default + Clone + Copy`
/// - Total memory allocated: `num_threads × max_size_per_thread × size_of::<T>()`
/// - Memory is allocated once at construction and never reallocated
///
/// ## Performance Notes
///
/// - **Cache Friendly**: Each thread works on separate memory regions
/// - **NUMA Aware**: Consider thread affinity for optimal performance
/// - **Lock-Free**: No mutex contention, only barrier synchronization
/// - **Predictable**: Fixed memory layout and deterministic timing
///
/// ## Examples
///
/// ### Basic Coordination
///
/// ```rust
/// use synched_memory::SynchedMemory;
/// use std::sync::Arc;
/// use std::thread;
///
/// let memory = Arc::new(SynchedMemory::<i32>::new(2, 10));
///
/// let handles: Vec<_> = (0..2).map(|thread_id| {
///     let mem = Arc::clone(&memory);
///     thread::spawn(move || {
///         let local_handler = mem.build_local_handler(thread_id);
///         let data = vec![thread_id as i32; 5];
///         let global_data = mem.write_local_memory_and_sync_read(&local_handler, &data);
///         
///         // Verify data integrity
///         assert_eq!(global_data.len(), 20); // 2 threads × 10 elements each
///         println!("Thread {} synchronized successfully", thread_id);
///     })
/// }).collect();
///
/// for handle in handles {
///     handle.join().unwrap();
/// }
/// ```
///
/// ### Performance Monitoring
///
/// ```rust,no_run
/// use std::sync::Arc;
///
/// use synched_memory::SynchedMemory;
/// #[cfg(feature = "profiler")]
/// use synched_memory::TimingStats;
///
/// let memory = Arc::new(SynchedMemory::<f64>::new(4, 1000));
///
/// // ... perform coordinated computation ...
///
/// memory.finalize();
///
/// #[cfg(feature = "profiler")]
/// {
///     let stats = memory.get_all_timing_stats();
///     stats.plot(); // Detailed timing analysis
///     
///     // Check for load imbalance
///     for thread_id in 0..4 {
///         if let Some(compute_time) = stats.thread_total_computation_time(thread_id) {
///             println!("Thread {}: {:?} total computation", thread_id, compute_time);
///         }
///     }
/// }
/// ```
pub struct SynchedMemory<T> {
    /// The shared memory buffer, divided into per-thread segments.
    /// Uses `UnsafeCell` to allow interior mutability without runtime checks.
    pub(crate) data: Arc<UnsafeCell<Vec<T>>>,

    /// Base pointer to the data - computed once at initialization to avoid data races
    pub(crate) base_ptr: *mut T,

    /// Number of threads that will access this memory.
    pub(crate) num_threads: usize,

    /// Maximum number of elements each thread can write to its segment.
    pub(crate) max_size_per_thread: usize,

    /// Barrier to ensure all threads finish writing before any can read.
    /// This is the key synchronization primitive for the write-then-read pattern.
    write_barrier: Arc<Barrier>,

    /// Optional timing profiler for performance analysis.
    #[cfg(feature = "profiler")]
    timer: Timer,
}

// SAFETY: SynchedMemory is safe to share between threads because:
// 1. Each thread only writes to its own memory segment (no data races)
// 2. Each thread only writes to its own timing data slots (no data races)
// 3. All synchronization is handled through atomic operations and barriers
// 4. The UnsafeCell access is carefully controlled to prevent concurrent mutation
unsafe impl<T: Send> Send for SynchedMemory<T> {}
unsafe impl<T: Send> Sync for SynchedMemory<T> {}

/// A handle representing a thread's exclusive access to its segment of shared memory.
///
/// `LocalMemHandler` provides a thread with direct access to its portion of the
/// shared memory buffer. This allows for high-performance writes without
/// any synchronization overhead during the write operation itself.
///
/// ## Safety
///
/// The raw pointer is safe to use because:
/// - Each thread gets a unique, non-overlapping memory segment
/// - The pointer remains valid for the lifetime of the `SynchedMemory`
/// - Write access is coordinated through the barrier system
/// - Bounds checking prevents out-of-range access
///
/// ## Thread Ownership
///
/// Each `LocalMemHandler` is tied to a specific thread ID and should not be
/// shared between threads. The thread ID is validated on each operation.
///
/// ## Examples
///
/// ```rust
/// use synched_memory::SynchedMemory;
///
/// let memory = SynchedMemory::<i32>::new(4, 10);
/// let local_handler = memory.build_local_handler(0); // Thread 0's segment
///
/// // This thread can now write to elements [0..10) exclusively
/// let data = vec![42; 5];
/// memory.write_local_memory(&local_handler, &data);
/// ```
pub struct LocalMemHandler<T> {
    /// Raw pointer to the start of this thread's memory segment.
    /// Safe to use because each thread gets a unique, non-overlapping segment.
    pub(crate) data_ptr: *mut T,

    /// The thread ID this local memory handler belongs to.
    /// Used for validation and debugging.
    pub(crate) thread_id: usize,
}

// SAFETY: LocalMemHandler is safe to send/sync between threads because:
// 1. Each LocalMemHandler points to a unique memory segment
// 2. The raw pointer is only used for writing to that thread's exclusive segment
// 3. Synchronization is handled at the SynchedMemory level
unsafe impl<T> Send for LocalMemHandler<T> where T: Send {}
unsafe impl<T> Sync for LocalMemHandler<T> where T: Sync {}

impl<T> SynchedMemory<T>
where
    T: Default + Clone + Copy,
{
    /// Creates a new `SynchedMemory` instance with the specified configuration.
    ///
    /// # Arguments
    ///
    /// * `num_threads` - The number of threads that will access this memory (must be > 0)
    /// * `max_size_per_thread` - Maximum elements each thread can write to its segment
    ///
    /// # Returns
    ///
    /// A new `SynchedMemory` instance ready for multi-threaded access.
    ///
    /// # Memory Allocation
    ///
    /// Total memory allocated = `num_threads × max_size_per_thread × size_of::<T>()`
    ///
    /// The memory is allocated once at construction and never reallocated during operation.
    ///
    /// # Panics
    ///
    /// Panics if `num_threads` is 0, as barriers require at least one thread.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use synched_memory::SynchedMemory;
    ///
    /// // Create memory for 4 threads, 100 elements each
    /// let memory = SynchedMemory::<i32>::new(4, 100);
    /// // Total memory: 4 × 100 × 4 bytes = 1,600 bytes
    ///
    /// // For larger data types
    /// let memory = SynchedMemory::<[f64; 8]>::new(8, 1000);
    /// // Total memory: 8 × 1000 × 64 bytes = 512,000 bytes
    /// ```
    #[inline]
    pub fn new(num_threads: usize, max_size_per_thread: usize) -> Self {
        assert!(num_threads > 0, "Number of threads must be greater than 0");

        let total_size = num_threads * max_size_per_thread;
        let mut data_vec = vec![T::default(); total_size];
        let base_ptr = data_vec.as_mut_ptr(); // Get the base pointer once
        let data = Arc::new(UnsafeCell::new(data_vec));
        let write_barrier = Arc::new(Barrier::new(num_threads));

        #[cfg(feature = "profiler")]
        let timer = Timer::new(num_threads);

        SynchedMemory {
            data,
            base_ptr,
            num_threads,
            max_size_per_thread,
            write_barrier,

            #[cfg(feature = "profiler")]
            timer,
        }
    }

    /// Returns the number of threads this memory instance supports.
    ///
    /// This value is set at construction and cannot be changed.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use synched_memory::SynchedMemory;
    ///
    /// let memory = SynchedMemory::<i32>::new(8, 50);
    /// assert_eq!(memory.num_threads(), 8);
    /// ```
    #[inline]
    pub fn num_threads(&self) -> usize {
        self.num_threads
    }

    /// Returns the total length of the underlying data vector.
    ///
    /// This equals `num_threads() × max_size_per_thread`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use synched_memory::SynchedMemory;
    ///
    /// let memory = SynchedMemory::<i32>::new(4, 25);
    /// assert_eq!(memory.data_len(), 100); // 4 × 25
    /// ```
    #[inline]
    pub fn data_len(&self) -> usize {
        unsafe { (*self.data.get()).len() }
    }

    /// Reads the complete synchronized memory state.
    ///
    /// This method returns a copy of the entire synchronized memory state.
    /// It should typically be called after all threads have synchronized
    /// (e.g., after `write_local_memory_and_sync_read()`).
    ///
    /// # Returns
    ///
    /// A complete copy of the synchronized memory state containing all thread segments.
    ///
    /// # Thread Safety
    ///
    /// This method is safe to call from any thread at any time, but the returned
    /// data is only guaranteed to be consistent after a synchronization point.
    ///
    /// # Performance
    ///
    /// This method clones the entire data vector, which has O(n) time complexity
    /// where n is the total number of elements. For large datasets, consider
    /// the memory and performance implications.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use synched_memory::SynchedMemory;
    /// use std::sync::Arc;
    ///
    /// let memory = Arc::new(SynchedMemory::<i32>::new(2, 10));
    /// // ... after synchronization ...
    /// let global_data = memory.read_global();
    /// println!("Global data: {} elements", global_data.len());
    /// ```
    #[inline]
    pub fn read_global(&self) -> Vec<T> {
        unsafe {
            let data = &*self.data.get();
            data.clone()
        }
    }

    /// Creates a `LocalMemHandler` for the specified thread.
    ///
    /// This handle provides exclusive write access to the thread's memory segment.
    /// Each thread must call this method exactly once with its unique thread ID.
    ///
    /// # Arguments
    ///
    /// * `thread_id` - Unique identifier for the thread (must be in range `[0, num_threads)`)
    ///
    /// # Returns
    ///
    /// A `LocalMemHandler` containing a pointer to the thread's exclusive memory segment.
    ///
    /// # Memory Layout
    ///
    /// Thread N's segment occupies indices `[N × max_size_per_thread, (N+1) × max_size_per_thread)`
    /// in the global data vector.
    ///
    /// # Side Effects
    ///
    /// When the `profiler` feature is enabled, this method records the computation
    /// start time for the thread.
    ///
    /// # Panics
    ///
    /// Panics if `thread_id >= num_threads()`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use synched_memory::SynchedMemory;
    ///
    /// let memory = SynchedMemory::<i32>::new(4, 10);
    ///
    /// // Each thread gets its own local memory handler
    /// let handler_0 = memory.build_local_handler(0); // Elements [0, 10)
    /// let handler_1 = memory.build_local_handler(1); // Elements [10, 20)
    /// let handler_2 = memory.build_local_handler(2); // Elements [20, 30)
    /// let handler_3 = memory.build_local_handler(3); // Elements [30, 40)
    /// ```
    #[inline]
    pub fn build_local_handler(&self, thread_id: usize) -> LocalMemHandler<T> {
        if thread_id >= self.num_threads() {
            panic!(
                "Thread ID {} out of bounds (max: {})",
                thread_id,
                self.num_threads() - 1
            );
        }

        let start_index = thread_id * self.max_size_per_thread;
        // Use the pre-computed base pointer instead of accessing through Vec
        let data_ptr = unsafe { self.base_ptr.add(start_index) };

        #[cfg(feature = "profiler")]
        unsafe {
            let start_times = &mut *self.timer.computation_start_time_for_each_block.get();
            start_times[thread_id] = Instant::now();
        }

        LocalMemHandler {
            data_ptr,
            thread_id,
        }
    }

    /// Writes data to a thread's local segment and synchronizes with other threads.
    ///
    /// This is the primary method for coordinated memory access. It performs the following operations:
    ///
    /// 1. **Timing** (profiler feature): Records computation end time and duration
    /// 2. **Write**: Writes data to the thread's local segment
    /// 3. **Barrier**: Waits for ALL threads to finish writing
    /// 4. **Read**: Reads and returns the complete synchronized global state
    /// 5. **Timing** (profiler feature): Records new computation start time
    ///
    /// # Arguments
    ///
    /// * `local_handler` - The thread's local memory handler (from `build_local_handler()`)
    /// * `data` - The data to write to the local segment (must fit in segment)
    ///
    /// # Returns
    ///
    /// The complete synchronized memory state after all threads have written their data.
    /// This vector contains `num_threads × max_size_per_thread` elements.
    ///
    /// # Synchronization Behavior
    ///
    /// This method implements the core write-then-read pattern:
    /// - **Write Phase**: Thread writes its data (no waiting)
    /// - **Barrier Phase**: All threads synchronize (blocking until all threads arrive)
    /// - **Read Phase**: Thread reads the complete global state (consistent view)
    ///
    /// # Performance Notes
    ///
    /// - Write operation is O(data.len()) per thread
    /// - Barrier synchronization blocks until slowest thread arrives
    /// - Read operation clones entire global state O(total_elements)
    /// - With profiler enabled, timing overhead is minimal (~few nanoseconds)
    ///
    /// # Panics
    ///
    /// - If `data.len() > max_size_per_thread`
    /// - If the local handler's thread ID is invalid
    ///
    /// # Examples
    ///
    /// ## Basic Usage
    /// ```rust
    /// use synched_memory::SynchedMemory;
    /// use std::sync::Arc;
    /// use std::thread;
    ///
    /// let memory = Arc::new(SynchedMemory::<i32>::new(2, 5));
    ///
    /// let handles: Vec<_> = (0..2).map(|thread_id| {
    ///     let mem = Arc::clone(&memory);
    ///     thread::spawn(move || {
    ///         let local_handler = mem.build_local_handler(thread_id);
    ///         let data = vec![thread_id as i32; 3];
    ///         
    ///         // This call blocks until both threads have written
    ///         let global_data = mem.write_local_memory_and_sync_read(&local_handler, &data);
    ///         println!("Thread {} sees global data: {} elements", thread_id, global_data.len());
    ///     })
    /// }).collect();
    ///
    /// for handle in handles {
    ///     handle.join().unwrap();
    /// }
    /// ```
    ///
    /// ## Iterative Processing
    /// ```rust,no_run
    /// use synched_memory::SynchedMemory;
    /// use std::sync::Arc;
    ///
    /// let memory = Arc::new(SynchedMemory::<f64>::new(4, 100));
    /// let local_handler = memory.build_local_handler(0); // Thread 0
    /// let mut global_state = Vec::new();
    ///
    /// for iteration in 0..1000 {
    ///     // Compute based on previous global state
    ///     let new_data = compute_iteration(&global_state, iteration);
    ///     
    ///     // Synchronize and get updated global state
    ///     global_state = memory.write_local_memory_and_sync_read(&local_handler, &new_data);
    /// }
    ///
    /// fn compute_iteration(global: &[f64], iter: usize) -> Vec<f64> {
    ///     vec![iter as f64; 50] // Compute 50 elements
    /// }
    /// ```
    #[inline]
    pub fn write_local_memory_and_sync_read(
        &self,
        local_handler: &LocalMemHandler<T>,
        data: &[T],
    ) -> Vec<T> {
        #[cfg(feature = "profiler")]
        let thread_id = local_handler.thread_id;

        // Record computation timing (profiler feature only)
        #[cfg(feature = "profiler")]
        {
            let computation_end_time = Instant::now();
            unsafe {
                let end_times = &mut *self.timer.computation_end_time_for_each_block.get();
                end_times[thread_id] = computation_end_time;
            }

            // Calculate and store computation time for this block
            unsafe {
                let start_times = &*self.timer.computation_start_time_for_each_block.get();
                let comp_times = &mut *self.timer.computation_time_for_each_thread_and_block.get();

                let computation_time = computation_end_time.duration_since(start_times[thread_id]);
                comp_times[thread_id].push(computation_time);
            }
        }

        // Write the data to the local segment
        self.write_local_memory(local_handler, data);

        // Wait for ALL threads to finish writing (key synchronization point)
        self.write_barrier.wait();

        // Read the global state after synchronization
        let global_data = self.read_global();

        // Record new computation start time for the next block (profiler feature only)
        #[cfg(feature = "profiler")]
        unsafe {
            let start_times = &mut *self.timer.computation_start_time_for_each_block.get();
            start_times[thread_id] = Instant::now();
        }

        global_data
    }

    /// Provides manual access to the write barrier for advanced use cases.
    ///
    /// Most users should use `write_local_memory_and_sync_read()` instead, which
    /// handles barrier synchronization automatically. This method is provided
    /// for advanced users who need fine-grained control over the synchronization.
    ///
    /// # Behavior
    ///
    /// This method blocks the calling thread until all `num_threads` threads
    /// have called either this method or `write_local_memory_and_sync_read()`.
    ///
    /// # Usage Pattern
    ///
    /// ```rust,no_run
    /// use synched_memory::SynchedMemory;
    ///
    /// let memory = SynchedMemory::<i32>::new(4, 10);
    /// let local_handler = memory.build_local_handler(0);
    ///
    /// // Manual synchronization (not recommended for typical use)
    /// memory.write_local_memory(&local_handler, &[1, 2, 3]);
    /// memory.wait_write_barrier(); // Wait for all threads
    /// let global_data = memory.read_global();
    /// ```
    ///
    /// # Deadlock Warning
    ///
    /// All threads must eventually call this method (or `write_local_memory_and_sync_read()`)
    /// for the barrier to release. Failure to do so will result in deadlock.
    #[inline]
    pub fn wait_write_barrier(&self) {
        self.write_barrier.wait();
    }

    /// Low-level method to write data to a thread's local segment.
    ///
    /// This method performs the actual memory write operation but does NOT handle
    /// synchronization. For coordinated access, use `write_local_memory_and_sync_read()`
    /// instead.
    ///
    /// # Operation Details
    ///
    /// 1. **Validation**: Checks data length and thread ID bounds
    /// 2. **Clear**: Fills the entire local segment with `T::default()` values
    /// 3. **Write**: Copies the new data to the beginning of the segment
    ///
    /// # Arguments
    ///
    /// * `local_handler` - The thread's local memory handler
    /// * `data` - The data to write (must fit in the local segment)
    ///
    /// # Memory Layout After Write
    ///
    /// ```text
    /// Thread's Segment: [data[0], data[1], ..., data[n-1], default, default, ...]
    ///                   |<----- data.len() ---->|<-- remaining space -->|
    ///                   |<------------- max_size_per_thread ------------->|
    /// ```
    ///
    /// # Safety
    ///
    /// This method uses unsafe code to write directly to memory, but is safe because:
    /// - Each thread writes only to its own segment (no overlap)
    /// - Segment boundaries are validated
    /// - Thread ID bounds are checked
    ///
    /// # Panics
    ///
    /// - If `data.len() > max_size_per_thread`
    /// - If the local handler's thread ID is >= `num_threads()`
    ///
    /// # Examples
    ///
    /// ```rust
    /// use synched_memory::SynchedMemory;
    ///
    /// let memory = SynchedMemory::<i32>::new(2, 10);
    /// let local_handler = memory.build_local_handler(0);
    ///
    /// // Low-level write (no synchronization)
    /// memory.write_local_memory(&local_handler, &[1, 2, 3, 4, 5]);
    ///
    /// // Must manually coordinate with other threads for consistent reads
    /// ```
    #[inline]
    pub fn write_local_memory(&self, local_handler: &LocalMemHandler<T>, data: &[T]) {
        // Validate input parameters
        if data.len() > self.max_size_per_thread {
            panic!(
                "Data length {} exceeds maximum size per thread {}",
                data.len(),
                self.max_size_per_thread
            );
        }

        if local_handler.thread_id >= self.num_threads() {
            panic!(
                "Thread ID {} out of bounds (max: {})",
                local_handler.thread_id,
                self.num_threads() - 1
            );
        }

        // Most optimized version - if you KNOW your T::default() is all-zeros:
        unsafe {
            // Fast memset-style zeroing (only use if T::default() == all-zeros)
            if std::mem::size_of::<T>() > 0 {
                // write_bytes writes `max_size_per_thread * size_of::<T>()` bytes
                std::ptr::write_bytes(local_handler.data_ptr, 0u8, self.max_size_per_thread);
            }

            // Write the new data to the segment
            std::ptr::copy_nonoverlapping(data.as_ptr(), local_handler.data_ptr, data.len());
        }
    }

    /// Finalizes the SynchedMemory by recording completion timestamp and final computation times.
    ///
    /// This method should be called after all threads have completed their processing.
    /// It is primarily used with the `profiler` feature to record final timing data.
    ///
    /// # Behavior
    ///
    /// - **Without profiler**: This method does nothing (zero overhead)
    /// - **With profiler**: Records completion timestamp and final computation durations
    ///
    /// # Usage
    ///
    /// Call this method from any thread after all computation threads have joined:
    ///
    /// ```rust,no_run
    /// use synched_memory::SynchedMemory;
    /// use std::sync::Arc;
    /// use std::thread;
    ///
    /// let memory = Arc::new(SynchedMemory::<i32>::new(4, 100));
    ///
    /// // Spawn computation threads
    /// let handles: Vec<_> = (0..4).map(|thread_id| {
    ///     let mem = Arc::clone(&memory);
    ///     thread::spawn(move || {
    ///         // ... computation work ...
    ///     })
    /// }).collect();
    ///
    /// // Wait for all threads to complete
    /// for handle in handles {
    ///     handle.join().unwrap();
    /// }
    ///
    /// // Finalize timing data
    /// memory.finalize();
    ///
    /// #[cfg(feature = "profiler")]
    /// {
    ///     let stats = memory.get_all_timing_stats();
    ///     stats.plot();
    /// }
    /// ```
    #[inline]
    pub fn finalize(&self) {
        #[cfg(feature = "profiler")]
        self.timer.finalize();
    }

    /// Gets the initialization timestamp.
    ///
    /// Returns the `Instant` when this `SynchedMemory` instance was created.
    /// Only available when the `profiler` feature is enabled.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// #[cfg(feature = "profiler")]
    /// use synched_memory::SynchedMemory;
    ///
    /// #[cfg(feature = "profiler")]
    /// {
    ///     let memory = SynchedMemory::<i32>::new(4, 100);
    ///     let init_time = memory.get_init_timestamp();
    ///     println!("Memory initialized at: {:?}", init_time);
    /// }
    /// ```
    #[inline]
    #[cfg(feature = "profiler")]
    pub fn get_init_timestamp(&self) -> Instant {
        self.timer.init_timestamp
    }

    /// Gets the completion timestamp.
    ///
    /// Returns the `Instant` when `finalize()` was called.
    /// Only available when the `profiler` feature is enabled.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// #[cfg(feature = "profiler")]
    /// use synched_memory::SynchedMemory;
    ///
    /// #[cfg(feature = "profiler")]
    /// {
    ///     let memory = SynchedMemory::<i32>::new(4, 100);
    ///     // ... do work ...
    ///     memory.finalize();
    ///     let completion_time = memory.get_completion_timestamp();
    ///     println!("Memory finalized at: {:?}", completion_time);
    /// }
    /// ```
    #[inline]
    #[cfg(feature = "profiler")]
    pub fn get_completion_timestamp(&self) -> Instant {
        unsafe { *self.timer.completion_timestamp.get() }
    }

    /// Gets the current computation start time for a specific thread.
    ///
    /// Returns the `Instant` when the thread last started a computation block.
    /// Only available when the `profiler` feature is enabled.
    ///
    /// # Arguments
    ///
    /// * `thread_id` - The thread ID to query (must be < `num_threads()`)
    ///
    /// # Returns
    ///
    /// `Some(Instant)` if thread_id is valid, `None` otherwise.
    #[inline]
    #[cfg(feature = "profiler")]
    pub fn get_computation_start_time(&self, thread_id: usize) -> Option<Instant> {
        if thread_id >= self.num_threads() {
            return None;
        }
        unsafe {
            let start_times = &*self.timer.computation_start_time_for_each_block.get();
            Some(start_times[thread_id])
        }
    }

    /// Gets the current computation end time for a specific thread.
    ///
    /// Returns the `Instant` when the thread last finished a computation block.
    /// Only available when the `profiler` feature is enabled.
    ///
    /// # Arguments
    ///
    /// * `thread_id` - The thread ID to query (must be < `num_threads()`)
    ///
    /// # Returns
    ///
    /// `Some(Instant)` if thread_id is valid, `None` otherwise.
    #[inline]
    #[cfg(feature = "profiler")]
    pub fn get_computation_end_time(&self, thread_id: usize) -> Option<Instant> {
        if thread_id >= self.num_threads() {
            return None;
        }
        unsafe {
            let end_times = &*self.timer.computation_end_time_for_each_block.get();
            Some(end_times[thread_id])
        }
    }

    /// Gets all computation times for a specific thread.
    ///
    /// Returns a vector of `Duration` values representing the computation time
    /// for each block processed by the specified thread. Only available when
    /// the `profiler` feature is enabled.
    ///
    /// # Arguments
    ///
    /// * `thread_id` - The thread ID to query (must be < `num_threads()`)
    ///
    /// # Returns
    ///
    /// A vector of computation durations, one for each block processed.
    /// Returns empty vector if thread_id is invalid.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// #[cfg(feature = "profiler")]
    /// use synched_memory::SynchedMemory;
    ///
    /// #[cfg(feature = "profiler")]
    /// {
    ///     let memory = SynchedMemory::<i32>::new(4, 100);
    ///     // ... after computation and finalization ...
    ///     
    ///     let thread_0_times = memory.get_thread_computation_times(0);
    ///     println!("Thread 0 processed {} blocks", thread_0_times.len());
    ///     for (block_id, duration) in thread_0_times.iter().enumerate() {
    ///         println!("Block {}: {:?}", block_id, duration);
    ///     }
    /// }
    /// ```
    #[inline]
    #[cfg(feature = "profiler")]
    pub fn get_thread_computation_times(&self, thread_id: usize) -> Vec<Duration> {
        if thread_id >= self.num_threads() {
            return Vec::new();
        }
        unsafe {
            let comp_times = &*self.timer.computation_time_for_each_thread_and_block.get();
            comp_times[thread_id].clone()
        }
    }

    /// Gets comprehensive timing statistics for all threads.
    ///
    /// Returns a `TimingStats` structure containing detailed timing information
    /// including initialization/completion timestamps and per-thread computation times.
    /// Only available when the `profiler` feature is enabled.
    ///
    /// # Returns
    ///
    /// A `TimingStats` instance with complete timing data for analysis.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// #[cfg(feature = "profiler")]
    /// use synched_memory::SynchedMemory;
    ///
    /// #[cfg(feature = "profiler")]
    /// {
    ///     let memory = SynchedMemory::<i32>::new(4, 100);
    ///     // ... after computation and finalization ...
    ///     
    ///     let stats = memory.get_all_timing_stats();
    ///     
    ///     // Print detailed timing table
    ///     stats.plot();
    ///     
    ///     // Access specific metrics
    ///     println!("Total runtime: {:?}", stats.total_runtime());
    ///     println!("Thread 0 computation time: {:?}", stats.thread_total_computation_time(0));
    /// }
    /// ```
    #[inline]
    #[cfg(feature = "profiler")]
    pub fn get_all_timing_stats(&self) -> TimingStats {
        unsafe {
            let computation_times =
                (*self.timer.computation_time_for_each_thread_and_block.get()).clone();
            let completion_timestamp = *self.timer.completion_timestamp.get();

            TimingStats {
                init_timestamp: self.timer.init_timestamp,
                completion_timestamp,
                computation_times_per_thread: computation_times,
            }
        }
    }

    /// Calculate total runtime from initialization to completion.
    ///
    /// Returns the duration between when this `SynchedMemory` was created
    /// and when `finalize()` was called. Only available when the `profiler`
    /// feature is enabled.
    ///
    /// # Returns
    ///
    /// Duration representing the total lifetime of this memory instance.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// #[cfg(feature = "profiler")]
    /// use synched_memory::SynchedMemory;
    ///
    /// #[cfg(feature = "profiler")]
    /// {
    ///     let memory = SynchedMemory::<i32>::new(4, 100);
    ///     // ... computation work ...
    ///     memory.finalize();
    ///     
    ///     let total_time = memory.get_total_runtime();
    ///     println!("Total execution time: {:?}", total_time);
    /// }
    /// ```
    #[inline]
    #[cfg(feature = "profiler")]
    pub fn get_total_runtime(&self) -> Duration {
        let completion_time = unsafe { *self.timer.completion_timestamp.get() };
        completion_time.duration_since(self.timer.init_timestamp)
    }
}
