use crate::hw::bcm2835::int_reg::{INT_REG_BASE, IntReg};
use core::ptr::{read_volatile, write_volatile};

fn regs() -> *mut IntReg {
    INT_REG_BASE as *mut IntReg
}

pub fn pending_irq() -> Option<u32> {
    unsafe {
        let r = regs();

        let irq1 = read_volatile(&(*r).irq_1_pend);
        if irq1 != 0 {
            return Some(irq1.trailing_zeros());
        }

        let irq2 = read_volatile(&(*r).irq_2_pend);
        if irq2 != 0 {
            return Some(32 + irq2.trailing_zeros());
        }

        let basic = read_volatile(&(*r).irq_basic_pend);
        if basic != 0 {
            return Some(64 + basic.trailing_zeros());
        }

        None
    }
}

pub enum IrqLine {
    Irq1(u32),  // 0..31
    Irq2(u32),  // 32..63
    Basic(u32), // 64+
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

pub fn enable_irq(irq: u32) {
    unsafe {
        let r = regs();
        match IrqLine::split(irq) {
            IrqLine::Irq1(bit) => write_volatile(&mut (*r).enable_irqs_1, 1 << bit),

            IrqLine::Irq2(bit) => write_volatile(&mut (*r).enable_irqs_2, 1 << bit),

            IrqLine::Basic(bit) => write_volatile(&mut (*r).enable_basic_irqs, 1 << bit),
        }
    }
}

pub fn disable_irq(irq: u32) {
    unsafe {
        let r = regs();
        match IrqLine::split(irq) {
            IrqLine::Irq1(bit) => write_volatile(&mut (*r).disable_irqs_1, 1 << bit),

            IrqLine::Irq2(bit) => write_volatile(&mut (*r).disable_irqs_2, 1 << bit),

            IrqLine::Basic(bit) => write_volatile(&mut (*r).disable_basic_irqs, 1 << bit),
        }
    }
}
