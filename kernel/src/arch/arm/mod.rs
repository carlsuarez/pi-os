//! ARM Architecture Support
//! Architecture-specific utilities and helpers.
pub mod context;
pub mod exception;
pub mod interrupt;
pub mod mmu;

/// Data Synchronization Barrier (DSB)
///
/// Ensures that all explicit memory accesses before this instruction complete
/// before any following instructions execute.
#[inline(always)]
pub fn dsb() {
    unsafe {
        core::arch::asm!(
            "mov r0, #0",
            "mcr p15, 0, r0, c7, c10, 4", // DSB
            out("r0") _,
            options(nostack, preserves_flags)
        );
    }
}

/// Data Memory Barrier (DMB)
///
/// Ensures memory accesses complete in program order.
#[inline(always)]
pub fn dmb() {
    unsafe {
        core::arch::asm!(
            "mov r0, #0",
            "mcr p15, 0, r0, c7, c10, 5", // DMB
            out("r0") _,
            options(nostack, preserves_flags)
        );
    }
}

/// Instruction Synchronization Barrier (ISB)
///
/// Flushes the pipeline and ensures all instructions after this point
/// see the effects of previous instructions.
#[inline(always)]
pub fn isb() {
    unsafe {
        core::arch::asm!(
            "mov r0, #0",
            "mcr p15, 0, r0, c7, c5, 4", // ISB
            out("r0") _,
            options(nostack, preserves_flags)
        );
    }
}

/// Wait for Interrupt
#[inline(always)]
pub fn wfi() {
    unsafe { core::arch::asm!("wfi", options(nomem, nostack, preserves_flags)) }
}

/// Wait for Event
#[inline(always)]
pub fn wfe() {
    unsafe { core::arch::asm!("wfe", options(nomem, nostack, preserves_flags)) }
}

/// Send Event
#[inline(always)]
pub fn sev() {
    unsafe { core::arch::asm!("sev", options(nomem, nostack, preserves_flags)) }
}
