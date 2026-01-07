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
use drivers::hw::bcm2835::{firmware_memory::get_arm_memory, interrupt, timer::Timer};
use drivers::{uart::*, uart_println};

// Linker symbols
unsafe extern "C" {
    static mut _free_memory_start: u8;
}

#[unsafe(no_mangle)]
pub extern "C" fn kernel_main() -> ! {
    // Initialize UART0
    init_uart0(115200).expect("Failed to initialize UART0\n");

    // Initialize page allocator (FIRST AND ONLY TIME)
    unsafe {
        let free_mem_start = core::ptr::addr_of!(_free_memory_start) as usize;
        let (base, size) = get_arm_memory().expect("Failed to get ARM memory from firmware\n");
        PageAllocator::init(free_mem_start, base + size);
    }

    let allocator = PageAllocator::get();

    allocator.alloc().expect("Failed to allocate test page\n");

    interrupt::enable_irq(IRQ_SYSTEM_TIMER_1); // Enable timer IRQ

    handlers::register(IRQ_SYSTEM_TIMER_1, handlers::timer);

    crate::arch::arm::interrupt::enable(); // Enable IRQs

    Timer::start(1000000); // 1 second

    uart_println!("Kernel initialized successfully!");
    loop {}
}

// Required panic handler
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    uart_println!("Kernel panic: {}", info);
    loop {}
}
