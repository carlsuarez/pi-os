//! BCM2835 Interrupt Controller Driver

use crate::hal::interrupt::{InterruptController, IrqNumber};
use core::ptr::{read_volatile, write_volatile};

/// Interrupt controller base address.
pub const INT_CONTROLLER_BASE: usize = 0x2000_b000;

/// Memory-mapped interrupt controller registers.
#[repr(C)]
struct Registers {
    _padding: [u8; 0x200],
    irq_basic_pend: u32,
    irq_1_pend: u32,
    irq_2_pend: u32,
    fiq_ctrl: u32,
    enable_irqs_1: u32,
    enable_irqs_2: u32,
    enable_basic_irqs: u32,
    disable_irqs_1: u32,
    disable_irqs_2: u32,
    disable_basic_irqs: u32,
}

#[inline(always)]
fn regs() -> *mut Registers {
    INT_CONTROLLER_BASE as *mut Registers
}

/// Interrupt line representation.
enum IrqLine {
    Irq1(u32),
    Irq2(u32),
    Basic(u32),
}

impl IrqLine {
    fn split(irq: u32) -> Self {
        match irq {
            0..=31 => IrqLine::Irq1(irq),
            32..=63 => IrqLine::Irq2(irq - 32),
            _ => IrqLine::Basic(irq - 64),
        }
    }
}

// ============================================================================
// Raw Hardware Functions
// ============================================================================

/// Query for a pending IRQ.
pub fn pending_irq() -> Option<u32> {
    unsafe {
        let r = regs();

        // Check IRQs 0-31
        let irq1 = read_volatile(&(*r).irq_1_pend);
        if irq1 != 0 {
            return Some(irq1.trailing_zeros());
        }

        // Check IRQs 32-63
        let irq2 = read_volatile(&(*r).irq_2_pend);
        if irq2 != 0 {
            return Some(32 + irq2.trailing_zeros());
        }

        // Check basic IRQs
        let basic = read_volatile(&(*r).irq_basic_pend);
        if basic != 0 {
            return Some(64 + basic.trailing_zeros());
        }

        None
    }
}

/// Enable an interrupt line.
pub fn enable_irq(irq: u32) {
    unsafe {
        let r = regs();
        match IrqLine::split(irq) {
            IrqLine::Irq1(bit) => {
                write_volatile(&mut (*r).enable_irqs_1, 1 << bit);
            }
            IrqLine::Irq2(bit) => {
                write_volatile(&mut (*r).enable_irqs_2, 1 << bit);
            }
            IrqLine::Basic(bit) => {
                write_volatile(&mut (*r).enable_basic_irqs, 1 << bit);
            }
        }
    }
}

/// Disable an interrupt line.
pub fn disable_irq(irq: u32) {
    unsafe {
        let r = regs();
        match IrqLine::split(irq) {
            IrqLine::Irq1(bit) => {
                write_volatile(&mut (*r).disable_irqs_1, 1 << bit);
            }
            IrqLine::Irq2(bit) => {
                write_volatile(&mut (*r).disable_irqs_2, 1 << bit);
            }
            IrqLine::Basic(bit) => {
                write_volatile(&mut (*r).disable_basic_irqs, 1 << bit);
            }
        }
    }
}

// ============================================================================
// HAL Implementation
// ============================================================================

/// BCM2835 interrupt controller.
#[derive(Debug)]
pub struct Bcm2835InterruptController;

impl Bcm2835InterruptController {
    /// Create a new interrupt controller.
    ///
    /// # Safety
    ///
    /// Interrupt controller registers must be properly mapped.
    pub const unsafe fn new() -> Self {
        Self
    }
}

/// Interrupt controller errors (operations are infallible).
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum InterruptError {}

impl InterruptController for Bcm2835InterruptController {
    type Error = InterruptError;

    fn enable(&mut self, irq: IrqNumber) -> Result<(), Self::Error> {
        enable_irq(irq);
        Ok(())
    }

    fn disable(&mut self, irq: IrqNumber) -> Result<(), Self::Error> {
        disable_irq(irq);
        Ok(())
    }

    fn is_pending(&self, _irq: IrqNumber) -> Result<bool, Self::Error> {
        // BCM2835 doesn't provide efficient per-IRQ pending check
        Ok(false)
    }

    fn next_pending(&self) -> Option<IrqNumber> {
        pending_irq()
    }
}
