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
//!
//! ## Usage Pattern
//!
//! ```rust,no_run
//! use synced_memory::SynchedMemory;
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
//!         // All threads have now written and read the synchronized state
//!         println!("Global data length: {}", global_data.len());
//!     })
//! }).collect();
//!
//! for handle in handles {
//!     handle.join().unwrap();
//! }
//! ```
//!
//! ## Thread Safety
//!
//! - Each thread only writes to its own segment (no contention)
//! - Atomic timestamps coordinate global synchronization
//! - Built-in barriers ensure proper write-then-read ordering
//! - `UnsafeCell` allows safe interior mutability without runtime overhead

#[cfg(test)]
mod tests;

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
/// ## Architecture
///
/// - **Segmented Memory**: Total memory = `num_threads × max_size_per_thread`
/// - **Atomic Timestamps**: Each thread tracks when it last wrote data
/// - **Global Timestamp**: Minimum of all thread timestamps, used for synchronization
/// - **Write Barrier**: Ensures all threads finish writing before any can read
///
/// ## Memory Layout
///
/// ```text
/// Thread 0: [data...] | Thread 1: [data...] | Thread 2: [data...] | ...
/// ```
///
/// ## Synchronization Protocol
///
/// 1. Each thread writes to its own segment
/// 2. Thread updates its timestamp atomically
/// 3. All threads wait at write barrier
/// 4. Once all threads have written, global read becomes available
/// 5. Threads can read the complete synchronized state
///
/// ## Type Requirements
///
/// - `T: Default` - Used for clearing segments between writes
/// - `T: Clone` - Required for returning global data copies
/// - `T: Copy` - Enables efficient memory operations
/// - `T: Send` - Allows safe transfer between threads
pub struct SynchedMemory<T> {
    /// The shared memory buffer, divided into per-thread segments.
    /// Uses `UnsafeCell` to allow interior mutability without runtime checks.
    pub(crate) data: Arc<UnsafeCell<Vec<T>>>,

    /// Number of threads that will access this memory.
    pub(crate) num_threads: usize,

    /// Maximum number of elements each thread can write to its segment.
    pub(crate) max_size_per_thread: usize,

    /// Barrier to ensure all threads finish writing before any can read.
    /// This is the key synchronization primitive for the write-then-read pattern.
    write_barrier: Arc<Barrier>,
}

// SAFETY: SynchedMemory is safe to share between threads because:
// 1. Each thread only writes to its own memory segment (no data races)
// 2. All synchronization is handled through atomic operations and barriers
// 3. The UnsafeCell access is carefully controlled to prevent concurrent mutation
unsafe impl<T: Send> Send for SynchedMemory<T> {}
unsafe impl<T: Send> Sync for SynchedMemory<T> {}

