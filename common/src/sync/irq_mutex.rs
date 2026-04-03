use super::irq::IrqControl;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};
use spin::{Mutex, MutexGuard};

/// IRQ-safe mutex built on top of `spin::Mutex`.
///
/// - Disables interrupts on lock
/// - Restores interrupt state on drop
/// - Uses spin::Mutex internally for locking
///
/// Safe in:
/// - IRQ context
/// - Normal kernel context
///
/// Not reentrant. Not fair.
pub struct IrqMutex<T: ?Sized, I: IrqControl> {
    _irq: PhantomData<I>,
    inner: Mutex<T>,
}

unsafe impl<T: Send + ?Sized, I: IrqControl> Send for IrqMutex<T, I> {}
unsafe impl<T: Send + ?Sized, I: IrqControl> Sync for IrqMutex<T, I> {}

impl<T, I: IrqControl> IrqMutex<T, I> {
    /// Create a new IRQ-safe mutex.
    pub const fn new(data: T) -> Self {
        Self {
            inner: Mutex::new(data),
            _irq: PhantomData,
        }
    }
}

impl<T: ?Sized, I: IrqControl> IrqMutex<T, I> {
    /// Lock the mutex, disabling interrupts.
    pub fn lock(&self) -> IrqMutexGuard<'_, T, I> {
        let irq_state = I::save_and_disable();
        let guard = self.inner.lock();

        IrqMutexGuard {
            guard,
            irq_state,
            _irq: PhantomData,
        }
    }

    /// Try to lock without blocking.
    pub fn try_lock(&self) -> Option<IrqMutexGuard<'_, T, I>> {
        let irq_state = I::save_and_disable();

        match self.inner.try_lock() {
            Some(guard) => Some(IrqMutexGuard {
                guard,
                irq_state,
                _irq: PhantomData,
            }),
            None => {
                I::restore(irq_state);
                None
            }
        }
    }
}

/// Guard returned by `IrqMutex::lock`.
///
/// Restores interrupt state on drop.
pub struct IrqMutexGuard<'a, T: ?Sized, I: IrqControl> {
    guard: MutexGuard<'a, T>,
    irq_state: I::State,
    _irq: PhantomData<I>,
}

impl<'a, T: ?Sized, I: IrqControl> Deref for IrqMutexGuard<'a, T, I> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &*self.guard
    }
}

impl<'a, T: ?Sized, I: IrqControl> DerefMut for IrqMutexGuard<'a, T, I> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut *self.guard
    }
}

impl<'a, T: ?Sized, I: IrqControl> Drop for IrqMutexGuard<'a, T, I> {
    fn drop(&mut self) {
        // Explicitly drop the lock first
        unsafe {
            core::ptr::drop_in_place(&mut self.guard);
        }
        I::restore(self.irq_state);
    }
}
