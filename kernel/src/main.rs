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
    kprintln!("Booting {} kernel", Platform::name());

    print_devices();

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
    // Direct VGA write — works before any subsystem is initialized
    #[cfg(target_arch = "x86")]
    {
        use core::fmt::Write;

        struct VgaPanic {
            col: usize,
        }
        impl core::fmt::Write for VgaPanic {
            fn write_str(&mut self, s: &str) -> core::fmt::Result {
                let vga = 0xb8000 as *mut u16;
                for byte in s.bytes() {
                    if self.col < 80 * 25 {
                        unsafe { vga.add(self.col).write_volatile(0x0f00 | byte as u16) };
                        self.col += 1;
                    }
                }
                Ok(())
            }
        }

        let _ = write!(VgaPanic { col: 0 }, "PANIC: {}", info);
    }

    loop {
        core::hint::spin_loop();
    }
}