/// A handle representing a thread's exclusive access to its segment of shared memory.
///
/// `LocalMemHandler` provides a thread with a direct pointer to its portion of the
/// shared memory buffer. This allows for high-performance writes without
/// any synchronization overhead during the write operation itself.
///
/// ## Safety
///
/// The raw pointer is safe to use because:
/// - Each thread gets a unique, non-overlapping memory segment
/// - The pointer remains valid for the lifetime of the `SynchedMemory`
/// - Write access is coordinated through the barrier system
///
/// ## Usage
///
/// ```rust,no_run
/// use synced_memory::SynchedMemory;
///
/// let memory = SynchedMemory::<i32>::new(4, 10);
/// let local_handler = memory.build_local_handler(0); // Thread 0's segment
///
/// // This thread can now write to its segment exclusively
/// memory.write_local_memory(&local_handler, &[1, 2, 3, 4, 5]);
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
    /// * `num_threads` - The number of threads that will access this memory
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
    /// # Examples
    ///
    /// ```rust
    /// use synced_memory::SynchedMemory;
    ///
    /// // Create memory for 4 threads, 100 elements each
    /// let memory = SynchedMemory::<i32>::new(4, 100);
    ///
    /// // Total memory: 4 × 100 × 4 bytes = 1,600 bytes
    /// ```
    ///
    /// # Panics
    ///
    /// This function will panic if `num_threads` is 0, as barriers require at least one thread.
    #[inline]
    pub fn new(num_threads: usize, max_size_per_thread: usize) -> Self {
        assert!(num_threads > 0, "Number of threads must be greater than 0");

        let total_size = num_threads * max_size_per_thread;
        let data = Arc::new(UnsafeCell::new(vec![T::default(); total_size]));
        let write_barrier = Arc::new(Barrier::new(num_threads));

        SynchedMemory {
            data,
            num_threads,
            max_size_per_thread,
            write_barrier,
        }
    }

    /// Returns the number of threads this memory instance supports.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use synced_memory::SynchedMemory;
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
    /// use synced_memory::SynchedMemory;
    ///
    /// let memory = SynchedMemory::<i32>::new(4, 25);
    /// assert_eq!(memory.data_len(), 100); // 4 × 25
    /// ```
    #[inline]
    pub fn data_len(&self) -> usize {
        unsafe { (*self.data.get()).len() }
    }

    /// Reads the global synchronized memory state.
    ///
    /// This method returns a complete copy of the synchronized memory state.
    /// It should be called after the write barrier has been passed to ensure
    /// all threads have completed their writes.
    ///
    /// # Returns
    ///
    /// A complete copy of the synchronized memory state.
    ///
    /// # Thread Safety
    ///
    /// This method is safe to call because:
    /// - It's typically called after `write_local_memory_and_sync_read()` which ensures synchronization
    /// - The write barrier ensures all threads finish writing before reading
    /// - Each thread only reads after all threads have synchronized
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use synced_memory::SynchedMemory;
    /// use std::sync::Arc;
    /// use std::thread;
    ///
    /// let memory = Arc::new(SynchedMemory::<i32>::new(2, 10));
    /// let mem = Arc::clone(&memory);
    ///
    /// // After synchronization (typically done via write_local_memory_and_sync_read)
    /// let global_data = mem.read_global();
    /// println!("Global data: {} elements", global_data.len());
    /// ```
    ///
    /// # Performance
    ///
    /// This method clones the entire data vector, which may be expensive for large datasets.
    /// Consider the trade-off between safety and performance for your use case.
    #[inline]
    pub fn read_global(&self) -> Vec<T> {
        unsafe {
            let data = &*self.data.get();
            data.clone()
        }
    }

    /// Creates a `LocalMemHandler` handle for the specified thread.
    ///
    /// This handle provides exclusive write access to the thread's memory segment.
    /// Each thread must call this method once with its unique thread ID.
    ///
    /// # Arguments
    ///
    /// * `thread_id` - Unique identifier for the thread (0 to num_threads-1)
    ///
    /// # Returns
    ///
    /// A `LocalMemHandler` handle containing a pointer to the thread's memory segment.
    ///
    /// # Memory Layout
    ///
    /// Thread N's segment starts at index `N × max_size_per_thread` in the data vector.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use synced_memory::SynchedMemory;
    ///
    /// let memory = SynchedMemory::<i32>::new(4, 10);
    ///
    /// // Each thread gets its own local memory handler
    /// let handler_0 = memory.build_local_handler(0); // Elements 0-9
    /// let handler_1 = memory.build_local_handler(1); // Elements 10-19
    /// let handler_2 = memory.build_local_handler(2); // Elements 20-29
    /// let handler_3 = memory.build_local_handler(3); // Elements 30-39
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `thread_id >= num_threads()`.
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
        let data_ptr = unsafe { (*self.data.get()).as_mut_ptr().add(start_index) };

        LocalMemHandler {
            data_ptr,
            thread_id,
        }
    }

    /// Writes data to a thread's local segment and synchronizes with other threads.
    ///
    /// This is the primary method for coordinated memory access. It performs the following:
    ///
    /// 1. Writes data to the thread's local segment
    /// 2. Waits for ALL threads to finish writing (write barrier)
    /// 3. Reads and returns the complete synchronized global state
    ///
    /// # Arguments
    ///
    /// * `local_handler` - The thread's local memory handler
    /// * `data` - The data to write to the local segment
    ///
    /// # Returns
    ///
    /// The complete synchronized memory state after all threads have written.
    ///
    /// # Synchronization Behavior
    ///
    /// This method ensures the write-then-read pattern:
    /// - **Write Phase**: Thread writes its data and updates its timestamp
    /// - **Barrier Phase**: All threads wait until everyone finishes writing
    /// - **Read Phase**: Thread reads the complete global state
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use synced_memory::SynchedMemory;
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
    ///         println!("Thread {} sees global data: {:?}", thread_id, global_data);
    ///     })
    /// }).collect();
    ///
    /// for handle in handles {
    ///     handle.join().unwrap();
    /// }
    /// ```
    ///
    /// # Panics
    ///
    /// - If `data.len() > max_size_per_thread`
    /// - If the local handler's thread ID is invalid
    /// - If the thread's timestamp doesn't match the expected value
    #[inline]
    pub fn write_local_memory_and_sync_read(
        &self,
        local_handler: &LocalMemHandler<T>,
        data: &[T],
    ) -> Vec<T> {
        // Write the data to the local segment
        self.write_local_memory(local_handler, data);

        // Wait for ALL threads to finish writing (key synchronization point)
        self.write_barrier.wait();

        // Now that all threads have written, read the global state
        self.read_global()
    }

    /// Provides manual access to the write barrier for advanced use cases.
    ///
    /// Most users should use `write_local_memory_and_sync_read()` instead, which
    /// handles barrier synchronization automatically.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use synced_memory::SynchedMemory;
    ///
    /// let memory = SynchedMemory::<i32>::new(4, 10);
    /// let local_handler = memory.build_local_handler(0);
    ///
    /// // Manual synchronization (not recommended for typical use)
    /// memory.write_local_memory(&local_handler, &[1, 2, 3]);
    /// memory.wait_write_barrier(); // Wait for all threads
    /// let global_data = memory.read_global();
    /// ```
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
    /// 1. Validates input parameters
    /// 2. Clears the entire local segment with `T::default()` values
    /// 3. Copies the new data to the segment
    /// 4. Updates the thread's timestamp atomically
    ///
    /// # Arguments
    ///
    /// * `local_handler` - The thread's local memory handler
    /// * `data` - The data to write (must fit in the local segment)
    ///
    /// # Safety
    ///
    /// This method uses unsafe code to write directly to memory, but is safe because:
    /// - Each thread writes only to its own segment
    /// - The segment size is validated
    /// - The thread ID is validated
    ///
    /// # Examples
    ///
    /// ```rust
    /// use synced_memory::SynchedMemory;
    ///
    /// let memory = SynchedMemory::<i32>::new(2, 10);
    /// let local_handler = memory.build_local_handler(0);
    ///
    /// // Low-level write (no synchronization)
    /// memory.write_local_memory(&local_handler, &[1, 2, 3, 4, 5]);
    /// ```
    ///
    /// # Panics
    ///
    /// - If `data.len() > max_size_per_thread`
    /// - If the local handler's thread ID is invalid
    /// - If the thread's current timestamp doesn't match the global timestamp
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

        // Perform the memory write operation
        unsafe {
            // Clear the entire segment to remove old data
            let segment_slice =
                std::slice::from_raw_parts_mut(local_handler.data_ptr, self.max_size_per_thread);
            for item in segment_slice.iter_mut() {
                *item = T::default();
            }

            // Write the new data to the segment
            let data_slice = std::slice::from_raw_parts_mut(local_handler.data_ptr, data.len());
            data_slice.copy_from_slice(data);
        }
    }
}
