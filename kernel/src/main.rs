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
use crate::fs::fd::{AccessMode, Fd, FdFlags, FileDescriptorTable};
use crate::{fs::vfs::vfs, irq::handlers};
use alloc::sync::Arc;
use core::panic::PanicInfo;
use drivers::console::console_write;
use drivers::device_manager::devices;
use drivers::hal::block_device::BlockDevice;
use drivers::kprint;
use drivers::kprintln;
use drivers::platform::{CurrentPlatform as Platform, Platform as PlatformTrait};

// ============================================================================
// Kernel Entry Point
// ============================================================================

#[unsafe(no_mangle)]
pub extern "C" fn kernel_main() -> ! {
    kprintln!("========================================");
    kprintln!("Booting {} kernel", Platform::name());
    kprintln!("========================================");

    // ------------------------------------------------------------------------
    // Fetch block device from DeviceManager
    // ------------------------------------------------------------------------
    let dev: Arc<dyn BlockDevice> = {
        let mgr = devices().lock();
        mgr.block("emmc0").expect("EMMC not registered")
    };

    // ------------------------------------------------------------------------
    // Mount filesystem
    // ------------------------------------------------------------------------
    vfs().init(Fat32Fs::mount(dev).expect("Failed to mount FAT32"));
    kprintln!("VFS initialized");

    // ------------------------------------------------------------------------
    // Interrupts
    // ------------------------------------------------------------------------
    let timer_irq = Platform::timer_irq();
    handlers::register(timer_irq, handlers::timer);
    Platform::enable_irq(timer_irq);
    crate::arch::arm::interrupt::enable();

    kprintln!("Timer IRQ {} enabled", timer_irq);

    // ------------------------------------------------------------------------
    // Directory test
    // ------------------------------------------------------------------------
    let dir = vfs().ls("/").expect("Failed to read root directory");
    kprintln!("Root directory:");
    for entry in dir {
        kprintln!(" - {}", entry);
    }

    // ------------------------------------------------------------------------
    // FD table test
    // ------------------------------------------------------------------------
    kprintln!("\n--- Testing FileDescriptorTable ---");

    let mut fd_table = FileDescriptorTable::new();
    let file = vfs().open("/test.txt").expect("Failed to open /test.txt");

    let access = AccessMode {
        read: true,
        write: false,
        append: false,
    };
    let fd = fd_table.alloc(file, FdFlags::NONE, access).unwrap();

    let mut buf = [0u8; 64];
    let n = fd_table.get_mut(fd).unwrap().read(&mut buf).unwrap();

    if let Ok(s) = core::str::from_utf8(&buf[..n]) {
        kprintln!("Read from /test.txt: {}", s);
    }

    fd_table.close(fd).unwrap();

    // ------------------------------------------------------------------------
    // Start timer
    // ------------------------------------------------------------------------
    Platform::timer_start(1_000_000);
    kprintln!("System timer started");

    // ------------------------------------------------------------------------
    // Ready
    // ------------------------------------------------------------------------
    kprintln!("========================================");
    kprintln!("Kernel initialization complete");
    kprintln!("Entering main loop");
    kprintln!("========================================");

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
