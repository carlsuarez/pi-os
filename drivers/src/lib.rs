//! Hardware Driver Subsystem
//!
//! This module provides a layered architecture for hardware abstraction:
//!
//! # Module Organization
//!
//! - [`hal`]: Platform-independent trait definitions
//! - [`platform`]: Platform-specific drivers (SoC level)
//! - [`peripheral`]: Reusable peripheral drivers
//!
//! # Design Principles
//!
//! 1. **Separation of Concerns**: Platform code is separate from peripheral code
//! 2. **Zero-Cost Abstractions**: HAL traits compile to direct hardware access
//! 3. **Type Safety**: Use the type system to prevent errors at compile time
//! 4. **Reusability**: Peripheral drivers work across different platforms
//! 5. **Clear Ownership**: Each driver has one clear purpose
//!
//! # Usage Example
//!
//! ```no_run
//! use drivers::hal::serial::SerialPort;
//! use drivers::peripheral::pl011::PL011;
//!
//! unsafe {
//!     let mut uart = PL011::new(0x2020_1000);
//!     uart.configure(SerialConfig::default())?;
//!     uart.write(b"Hello, world!\n")?;
//! }
//! ```

#![no_std]
#![allow(dead_code)]

pub mod hal;
pub mod peripheral;
pub mod platform;

// Re-export commonly used types
pub use hal::gpio::{GpioController, PinLevel};
pub use hal::interrupt::InterruptController;
pub use hal::serial::{SerialConfig, SerialPort};
pub use hal::timer::Timer;

extern crate alloc;
