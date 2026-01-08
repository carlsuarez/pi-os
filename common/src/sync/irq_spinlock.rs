use core::{
    cell::UnsafeCell,
    marker::PhantomData,
    sync::atomic::{AtomicBool, Ordering},
};

use super::irq::IrqControl;

/// IRQ-safe spinlock.
///
/// - Disables interrupts on lock
/// - Spins until acquired
/// - Restores interrupt state on drop
///
/// Safe to use from:
/// - IRQ context
/// - Normal kernel context
///
/// Not fair. Not reentrant.
pub struct IrqSpinLock<T, I: IrqControl> {
    locked: AtomicBool,
    data: UnsafeCell<T>,
    _irq: PhantomData<I>, // Prevent unused type parameter warning
}

unsafe impl<T: Send, I: IrqControl> Send for IrqSpinLock<T, I> {}
unsafe impl<T: Send, I: IrqControl> Sync for IrqSpinLock<T, I> {}

impl<T, I: IrqControl> IrqSpinLock<T, I> {
    /// Create a new IRQ-safe spinlock.
    pub const fn new(data: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            data: UnsafeCell::new(data),
            _irq: PhantomData,
        }
    }

    /// Acquire the lock with interrupts disabled.
    pub fn lock(&self) -> IrqSpinLockGuard<'_, T, I> {
        // Disable interrupts first
        let irq_state = I::disable();

        // Spin until lock is acquired
        while self
            .locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }

        IrqSpinLockGuard {
            lock: self,
            irq_state,
        }
    }
}

/// Guard returned by `IrqSpinLock::lock`.
///
/// Restores interrupt state on drop.
pub struct IrqSpinLockGuard<'a, T, I: IrqControl> {
    lock: &'a IrqSpinLock<T, I>,
    irq_state: I::State,
}

impl<'a, T, I: IrqControl> core::ops::Deref for IrqSpinLockGuard<'a, T, I> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T, I: IrqControl> core::ops::DerefMut for IrqSpinLockGuard<'a, T, I> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<'a, T, I: IrqControl> Drop for IrqSpinLockGuard<'a, T, I> {
    fn drop(&mut self) {
        // Release lock first
        self.lock.locked.store(false, Ordering::Release);

        // Restore IRQ state
        I::restore(self.irq_state);
    }
}
