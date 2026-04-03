//! Platform Abstraction Layer — read-only query API.
//!
//! This module exposes the hardware inventory that the boot layer
//! (`kernel::boot`) discovered and wrote via [`PlatformBuilder`].
//! Drivers and the rest of the kernel only ever *read* from here.
//!
//! # Crate boundary
//!
//! `drivers` → read-only ([`Platform`], [`DeviceInfo`], [`MemoryRegion`], …)
//! `kernel`  → write side ([`PlatformBuilder`]) + all boot/parse logic

pub mod builder;

use core::sync::atomic::{AtomicBool, Ordering};

use alloc::{format, string::String};
// Re-export
pub use builder::PlatformBuilder;

//  Public types

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryType {
    Available,
    Reserved,
    Mmio,
    Kernel,
    Framebuffer,
}

#[derive(Debug, Clone, Copy)]
pub struct MemoryRegion {
    pub base: usize,
    pub size: usize,
    pub mem_type: MemoryType,
}

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub name: &'static str,
    pub compatible: &'static str,
    pub base_addr: usize,
    pub size: usize,
    pub irq: Option<u32>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Architecture {
    X86,
    X86_64,
    Arm,
    AArch64,
}

/// Structured memory map summary for the kernel memory manager.
#[derive(Debug, Clone, Copy)]
pub struct MemoryMap {
    pub ram_start: usize,
    pub ram_size: usize,
    pub peripheral_base: usize,
    pub peripheral_size: usize,
}

//  Static storage (written once by PlatformBuilder, read-only after)

pub(crate) const MAX_DEVICES: usize = 32;
pub(crate) const MAX_MEMORY_REGIONS: usize = 64;

pub(crate) static mut MEMORY_REGIONS: [MemoryRegion; MAX_MEMORY_REGIONS] = [MemoryRegion {
    base: 0,
    size: 0,
    mem_type: MemoryType::Reserved,
}; MAX_MEMORY_REGIONS];
pub(crate) static mut MEMORY_REGION_COUNT: usize = 0;

pub(crate) static mut DEVICES: [Option<DeviceInfo>; MAX_DEVICES] = [const { None }; MAX_DEVICES];
pub(crate) static mut DEVICE_COUNT: usize = 0;

pub(crate) static mut CMDLINE: Option<&'static str> = None;
pub(crate) static mut PLATFORM_NAME: &'static str = "Unknown";
pub(crate) static mut ARCH: Architecture = Architecture::X86;

pub(crate) static INITIALIZED: AtomicBool = AtomicBool::new(false);

//  Platform — query API

pub struct Platform;

