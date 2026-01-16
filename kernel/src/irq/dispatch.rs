//! Interrupt Dispatch
//!
//! Called from architecture-specific exception handlers.

use crate::arch::arm::exception::TrapFrame;
use crate::arch::arm::interrupt;
use drivers::platform::{CurrentPlatform as Platform, Platform as PlatformTrait};

/// Dispatch an interrupt to its registered handler
///
/// This is the main interrupt dispatcher called from the exception vector.
///
/// # Arguments
/// - `irq`: IRQ number that fired
/// - `tf`: Trap frame containing saved CPU state
///
/// # Process
/// 1. Mask the IRQ to prevent re-entry
/// 2. Enable interrupts to allow nesting
/// 3. Call registered handler
/// 4. Disable interrupts for critical exit
/// 5. Unmask the IRQ
pub fn dispatch(irq: u32, tf: &mut TrapFrame) {
    // Mask this specific IRQ line to prevent re-entry
    Platform::disable_irq(irq);

    // Enable CPU interrupts to allow nested interrupts
    // (other IRQs can fire while we handle this one)
    interrupt::enable();

    // Call the registered handler for this IRQ
    if let Some(handler) = crate::irq::handlers::get_handler(irq) {
        handler(tf);
    } else {
        // No handler registered - spurious interrupt
        crate::kprintln!("Unhandled IRQ: {}", irq);
    }

    // Enter critical section for cleanup
    interrupt::disable();

    // Unmask this IRQ line so it can fire again
    Platform::enable_irq(irq);

    // Return to interrupted code
}

/// Get next pending interrupt and dispatch it
///
/// This can be called from the main interrupt handler to poll
/// for pending interrupts.
pub fn dispatch_next(tf: &mut TrapFrame) -> bool {
    if let Some(irq) = Platform::next_pending_irq() {
        dispatch(irq, tf);
        true
    } else {
        false
    }
}

/// Dispatch all pending interrupts
///
/// Useful for systems where multiple interrupts may be pending.
pub fn dispatch_all(tf: &mut TrapFrame) {
    while dispatch_next(tf) {
        // Continue dispatching until no more pending
    }
}
