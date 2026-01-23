//! Hardware Abstraction Layer (HAL) - Platform-Independent Traits
//!
//! This module defines generic traits for interacting with hardware
//! peripherals. These traits are implemented by platform-specific
//! and peripheral drivers, allowing application code to be written
//! in a platform-independent manner.
//!
//! # Design Principles
//!
//! - **Zero-cost abstractions**: Traits compile to direct hardware access
//! - **Type safety**: Use associated types to catch errors at compile time
//! - **Flexibility**: Support diverse hardware capabilities
//! - **No platform leakage**: Traits must not reference platform-specific types
//!
//! # Available Interfaces
//!
//! - [`gpio`]: General Purpose Input/Output control
//! - [`serial`]: Serial port (UART) communication
//! - [`timer`]: Hardware timers and delays
//! - [`interrupt`]: Interrupt controller management
//! - [`block_device`]: Block storage device access

pub mod block_device;
pub mod framebuffer;
pub mod gpio;
pub mod interrupt;
pub mod serial;
pub mod timer;
