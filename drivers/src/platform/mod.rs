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

use crate::hal::serial::SerialPort;

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

/// Platform trait - implemented by each supported platform
pub trait Platform {
    /// Platform name for debugging
    fn name() -> &'static str;

    /// Early platform initialization
    ///
    /// Called before any other initialization, including heap.
    /// Should configure GPIO, clocks, etc.
    ///
    /// # Safety
    /// Must only be called once, very early in boot.
    unsafe fn early_init();

    /// Get static memory map for this platform
    ///
    /// Returns default values. Use `query_ram_size()` for actual RAM.
    fn memory_map() -> MemoryMap;

    /// Query actual RAM size from firmware/hardware
    ///
    /// Returns `(base, size)` in bytes.
    ///
    /// # Safety
    /// Must only be called after `early_init()`.
    unsafe fn query_ram_size() -> Option<(usize, usize)>;

    /// Initialize console for early debugging
    ///
    /// # Safety
    /// Must only be called after `early_init()`.
    unsafe fn init_console(baud_rate: u32) -> Result<(), &'static str>;

    /// Write string to console (blocking)
    fn console_write(s: &str);

    /// Read a character from console (blocking)
    fn console_read() -> u8;

    /// Read a character from console (non-blocking)
    fn console_read_nonblocking() -> Option<u8>;

    /// Initialize interrupt controller
    ///
    /// # Safety
    /// Must only be called once.
    unsafe fn init_interrupts();

    /// Enable (unmask) an IRQ line
    fn enable_irq(irq: u32);

    /// Disable (mask) an IRQ line
    fn disable_irq(irq: u32);

    /// Get next pending interrupt
    ///
    /// Returns the IRQ number of the highest-priority pending interrupt,
    /// or None if no interrupts are pending.
    fn next_pending_irq() -> Option<u32>;

    /// Initialize system timer
    ///
    /// # Safety
    /// Must only be called once.
    unsafe fn init_timer();

    /// Start timer with given interval
    ///
    /// # Arguments
    /// - `interval_us`: Interval in microseconds
    fn timer_start(interval_us: u32);

    /// Clear timer interrupt
    fn timer_clear();

    /// Get timer IRQ number
    fn timer_irq() -> u32;

    /// Initialize block devices
    /// # Safety
    /// Must only be called once.
    unsafe fn init_block_devices() -> Result<(), &'static str>;

    /// Access a UART by index
    ///
    /// Executes the closure with mutable access to the specified UART.
    /// Returns None if the UART index is invalid.
    ///
    /// # Arguments
    /// - `index`: UART index (0 = primary UART, 1+ = auxiliary UARTs if available)
    /// - `f`: Closure that receives mutable reference to the UART
    ///
    /// # Safety
    /// Caller must ensure the UART is properly initialized before use.
    fn with_uart<R>(index: usize, f: impl FnOnce(&mut dyn SerialPort) -> R) -> Option<R>;
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
