use crate::arch::arm::exception::TrapFrame;
use crate::arch::arm::interrupt;
use drivers::hw::bcm2835::interrupt as bcm_irq;

pub fn dispatch(irq: u32, tf: &mut TrapFrame) {
    // Mask this IRQ line
    bcm_irq::disable_irq(irq);

    // Allow nested IRQs
    interrupt::enable();

    if let Some(handler) = crate::irq::handlers::get_handler(irq) {
        handler(tf);
    }

    // Critical section for exit
    interrupt::disable();

    // Unmask IRQ line
    bcm_irq::enable_irq(irq);
}
