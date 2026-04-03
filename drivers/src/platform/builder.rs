//! [`PlatformBuilder`] — the write side of the platform layer.
//!
//! `kernel::boot` uses this to populate the static tables that
//! [`super::Platform`] exposes for reading.  Nothing in `drivers`
//! itself calls these methods; they are `pub` so `kernel` (a separate
//! crate that depends on `drivers`) can reach them.

use super::{
    ARCH, Architecture, CMDLINE, DEVICE_COUNT, DEVICES, DeviceInfo, INITIALIZED, MAX_DEVICES,
    MAX_MEMORY_REGIONS, MEMORY_REGION_COUNT, MEMORY_REGIONS, MemoryRegion, MemoryType,
    PLATFORM_NAME,
};
use core::sync::atomic::Ordering;

pub struct PlatformBuilder;

impl PlatformBuilder {
    /// Mark the platform as initialized.
    ///
    /// Returns `Err` if called more than once.
    pub fn begin() -> Result<(), &'static str> {
        if INITIALIZED.swap(true, Ordering::SeqCst) {
            return Err("Platform already initialized");
        }
        Ok(())
    }

    pub fn set_arch(arch: Architecture) {
        unsafe {
            ARCH = arch;
        }
    }

    pub fn set_platform_name(name: &'static str) {
        unsafe {
            PLATFORM_NAME = name;
        }
    }

    pub fn set_cmdline(cmdline: &'static str) {
        unsafe {
            CMDLINE = Some(cmdline);
        }
    }

    /// Add a discovered device to the platform device table.
    ///
    /// Silently drops entries beyond [`MAX_DEVICES`].
    pub fn add_device(device: DeviceInfo) {
        unsafe {
            if DEVICE_COUNT < MAX_DEVICES {
                DEVICES[DEVICE_COUNT] = Some(device);
                DEVICE_COUNT += 1;
            }
        }
    }

    /// Add a discovered memory region to the platform memory map.
    ///
    /// Silently drops entries beyond [`MAX_MEMORY_REGIONS`].
    pub fn add_memory_region(region: MemoryRegion) {
        unsafe {
            if MEMORY_REGION_COUNT < MAX_MEMORY_REGIONS {
                MEMORY_REGIONS[MEMORY_REGION_COUNT] = region;
                MEMORY_REGION_COUNT += 1;
            }
        }
    }

    /// Convenience: add an MMIO-typed region.
    pub fn add_mmio_region(base: usize, size: usize) {
        Self::add_memory_region(MemoryRegion {
            base,
            size,
            mem_type: MemoryType::Mmio,
        });
    }

    /// Convenience: add an Available RAM region.
    pub fn add_ram_region(base: usize, size: usize) {
        Self::add_memory_region(MemoryRegion {
            base,
            size,
            mem_type: MemoryType::Available,
        });
    }
}
