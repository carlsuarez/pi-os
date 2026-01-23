//! Platform Abstraction Layer
//!
//! This module provides a platform-agnostic interface for hardware access.
//! Each platform (BCM2835, BCM2711, etc.) implements the Platform trait.
//!
//! # Usage
//!
//! ```rust
//! use crate::platform::{CurrentPlatform as Platform, Platform as PlatformTrait};
//!
//! // Initialize platform
//! unsafe { Platform::early_init(); }
//!
//! // Use platform services
//! Platform::enable_irq(timer_irq);
//! Platform::timer_start(1_000_000);
//! ```

use crate::device_manager::DeviceManager;

/// Platform memory map information
#[derive(Debug, Clone, Copy)]
pub struct MemoryMap {
    /// Start of RAM
    pub ram_start: usize,
    /// Size of RAM in bytes (may be default, query actual with query_ram_size)
    pub ram_size: usize,
    /// Start of peripheral region
    pub peripheral_base: usize,
    /// Size of peripheral region
    pub peripheral_size: usize,
    /// Kernel load address
    pub kernel_start: usize,
}

/// Platform trait defining required platform-specific functionality.
pub trait Platform {
    /// Platform name for debugging
    fn name() -> &'static str;

    /// Early platform initialization (GPIO, clocks, etc.)
    ///
    /// # Safety
    /// Must only be called once, very early in boot.
    unsafe fn early_init();

    /// Get static memory map for this platform
    fn memory_map() -> MemoryMap;

    /// Query actual RAM size from firmware/hardware
    ///
    /// # Safety
    /// Must only be called after `early_init()`.
    unsafe fn query_ram_size() -> Option<(usize, usize)>;

    /// Initialize and register all platform devices
    ///
    /// This replaces init_console, init_interrupts, init_timer, init_block_devices
    ///
    /// # Safety
    /// Must only be called once after `early_init()`.
    unsafe fn init_devices(device_mgr: &mut DeviceManager) -> Result<(), &'static str>;

    /// Platform-specific interrupt handling
    fn next_pending_irq() -> Option<u32>;
    fn enable_irq(irq: u32);
    fn disable_irq(irq: u32);

    /// Platform-specific timer control
    fn timer_start(interval_us: u32);
    fn timer_clear();
    fn timer_irq() -> u32;
}

// Platform selection based on Cargo features
cfg_if::cfg_if! {
    if #[cfg(feature = "bcm2835")] {
        pub mod bcm2835;
        pub use bcm2835::Bcm2835Platform as CurrentPlatform;
    } else if #[cfg(feature = "bcm2711")] {
        mod bcm2711;
        pub use bcm2711::Bcm2711Platform as CurrentPlatform;
    } else {
        compile_error!(
            "No platform selected!\n\
            Use: cargo build --features bcm2835\n\
            Or:  cargo build --features bcm2711"
        );
    }
}

// Ensure only one platform is selected
#[cfg(all(feature = "bcm2835", feature = "bcm2711"))]
compile_error!("Multiple platforms selected! Choose only one: bcm2835 OR bcm2711");
