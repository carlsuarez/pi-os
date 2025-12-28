use crate::arch::arm::exception::TrapFrame;
use drivers::uart::uart0;

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

pub fn timer(tf: &mut TrapFrame) {
    drivers::hw::bcm2835::timer::Timer::clear_interrupt();
    uart0().puts("timer interrupt\n");
    drivers::hw::bcm2835::timer::Timer::start(1_000_000); // 1 second
}
pub fn uart(tf: &mut TrapFrame) {
    uart0().puts("uart interrupt\n");
}
