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

use crate::fs::FileSystem;
use crate::fs::fat::fat32::*;
use crate::{fs::vfs::vfs, irq::handlers};
use alloc::sync::Arc;
use core::panic::PanicInfo;
use drivers::hal::block_device::BlockDevice;
use drivers::platform::bcm2835::EMMC;
use drivers::platform::{CurrentPlatform as Platform, Platform as PlatformTrait};

// ============================================================================
// Kernel Entry Point
// ============================================================================

#[unsafe(no_mangle)]
pub extern "C" fn kernel_main() -> ! {
    kprintln!("========================================");
    kprintln!("Booting {} kernel", Platform::name());
    kprintln!("========================================");

    // Get the BlockDevice
    let dev: Arc<dyn BlockDevice> = {
        let guard = EMMC.lock();
        // We clone the Arc inside the Option.
        // This gives us a new Arc pointer to the SAME driver.
        let dev_arc = guard.as_ref().expect("EMMC not initialized").clone();
        dev_arc
    };

    // Mount root filesystem (FAT32)
    vfs().init(Fat32Fs::mount(dev).expect("Failed to mount FAT32"));

    kprintln!("VFS initialized");

    // Setup Interrupt Handlers
    let timer_irq = Platform::timer_irq();
    handlers::register(timer_irq, handlers::timer);
    kprintln!("Registered timer interrupt handler (IRQ {})", timer_irq);

    // Enable Interrupts
    Platform::enable_irq(timer_irq);
    kprintln!("Enabled timer interrupt");

    crate::arch::arm::interrupt::enable();
    kprintln!("Enabled CPU interrupts");

    if let Ok(dir) = vfs().ls("/") {
        kprintln!("Root directory contents:");
        for entry in dir {
            kprintln!(" - {}", entry);
        }
    } else {
        kprintln!("Failed to read root directory");
    }

    let test = vfs().open("/test.txt").unwrap();

    let mut buffer = [0u8; 64];
    test.seek(crate::fs::file::SeekWhence::Start, 0).unwrap();
    let bytes_read = test.read(&mut buffer, 0).unwrap();
    let content = core::str::from_utf8(&buffer[..bytes_read]).unwrap();
    kprintln!("Read {} bytes from /test.txt:", bytes_read);
    kprintln!("{}", content);

    // Start System Timer
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
