use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// A simple read-write lock for mutual exclusion in a `no_std` environment.
///
/// `RwLock` allows multiple readers or exclusive access to data across threads or cores by
/// continuously spinning (busy-waiting) until the lock becomes available.
/// It is useful in bare-metal or kernel development where blocking is
/// not possible.
///
/// # Type Parameters
///
/// * `T` - The type of data protected by the read-write lock.
pub struct RwLock<T> {
    reader_count: AtomicUsize,
    writer_lock: AtomicBool,
    data: UnsafeCell<T>,
}

// SAFETY: RwLock can be shared between threads if T can be sent between threads
unsafe impl<T: Send> Sync for RwLock<T> {}
unsafe impl<T: Send> Send for RwLock<T> {}

impl<T> RwLock<T> {
    /// Creates a new `RwLock` wrapping the provided data.
    ///
    /// # Example
    ///
    /// ```
    /// let lock = RwLock::new(0);
    /// ```
    pub const fn new(data: T) -> Self {
        Self {
            reader_count: AtomicUsize::new(0),
            writer_lock: AtomicBool::new(false),
            data: UnsafeCell::new(data),
        }
    }

    pub fn read(&self) -> RwLockGuard<'_, T> {
        while self.writer_lock.load(Ordering::Acquire) {
            core::hint::spin_loop();
        }

        self.reader_count.fetch_add(1, Ordering::AcqRel);
        RwLockGuard {
            lock: self,
            writer: false,
        }
    }

    pub fn write(&self) -> RwLockGuard<'_, T> {
        while self
            .writer_lock
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
            || self.reader_count.load(Ordering::Acquire) > 0
        {
            core::hint::spin_loop();
        }
        RwLockGuard {
            lock: self,
            writer: true,
        }
    }
}

/// A guard that provides access to the data protected by a `RwLock`.
///
/// This guard is returned by `RwLock::read` and `RwLock::write`. It releases the lock or decrements the reader count
/// automatically when dropped.
pub struct RwLockGuard<'a, T> {
    lock: &'a RwLock<T>,
    writer: bool,
}

impl<T> core::ops::Deref for RwLockGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // SAFETY: The lock is held, so we have exclusive access
        unsafe { &*self.lock.data.get() }
    }
}

impl<T> core::ops::DerefMut for RwLockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: The lock is held, so we have exclusive access
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T> Drop for RwLockGuard<'_, T> {
    /// Releases the lock when the guard goes out of scope.
    fn drop(&mut self) {
        if self.writer {
            self.lock.writer_lock.store(false, Ordering::Release);
        } else {
            self.lock.reader_count.fetch_sub(1, Ordering::Release);
        }
    }
}
