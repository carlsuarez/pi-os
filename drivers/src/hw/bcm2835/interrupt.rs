use crate::hw::bcm2835::int_reg::{INT_REG_BASE, IntReg};
use core::ptr::{addr_of, addr_of_mut, read_volatile, write_volatile};

/// Return a pointer to the BCM2835 interrupt controller register block.
///
/// # Safety
/// The returned pointer refers to memory-mapped hardware registers and
/// must only be accessed using volatile operations.
fn regs() -> *mut IntReg {
    INT_REG_BASE as *mut IntReg
}

/// Query the interrupt controller for a pending IRQ.
///
/// This function checks, in order:
/// 1. IRQs 0–31 (`IRQ_PENDING_1`)
/// 2. IRQs 32–63 (`IRQ_PENDING_2`)
/// 3. Basic IRQs 64+ (`IRQ_BASIC_PENDING`)
///
/// If any interrupt is pending, the lowest-numbered pending IRQ is
/// returned. If no interrupts are pending, `None` is returned.
pub fn pending_irq() -> Option<u32> {
    unsafe {
        let r = regs();

        // Check pending IRQs 0–31
        let irq1_ptr = addr_of!((*r).irq_1_pend);
        let irq1 = read_volatile(irq1_ptr);
        if irq1 != 0 {
            return Some(irq1.trailing_zeros());
        }

        // Check pending IRQs 32–63
        let irq2_ptr = addr_of!((*r).irq_2_pend);
        let irq2 = read_volatile(irq2_ptr);
        if irq2 != 0 {
            return Some(32 + irq2.trailing_zeros());
        }

        // Check basic pending IRQs (64+)
        let basic_ptr = addr_of!((*r).irq_basic_pend);
        let basic = read_volatile(basic_ptr);
        if basic != 0 {
            return Some(64 + basic.trailing_zeros());
        }

        None
    }
}

/// Logical representation of an interrupt line.
///
/// The BCM2835 interrupt controller divides interrupts into
/// two 32-bit banks and a set of basic interrupts.
pub enum IrqLine {
    /// IRQs 0–31.
    Irq1(u32),
    /// IRQs 32–63.
    Irq2(u32),
    /// Basic IRQs (64+).
    Basic(u32),
}

impl IrqLine {
    /// Split a global IRQ number into its bank and bit index.
    ///
    /// This converts a flat IRQ number into the corresponding
    /// interrupt register and bit position.
    fn split(irq: u32) -> Self {
        match irq {
            0..=31 => IrqLine::Irq1(irq),
            32..=63 => IrqLine::Irq2(irq - 32),
            _ => IrqLine::Basic(irq - 64),
        }
    }
}

/// Enable an interrupt line in the interrupt controller.
///
/// Writing a `1` to the corresponding enable register bit
/// unmasks the interrupt. Other bits are unaffected.
pub fn enable_irq(irq: u32) {
    unsafe {
        let r = regs();
        match IrqLine::split(irq) {
            IrqLine::Irq1(bit) => {
                let ptr = addr_of_mut!((*r).enable_irqs_1);
                write_volatile(ptr, 1 << bit);
            }
            IrqLine::Irq2(bit) => {
                let ptr = addr_of_mut!((*r).enable_irqs_2);
                write_volatile(ptr, 1 << bit);
            }
            IrqLine::Basic(bit) => {
                let ptr = addr_of_mut!((*r).enable_basic_irqs);
                write_volatile(ptr, 1 << bit);
            }
        }
    }
}

/// Disable (mask) an interrupt line in the interrupt controller.
///
/// Writing a `1` to the corresponding disable register bit
/// masks the interrupt. Other bits are unaffected.
pub fn disable_irq(irq: u32) {
    unsafe {
        let r = regs();
        match IrqLine::split(irq) {
            IrqLine::Irq1(bit) => {
                let ptr = addr_of_mut!((*r).disable_irqs_1);
                write_volatile(ptr, 1 << bit);
            }
            IrqLine::Irq2(bit) => {
                let ptr = addr_of_mut!((*r).disable_irqs_2);
                write_volatile(ptr, 1 << bit);
            }
            IrqLine::Basic(bit) => {
                let ptr = addr_of_mut!((*r).disable_basic_irqs);
                write_volatile(ptr, 1 << bit);
            }
        }
    }
}
