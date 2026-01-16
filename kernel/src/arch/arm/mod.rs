//! ARM Architecture Support
//!
//! Architecture-specific utilities and helpers.

pub mod context;
pub mod exception;
pub mod interrupt;
pub mod mmu;

/// Wait for interrupt (low-power idle)
///
/// Puts the CPU into a low-power state until an interrupt occurs.
/// This is more power-efficient than busy-waiting in a loop.
///
/// # Example
///
/// ```rust
/// loop {
///     wfi(); // Sleep until interrupt
///     // Process work triggered by interrupt
/// }
/// ```
#[inline(always)]
pub fn wfi() {
    unsafe {
        core::arch::asm!("wfi", options(nomem, nostack, preserves_flags));
    }
}

/// Wait for event (low-power idle with events)
///
/// Similar to WFI but also wakes on SEV (Send Event) instruction.
#[inline(always)]
pub fn wfe() {
    unsafe {
        core::arch::asm!("wfe", options(nomem, nostack, preserves_flags));
    }
}

/// Send event to all CPUs
///
/// Wakes CPUs waiting in WFE.
#[inline(always)]
pub fn sev() {
    unsafe {
        core::arch::asm!("sev", options(nomem, nostack, preserves_flags));
    }
}

/// Data synchronization barrier
///
/// Ensures all memory accesses before this point complete
/// before any after it begin.
#[inline(always)]
pub fn dsb() {
    unsafe {
        core::arch::asm!("dsb", options(nostack, preserves_flags));
    }
}

/// Data memory barrier
///
/// Ensures memory accesses occur in program order.
#[inline(always)]
pub fn dmb() {
    unsafe {
        core::arch::asm!("dmb", options(nostack, preserves_flags));
    }
}

/// Instruction synchronization barrier
///
/// Flushes the pipeline and ensures all instructions
/// after this point see the effects of instructions before.
#[inline(always)]
pub fn isb() {
    unsafe {
        core::arch::asm!("isb", options(nostack, preserves_flags));
    }
}
