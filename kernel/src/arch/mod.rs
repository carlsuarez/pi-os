#[cfg(target_arch = "arm")]
pub mod arm;

#[cfg(target_arch = "x86")]
pub mod x86;

#[cfg(target_arch = "x86_64")]
pub mod x86_64;

cfg_if::cfg_if! {
    if #[cfg(target_arch = "arm")] {
        // ARM-specific implementation
        pub use crate::arch::arm::interrupt::ArmIrq as Irq;
    }
    else if #[cfg(target_arch = "x86")] {
        // x86-specific implementation
        pub use crate::arch::x86::interrupt::X86Irq as Irq;
    }
    else {
        compile_error!("Unsupported architecture");
    }
}

cfg_if::cfg_if! {
    if #[cfg(target_arch = "arm")] {
        // ARM-specific implementation
        pub use crate::arch::arm::exception::trap::TrapFrame as TrapFrame;
    }
    else if #[cfg(target_arch = "x86")] {
        // x86-specific implementation
        pub use crate::arch::x86::exception::trap::TrapFrame as TrapFrame;
    }
    else {
        compile_error!("Unsupported architecture");
    }
}

// Type alias that works everywhere
pub type IrqSpinLock<T> = common::sync::irq_spinlock::IrqSpinLock<T, Irq>;
