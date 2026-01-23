use crate::arch::arm::exception::TrapFrame;
use drivers::{
    console::console_write,
    platform::{CurrentPlatform as Platform, Platform as PlatformTrait},
};
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
    Platform::timer_clear();
    console_write("Timer interrupt\n");
    Platform::timer_start(1_000_000); // 1 second
}

pub fn uart(_tf: &mut TrapFrame) {}
