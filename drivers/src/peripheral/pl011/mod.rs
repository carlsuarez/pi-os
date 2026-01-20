//! ARM PrimeCell PL011 UART Driver
//!
//! This module provides a driver for the ARM PL011 UART peripheral,
//! which is commonly found in ARM-based systems.
//!
//! # Features
//!
//! - Configurable baud rate
//! - 8N1 configuration (8 data bits, no parity, 1 stop bit)
//! - FIFO support
//! - Blocking and non-blocking I/O
//!
//! # Example
//!
//! ```no_run
//! use drivers::peripheral::pl011::PL011;
//! use drivers::hal::serial::{SerialPort, SerialConfig};
//!
//! unsafe {
//!     let mut uart = PL011::new(0x2020_1000);
//!     uart.configure(SerialConfig::new_8n1(115200)).unwrap();
//!     uart.write(b"Hello, world!\n").unwrap();
//! }
//! ```

use crate::hal::serial::{
    DataBits, NonBlockingSerial, Parity, SerialConfig, SerialError, SerialPort, StopBits,
};
use core::ptr::{read_volatile, write_volatile};

/// PL011 clock frequency
const PL011_CLOCK_HZ: u32 = 48_000_000;

// Register offsets
const FR_OFFSET: usize = 0x18;
const IBRD_OFFSET: usize = 0x24;
const FBRD_OFFSET: usize = 0x28;
const LCRH_OFFSET: usize = 0x2C;
const CR_OFFSET: usize = 0x30;
const IMSC_OFFSET: usize = 0x38;
const ICR_OFFSET: usize = 0x44;

// Flag Register (FR) bits
const FR_BUSY: u32 = 1 << 3;
const FR_RXFE: u32 = 1 << 4;
const FR_TXFF: u32 = 1 << 5;

// Control Register (CR) bits
const CR_UARTEN: u32 = 1 << 0;
const CR_TXE: u32 = 1 << 8;
const CR_RXE: u32 = 1 << 9;

// Line Control Register (LCRH) bits
const LCRH_WLEN_8: u32 = 0b11 << 5;
const LCRH_FEN: u32 = 1 << 4;

/// PL011 UART driver.
pub struct PL011 {
    base: usize,
}

impl PL011 {
    /// Create a new PL011 UART instance.
    ///
    /// # Safety
    ///
    /// - `base` must point to a valid PL011 peripheral
    /// - Only one instance should exist per UART hardware
    /// - Memory must be properly mapped as device memory
    pub const unsafe fn new(base: usize) -> Self {
        Self { base }
    }

    #[inline]
    fn read_reg(&self, offset: usize) -> u32 {
        unsafe { read_volatile((self.base + offset) as *const u32) }
    }

    #[inline]
    fn write_reg(&mut self, offset: usize, value: u32) {
        unsafe { write_volatile((self.base + offset) as *mut u32, value) }
    }

    /// Wait for the UART to finish transmitting.
    fn wait_idle(&self) {
        while self.read_reg(FR_OFFSET) & FR_BUSY != 0 {
            core::hint::spin_loop();
        }
    }

    /// Calculate baud rate divisors.
    fn calculate_divisors(baud_rate: u32) -> Result<(u32, u32), SerialError> {
        if baud_rate == 0 {
            return Err(SerialError::InvalidConfig);
        }

        // BAUDDIV = (FUARTCLK / (16 × Baud rate))
        let divisor = ((PL011_CLOCK_HZ as u64) << 6) / (16 * baud_rate as u64);

        let integer = (divisor >> 6) as u32;
        let fractional = (divisor & 0x3F) as u32;

        if integer == 0 || integer > 0xFFFF {
            return Err(SerialError::InvalidConfig);
        }

        Ok((integer, fractional))
    }
}

// ============================================================================
// HAL Implementation
// ============================================================================

impl SerialPort for PL011 {
    fn configure(&mut self, config: SerialConfig) -> Result<(), SerialError> {
        // Validate configuration
        if !matches!(config.data_bits, DataBits::Eight) {
            return Err(SerialError::InvalidConfig);
        }

        if !matches!(config.parity, Parity::None) {
            return Err(SerialError::InvalidConfig);
        }

        if !matches!(config.stop_bits, StopBits::One) {
            return Err(SerialError::InvalidConfig);
        }

        // Disable UART
        let mut cr = self.read_reg(CR_OFFSET);
        cr &= !CR_UARTEN;
        self.write_reg(CR_OFFSET, cr);

        // Wait for any transmission to complete
        self.wait_idle();

        // Flush FIFOs
        let mut lcrh = self.read_reg(LCRH_OFFSET);
        lcrh &= !LCRH_FEN;
        self.write_reg(LCRH_OFFSET, lcrh);

        // Calculate and set baud rate divisors
        let (ibrd, fbrd) = Self::calculate_divisors(config.baud_rate)?;
        self.write_reg(IBRD_OFFSET, ibrd);
        self.write_reg(FBRD_OFFSET, fbrd);

        // Configure line control: 8N1 with FIFOs enabled
        self.write_reg(LCRH_OFFSET, LCRH_WLEN_8 | LCRH_FEN);

        // Clear all pending interrupts
        self.write_reg(ICR_OFFSET, 0x07FF);

        // Disable all interrupts
        self.write_reg(IMSC_OFFSET, 0);

        // Enable UART, transmitter, and receiver
        self.write_reg(CR_OFFSET, CR_UARTEN | CR_TXE | CR_RXE);

        Ok(())
    }

    fn write_byte(&mut self, byte: u8) -> Result<(), SerialError> {
        // Wait for TX FIFO to have space
        while self.read_reg(FR_OFFSET) & FR_TXFF != 0 {
            core::hint::spin_loop();
        }

        self.write_reg(0x00, byte as u32);
        Ok(())
    }

    fn read_byte(&mut self) -> Result<u8, SerialError> {
        // Wait for data to be available
        while self.read_reg(FR_OFFSET) & FR_RXFE != 0 {
            core::hint::spin_loop();
        }

        Ok((self.read_reg(0x00) & 0xFF) as u8)
    }

    fn flush(&mut self) -> Result<(), SerialError> {
        self.wait_idle();
        Ok(())
    }

    fn is_busy(&self) -> bool {
        self.read_reg(FR_OFFSET) & FR_BUSY != 0
    }

    fn as_nonblocking(&mut self) -> Option<&mut dyn NonBlockingSerial> {
        Some(self)
    }
}

impl NonBlockingSerial for PL011 {
    fn try_write_byte(&mut self, byte: u8) -> Result<(), SerialError> {
        if self.read_reg(FR_OFFSET) & FR_TXFF != 0 {
            return Err(SerialError::WouldBlock);
        }

        self.write_reg(0x00, byte as u32);
        Ok(())
    }

    fn try_read_byte(&mut self) -> Result<u8, SerialError> {
        if self.read_reg(FR_OFFSET) & FR_RXFE != 0 {
            return Err(SerialError::WouldBlock);
        }

        Ok((self.read_reg(0x00) & 0xFF) as u8)
    }
}

// SAFETY: PL011 wraps memory-mapped hardware that can be safely
// accessed from any thread when protected by synchronization.
unsafe impl Send for PL011 {}
unsafe impl Sync for PL011 {}

pub use PL011 as Pl011;
