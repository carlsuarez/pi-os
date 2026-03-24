//! Kernel Main Entry Point
//!
#![no_std]
#![no_main]
#![feature(alloc_error_handler)]
#![allow(dead_code, unused_imports)]
extern crate alloc;

mod arch;
mod fs;
mod irq;
mod kcore;
mod mm;
mod process;
mod subsystems;
mod syscall;

use crate::arch::Irq;
use crate::fs::FileSystem;
use crate::fs::fat::fat32::*;
use crate::fs::fd::{AccessMode, FdFlags, FileDescriptorTable};
use crate::subsystems::print_devices;
use crate::{fs::vfs::vfs, irq::handlers};
use alloc::sync::Arc;
use common::sync::irq::IrqControl;
use core::panic::PanicInfo;
use drivers::hal::block_device::BlockDevice;
use drivers::platform::Platform;
use subsystems::device_manager;

// ============================================================================
// Kernel Entry Point
// ============================================================================

#[unsafe(no_mangle)]
pub extern "C" fn kernel_main() -> ! {
    kprintln!("========================================");
    kprintln!("Booting {} kernel", Platform::name());
    kprintln!("========================================");

    print_devices();

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
    loop {}
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
        Irq::wait_for_interrupt();
    }
}
