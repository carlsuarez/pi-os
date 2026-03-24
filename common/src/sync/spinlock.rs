use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, Ordering};

/// A simple spinlock for mutual exclusion in a `no_std` environment.
///
/// `SpinLock` allows exclusive access to data across threads or cores by
/// continuously spinning (busy-waiting) until the lock becomes available.
/// It is useful in bare-metal or kernel development where blocking is
/// not possible.
///
/// # Type Parameters
///
/// * `T` - The type of data protected by the spinlock.
pub struct SpinLock<T: ?Sized> {
    locked: AtomicBool,
    data: UnsafeCell<T>,
}

// SAFETY: SpinLock can be shared between threads if T can be sent between threads
unsafe impl<T: ?Sized + Send> Sync for SpinLock<T> {}
unsafe impl<T: ?Sized + Send> Send for SpinLock<T> {}

impl<T: ?Sized> SpinLock<T> {
    /// Acquires the lock, spinning until it is available.
    ///
    /// Returns a `SpinLockGuard` which provides mutable access to the
    /// underlying data. The lock is automatically released when the guard
    /// is dropped.
    pub fn lock(&self) -> SpinLockGuard<'_, T> {
        while self
            .locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        SpinLockGuard { lock: self }
    }
}

impl SpinLock<[()]> {
    /// Creates a new `SpinLock` wrapping the provided data.
    pub const fn new<T>(data: T) -> SpinLock<T> {
        SpinLock {
            locked: AtomicBool::new(false),
            data: UnsafeCell::new(data),
        }
    }
}

/// A guard that provides access to the data protected by a `SpinLock`.
pub struct SpinLockGuard<'a, T: ?Sized> {
    lock: &'a SpinLock<T>,
}

impl<T: ?Sized> core::ops::Deref for SpinLockGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // SAFETY: The lock is held, so we have exclusive access
        unsafe { &*self.lock.data.get() }
    }
}

impl<T: ?Sized> core::ops::DerefMut for SpinLockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: The lock is held, so we have exclusive access
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T: ?Sized> Drop for SpinLockGuard<'_, T> {
    /// Releases the lock when the guard goes out of scope.
    fn drop(&mut self) {
        self.lock.locked.store(false, Ordering::Release);
    }
}
