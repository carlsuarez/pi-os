pub mod spinlock;
pub use spinlock::SpinLock;
pub mod irq;
pub mod irq_spinlock;
pub mod rwlock;
pub use rwlock::RwLock;

cfg_if::cfg_if! {
    if #[cfg(target_arch = "arm")] {
        // ARM-specific implementation
        use crate::arch::arm::irq::ArmIrq as PlatformIrq;
    }
    else {
        compile_error!("Unsupported architecture");
    }
}

// Type alias that works everywhere
pub type IrqSpinLock<T> = irq_spinlock::IrqSpinLock<T, PlatformIrq>;