impl Platform {
    pub fn name() -> &'static str {
        unsafe { PLATFORM_NAME }
    }

    pub fn arch() -> &'static str {
        match unsafe { ARCH } {
            Architecture::X86 => "x86",
            Architecture::X86_64 => "x86_64",
            Architecture::Arm => "arm",
            Architecture::AArch64 => "aarch64",
        }
    }

    pub fn cmdline() -> Option<&'static str> {
        unsafe { CMDLINE }
    }

    pub fn memory_regions() -> &'static [MemoryRegion] {
        unsafe { &MEMORY_REGIONS[..MEMORY_REGION_COUNT] }
    }

    pub fn total_ram() -> usize {
        Self::memory_regions()
            .iter()
            .filter(|r| r.mem_type == MemoryType::Available)
            .map(|r| r.size)
            .sum()
    }

    pub fn memory_map() -> MemoryMap {
        let ram = Self::memory_regions()
            .iter()
            .filter(|r| r.mem_type == MemoryType::Available)
            .max_by_key(|r| r.size)
            .expect("no available RAM region found");

        let periph_base = Self::memory_regions()
            .iter()
            .filter(|r| r.mem_type == MemoryType::Mmio)
            .map(|r| r.base)
            .min();

        let periph_end = Self::memory_regions()
            .iter()
            .filter(|r| r.mem_type == MemoryType::Mmio)
            .map(|r| r.base + r.size)
            .max();

        let (peripheral_base, peripheral_size) = match (periph_base, periph_end) {
            (Some(base), Some(end)) => (base, end.saturating_sub(base)),
            _ => (0, 0),
        };

        MemoryMap {
            ram_start: ram.base,
            ram_size: ram.size,
            peripheral_base,
            peripheral_size,
        }
    }

    pub fn find_device(name_or_compat: &str) -> Option<&'static DeviceInfo> {
        unsafe {
            DEVICES[..DEVICE_COUNT]
                .iter()
                .filter_map(|d| d.as_ref())
                .find(|d| d.name == name_or_compat || d.compatible.contains(name_or_compat))
        }
    }

    pub fn devices() -> impl Iterator<Item = &'static DeviceInfo> + 'static {
        unsafe { (0..DEVICE_COUNT).filter_map(|i| DEVICES[i].as_ref()) }
    }

    /// Initialize and register all platform devices with the device manager.
    ///
    /// # Safety
    /// Must be called after `PlatformBuilder::begin()` and after memory
    /// management is initialized.
    pub unsafe fn init_devices(
        device_mgr: &mut crate::device_manager::DeviceManager,
    ) -> Result<(), String> {
        use crate::peripheral::x86::mb2fb::{MB2_FB_TAG, Mb2Fb};
        use crate::peripheral::*;

        unsafe {
            for device in Self::devices() {
                match device.compatible {
                    //  UART
                    "arm,pl011" | "arm,primecell" => {
                        let uart = arm::pl011::Pl011::new(device.base_addr);
                        device_mgr.register_serial(device.name, uart)?;
                    }

                    "16550a-uart" | "ns16550a" => {
                        #[cfg(target_arch = "x86")]
                        let uart =
                            x86::uart16550::Uart16550::<x86::uart16550::Pio>::new(device.base_addr);
                        #[cfg(not(target_arch = "x86"))]
                        let uart = x86::uart16550::Uart16550::<x86::uart16550::Mmio>::new(
                            device.base_addr,
                        );
                        device_mgr.register_serial(device.name, uart)?;
                    }

                    //  Timers
                    "brcm,bcm2835-system-timer" => {
                        let timer = bcm2835::timer::Bcm2835Timer::new(device.base_addr)
                            .map_err(|e| format!("Timer init failed: {:?}", e))?;
                        device_mgr.register_timer(device.name, timer, Some(1))?;
                    }
                    "arm,armv7-timer" | "arm,armv8-timer" => {}
                    "i8254-pit" | "intel,8254" => {}

                    //  Interrupt controllers
                    "brcm,bcm2835-armctrl-ic" | "brcm,bcm2836-armctrl-ic" => {
                        let intc = bcm2835::intc::Bcm2835InterruptController::new(device.base_addr);
                        device_mgr.register_interrupt_controller(device.name, intc)?;
                    }
                    "arm,gic-400" | "arm,cortex-a15-gic" | "arm,gic-v3" => {}
                    "i8259-pic" | "intel,8259" => {}

                    //  Framebuffer
                    "multiboot2-fb" | "simple-framebuffer" => {
                        // Ignore for early boot, Mb2Fb will consume the MB2_FB_TAG directly during its own init.
                    }

                    //  Block devices
                    "brcm,bcm2835-sdhost" | "brcm,bcm2711-emmc2" => {
                        let block_dev = bcm2835::emmc::Emmc::new(device.base_addr)
                            .map_err(|e| format!("Emmc init failed: {:?}", e))?;
                        device_mgr.register_block(device.name, block_dev)?;
                    }

                    //  Consoles
                    "vga-text" => {
                        // VGA text console is initialized in subsystems::init — no
                        // device manager registration needed here.
                    }

                    _ => {
                        log::warn!(
                            "Unknown device '{}' (compatible: '{}') at {:#x} (size: {:#x})",
                            device.name,
                            device.compatible,
                            device.base_addr,
                            device.size
                        );
                    }
                }
            }
        }

        Ok(())
    }
}
