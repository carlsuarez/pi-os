#![no_std]
#![no_main]
#![allow(dead_code)]
#![feature(alloc_error_handler)]

extern crate alloc;
mod arch;
mod irq;
mod kcore;
mod mm;
mod syscall;
use crate::arch::arm::interrupt::irq_numbers::*;
use crate::irq::handlers;
use crate::mm::heap_allocator;
use crate::mm::page_allocator::PAGE_ALLOCATOR;
use core::panic::PanicInfo;
use drivers::hw::bcm2835::{firmware_memory::get_arm_memory, interrupt, timer::Timer};
use drivers::{uart::*, uart_println};

// Linker symbols
unsafe extern "C" {
    static mut _free_memory_start: u8;
    static mut _kernel_heap_start: u8;
    static mut _kernel_heap_end: u8;
}

#[unsafe(no_mangle)]
pub extern "C" fn kernel_main() -> ! {
    // Initialize UART0
    init_uart0(115200).expect("Failed to initialize UART0\n");

    // Initialize page allocator and heap (FIRST AND ONLY TIME)
    unsafe {
        let free_mem_start = core::ptr::addr_of!(_free_memory_start) as usize;
        let (base, size) = get_arm_memory().expect("Failed to get ARM memory from firmware\n");
        PAGE_ALLOCATOR.init(free_mem_start, base + size);

        let heap_start = core::ptr::addr_of!(_kernel_heap_start) as usize;
        let heap_end = core::ptr::addr_of!(_kernel_heap_end) as usize;
        heap_allocator::init_heap(heap_start, heap_end);
    }

    let page = PAGE_ALLOCATOR
        .alloc()
        .expect("Failed to allocate test page\n");

    uart_println!("Allocated test page at address: {:#X}", page.addr());
    let page_ptr = page.addr() as *mut u8;
    unsafe {
        core::ptr::write_bytes(page_ptr, 0xAB, 4096);
        for i in 0..16 {
            uart_println!("Byte {}: {:#X}", i, *page_ptr.add(i));
        }
    }

    test_heap_allocations();

    interrupt::enable_irq(IRQ_SYSTEM_TIMER_1); // Enable timer IRQ

    handlers::register(IRQ_SYSTEM_TIMER_1, handlers::timer);

    crate::arch::arm::interrupt::enable(); // Enable IRQs

    Timer::start(1000000); // 1 second

    uart_println!("Kernel initialized successfully!");
    loop {}
}

fn test_heap_allocations() {
    use alloc::boxed::Box;
    use alloc::string::String;
    use alloc::vec::Vec;

    uart_println!("Testing heap allocations...");

    // Test Vec
    let mut v = Vec::new();
    v.push(1);
    v.push(2);
    v.push(3);
    uart_println!("Vec works! {:?}", v);

    // Test String
    let mut s = String::from("Hello ");
    s.push_str("from heap!");
    uart_println!("String works! {}", s);

    // Test Box
    let boxed = Box::new(42);
    uart_println!("Box works! {}", *boxed);

    // Test collections
    let numbers: Vec<u32> = (0..10).collect();
    uart_println!("Range collection works! len={}", numbers.len());

    uart_println!("All heap tests passed!");
}

// Required panic handler
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    uart_println!("Kernel panic: {}", info);
    loop {}
}
