#![no_std]
#![no_main]

mod arch;
mod irq;
mod syscall;
use crate::arch::arm::interrupt::irq_numbers::*;
use crate::arch::arm::mmu;
use crate::irq::handlers;
use core::panic::PanicInfo;
use drivers::hw::bcm2835::{interrupt, timer::Timer};
use drivers::uart::*;

#[unsafe(no_mangle)]
pub extern "C" fn kernel_main() -> ! {
    interrupt::enable_irq(IRQ_SYSTEM_TIMER_1); // Enable timer IRQ

    if let Err(_) = uart0().init(115200) {
        loop {}
    }

    handlers::register(IRQ_SYSTEM_TIMER_1, handlers::timer);

    crate::arch::arm::interrupt::enable(); // Enable IRQs

    Timer::start(1000000); // 1 second

    uart0().puts("Hello world!\n");
    loop {}
}

// Required panic handler
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
