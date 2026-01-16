//! BCM2835 Platform Driver
//!
//! This module provides drivers for the Broadcom BCM2835 SoC
//! found in Raspberry Pi 1 and Zero.
//!
//! # Architecture
//!
//! The BCM2835 platform consists of:
//! - GPIO controller
//! - System timer with 4 compare channels
//! - Interrupt controller
//! - Various peripherals (UART, SPI, I2C, etc.)
//!
//! # Memory Map
//!
//! - Peripheral base: `0x2000_0000`
//! - GPIO base: `0x2020_0000`
//! - Timer base: `0x2000_3000`
//! - Interrupt controller base: `0x2000_b000`

pub mod gpio;
pub mod interrupt;
pub mod timer;
pub use gpio::Bcm2835Gpio;
pub use interrupt::Bcm2835InterruptController;
pub use timer::Bcm2835Timer;
pub mod framebuffer;
pub mod mailbox;

use super::{MemoryMap, Platform};
use crate::peripheral::pl011::PL011;
use crate::platform::bcm2835::timer::Channel;
use crate::{
    hal::{
        gpio::{GpioController, PullMode},
        interrupt::InterruptController,
        serial::{NonBlockingSerial, SerialConfig, SerialPort},
        timer::Timer,
    },
    platform::PlatformExt,
};
use common::arch::arm::bcm2835::irq::*;
use common::sync::SpinLock;

/// BCM2835 peripheral base address.
pub const PERIPHERAL_BASE: usize = 0x2000_0000;

/// BCM2835 platform (Raspberry Pi 1 / Zero)
pub struct Bcm2835Platform;

// ============================================================================
// Global Hardware Instances
// ============================================================================

/// Interrupt controller instance
static INTERRUPT_CONTROLLER: SpinLock<Option<Bcm2835InterruptController>> = SpinLock::new(None);

/// System timer instance
static TIMER: SpinLock<Option<Bcm2835Timer>> = SpinLock::new(None);

/// Console UART instance
static CONSOLE: SpinLock<Option<PL011>> = SpinLock::new(None);

// ============================================================================
// Platform Implementation
// ============================================================================

impl Platform for Bcm2835Platform {
    fn name() -> &'static str {
        "BCM2835 (Raspberry Pi 1/Zero)"
    }

    unsafe fn early_init() {
        // Configure GPIO pins for UART0
        let mut gpio = unsafe { Bcm2835Gpio::new() };

        // UART0 TX = GPIO 14, Alt Function 0
        gpio.set_alt_function(14, 0).ok();
        gpio.set_pull(14, PullMode::None).ok();

        // UART0 RX = GPIO 15, Alt Function 0
        gpio.set_alt_function(15, 0).ok();
        gpio.set_pull(15, PullMode::Up).ok();
    }

    fn memory_map() -> MemoryMap {
        MemoryMap {
            ram_start: 0x0000_0000,
            ram_size: 512 * 1024 * 1024, // Default 512MB
            peripheral_base: PERIPHERAL_BASE,
            peripheral_size: 16 * 1024 * 1024, // 16MB
            kernel_start: 0x8000,
        }
    }

    unsafe fn query_ram_size() -> Option<(usize, usize)> {
        unsafe { mailbox::get_arm_memory() }
    }

    unsafe fn init_console(baud_rate: u32) -> Result<(), &'static str> {
        let mut uart = unsafe { PL011::new(0x2020_1000) };

        uart.configure(SerialConfig::new_8n1(baud_rate))
            .map_err(|_| "Failed to configure UART")?;

        *CONSOLE.lock() = Some(uart);
        Ok(())
    }

    fn console_write(s: &str) {
        if let Some(ref mut uart) = *CONSOLE.lock() {
            uart.write(s.as_bytes()).ok();
        }
    }

    fn console_read() -> u8 {
        if let Some(ref mut uart) = *CONSOLE.lock() {
            uart.read_byte().unwrap_or(0)
        } else {
            0
        }
    }

    fn console_read_nonblocking() -> Option<u8> {
        if let Some(ref mut uart) = *CONSOLE.lock() {
            uart.try_read_byte().ok()
        } else {
            None
        }
    }

    unsafe fn init_interrupts() {
        let intc = unsafe { Bcm2835InterruptController::new() };
        *INTERRUPT_CONTROLLER.lock() = Some(intc);
    }

    fn enable_irq(irq: u32) {
        if let Some(ref mut intc) = *INTERRUPT_CONTROLLER.lock() {
            intc.enable(irq).ok();
        }
    }

    fn disable_irq(irq: u32) {
        if let Some(ref mut intc) = *INTERRUPT_CONTROLLER.lock() {
            intc.disable(irq).ok();
        }
    }

    fn next_pending_irq() -> Option<u32> {
        if let Some(ref intc) = *INTERRUPT_CONTROLLER.lock() {
            intc.next_pending()
        } else {
            None
        }
    }

    unsafe fn init_timer() {
        let timer = unsafe { Bcm2835Timer::new() };
        *TIMER.lock() = Some(timer);
    }

    fn timer_start(interval_us: u32) {
        if let Some(ref mut timer) = *TIMER.lock() {
            timer.start(Channel::Channel1, interval_us).ok();
        }
    }

    fn timer_clear() {
        if let Some(ref mut timer) = *TIMER.lock() {
            timer.clear_interrupt(Channel::Channel1).ok();
        }
    }

    fn timer_irq() -> u32 {
        IRQ_SYSTEM_TIMER_1
    }
}

impl PlatformExt for Bcm2835Platform {
    fn with_uart<R>(index: usize, f: impl FnOnce(&mut PL011) -> R) -> Option<R> {
        match index {
            0 => Some(f(CONSOLE.lock().as_mut()?)),
            _ => None,
        }
    }
}
