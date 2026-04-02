use drivers::device_manager::DeviceManager;

use crate::arch::TrapFrame;
use crate::subsystems::{serial_console, system_timer};
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
    let channel = DeviceManager::sys_timer_channel()
        .expect("timer IRQ fired but no system timer channel registered");

    let sys_timer = system_timer().expect("timer IRQ fired but no system timer registered");

    let mut timer = sys_timer.lock();
    timer.stop(channel).expect("failed to stop system timer");
    timer
        .clear_interrupt(channel)
        .expect("failed to clear timer interrupt");

    drop(timer); // release before console write to minimize lock hold time

    let _ = serial_console()
        .expect("no console registered")
        .lock()
        .write(b"Timer interrupt\n");

    sys_timer
        .lock()
        .start(channel, 1_000_000)
        .expect("failed to restart system timer");
}

pub fn uart(_tf: &mut TrapFrame) {}
