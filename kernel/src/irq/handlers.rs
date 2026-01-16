use crate::arch::arm::exception::TrapFrame;
use drivers::platform::{CurrentPlatform, Platform};
pub type IrqHandler = fn(&mut TrapFrame);

const MAX_IRQS: usize = 128;

static mut IRQ_HANDLERS: [Option<IrqHandler>; MAX_IRQS] = [None; MAX_IRQS];

pub fn register(irq: u32, handler: IrqHandler) {
    unsafe {
        IRQ_HANDLERS[irq as usize] = Some(handler);
    }
}

pub(crate) fn get_handler(irq: u32) -> Option<IrqHandler> {
    unsafe { IRQ_HANDLERS[irq as usize] }
}

pub fn timer(_tf: &mut TrapFrame) {
    CurrentPlatform::timer_clear();
    crate::kprintln!("Timer interrupt");
    CurrentPlatform::timer_start(1_000_000); // 1 second
}

pub fn uart(_tf: &mut TrapFrame) {}
