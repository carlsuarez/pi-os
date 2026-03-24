//! Interrupt Dispatch
//!
//! Called from architecture-specific exception handlers.

use crate::arch::{Irq, TrapFrame};
use crate::subsystems::irq_controller;
use common::sync::irq::{self, IrqControl};

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
    let irqctl = irq_controller().expect("no IRQ controller registered");
    // Mask this specific IRQ line to prevent re-entry
    let _ = irqctl.lock().disable(irq);

    // Enable CPU interrupts to allow nested interrupts
    // (other IRQs can fire while we handle this one)
    crate::arch::Irq::enable();

    // Call the registered handler for this IRQ
    if let Some(handler) = crate::irq::handlers::get_handler(irq) {
        handler(tf);
    } else {
        // No handler registered - spurious interrupt
        crate::kprintln!("Unhandled IRQ: {}", irq);
    }

    // Enter critical section for cleanup
    Irq::disable();

    // Unmask this IRQ line so it can fire again
    let _ = irqctl.lock().enable(irq);

    // Return to interrupted code
}

/// Get next pending interrupt and dispatch it
///
/// This can be called from the main interrupt handler to poll
/// for pending interrupts.
pub fn dispatch_next(tf: &mut TrapFrame) -> bool {
    if let Some(irq) = { irq_controller().unwrap().lock().next_pending() } {
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
