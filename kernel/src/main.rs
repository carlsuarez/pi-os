//! Kernel Main Entry Point
//!

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]
#[allow(dead_code)]
extern crate alloc;

mod arch;
mod fs;
mod irq;
mod kcore;
mod mm;
mod process;
mod syscall;

use crate::irq::handlers;
use crate::mm::page_allocator::PAGE_ALLOCATOR;
use core::panic::PanicInfo;
use drivers::platform::{CurrentPlatform as Platform, Platform as PlatformTrait};

// ============================================================================
// Kernel Entry Point
// ============================================================================

#[unsafe(no_mangle)]
pub extern "C" fn kernel_main() -> ! {
    kprintln!("========================================");
    kprintln!("Booting {} kernel", Platform::name());
    kprintln!("========================================");

    // Test Memory Allocators
    test_memory_allocators();

    // Setup Interrupt Handlers
    let timer_irq = Platform::timer_irq();
    handlers::register(timer_irq, handlers::timer);
    kprintln!("Registered timer interrupt handler (IRQ {})", timer_irq);

    // Enable Interrupts
    Platform::enable_irq(timer_irq);
    kprintln!("Enabled timer interrupt");

    crate::arch::arm::interrupt::enable();
    kprintln!("Enabled CPU interrupts");

    //: Start System Timer
    Platform::timer_start(1_000_000); // 1 second interval
    kprintln!("Started system timer (1 second interval)");

    // Kernel is now fully initialized
    kprintln!("========================================");
    kprintln!("Kernel initialization complete!");
    kprintln!("Platform: {}", Platform::name());
    kprintln!("Entering main loop...");
    kprintln!("========================================");

    // Main kernel loop
    kernel_main_loop();
}

// ============================================================================
// Kernel Main Loop
// ============================================================================

fn kernel_main_loop() -> ! {
    loop {
        // Wait for interrupt
        crate::arch::arm::wfi();

        // Process pending work here
        // - Schedule processes
        // - Handle deferred work
        // - etc.
    }
}

// ============================================================================
// Memory Allocator Tests
// ============================================================================

fn test_memory_allocators() {
    use alloc::boxed::Box;
    use alloc::string::String;
    use alloc::vec::Vec;

    kprintln!("Testing memory allocators...");

    // Test page allocator
    let page = PAGE_ALLOCATOR.alloc().expect("Failed to allocate page");
    kprintln!("  Page allocator: allocated page at 0x{:X}", page.addr());

    let page_ptr = page.addr() as *mut u8;
    unsafe {
        core::ptr::write_bytes(page_ptr, 0xAB, 16);
        kprint!("  First 16 bytes: ");
        for i in 0..16 {
            kprint!("{:02X} ", *page_ptr.add(i));
        }
        kprintln!();
    }

    // Test heap allocator - Vec
    let mut v = Vec::new();
    v.push(1);
    v.push(2);
    v.push(3);
    kprintln!("  Heap allocator (Vec): {:?}", v);

    // Test heap allocator - String
    let mut s = String::from("Hello ");
    s.push_str("from heap!");
    kprintln!("  Heap allocator (String): {}", s);

    // Test heap allocator - Box
    let boxed = Box::new(42);
    kprintln!("  Heap allocator (Box): {}", *boxed);

    // Test collections
    let numbers: Vec<u32> = (0..10).collect();
    kprintln!("  Heap allocator (Range): {} elements", numbers.len());

    kprintln!("All memory tests passed");
}

// ============================================================================
// Panic Handler
// ============================================================================

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    kprintln!();
    kprintln!("========================================");
    kprintln!("KERNEL PANIC!");
    kprintln!("========================================");
    kprintln!("{}", info);
    kprintln!("========================================");
    kprintln!("System halted.");

    loop {
        crate::arch::arm::wfi();
    }
}

// ============================================================================
// Print Macros
// ============================================================================

/// Print to console without newline
#[macro_export]
macro_rules! kprint {
    ($($arg:tt)*) => {{
        use alloc::format;
        use drivers::platform::{CurrentPlatform as Platform, Platform as PlatformTrait};
        let s = format!($($arg)*);
        Platform::console_write(&s);
    }};
}

/// Print to console with newline
#[macro_export]
macro_rules! kprintln {
    () => { $crate::kprint!("\n") };
    ($($arg:tt)*) => {{
        $crate::kprint!($($arg)*);
        $crate::kprint!("\n");
    }};
}
