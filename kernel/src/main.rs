#![no_std]
#![no_main]
#![allow(dead_code)]

mod arch;
mod irq;
mod kcore;
mod mm;
mod syscall;
use crate::arch::arm::interrupt::irq_numbers::*;
use crate::irq::handlers;
use crate::mm::PageAllocator;
use core::panic::PanicInfo;
use drivers::hw::bcm2835::{
    interrupt,
    memory::{get_ram_end, get_ram_start},
    timer::Timer,
};
use drivers::uart::*;

#[unsafe(no_mangle)]
pub extern "C" fn kernel_main() -> ! {
    // Initialize page allocator (FIRST AND ONLY TIME)
    unsafe {
        PageAllocator::init(get_ram_start(), get_ram_end());
    }

    let allocator = PageAllocator::get();

    allocator.alloc().expect("Failed to allocate test page");

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
