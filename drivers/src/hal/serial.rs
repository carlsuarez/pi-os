//! Serial Port (UART) Hardware Abstraction Layer.
//!
//! This module defines platform-independent traits for serial communication.

use core::fmt;

/// Serial port configuration.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct SerialConfig {
    /// Baud rate in bits per second.
    pub baud_rate: u32,
    /// Number of data bits per frame.
    pub data_bits: DataBits,
    /// Parity checking mode.
    pub parity: Parity,
    /// Number of stop bits.
    pub stop_bits: StopBits,
}

impl SerialConfig {
    /// Create a standard 8N1 configuration at the specified baud rate.
    ///
    /// 8N1 means: 8 data bits, no parity, 1 stop bit.
    pub const fn new_8n1(baud_rate: u32) -> Self {
        Self {
            baud_rate,
            data_bits: DataBits::Eight,
            parity: Parity::None,
            stop_bits: StopBits::One,
        }
    }
}

impl Default for SerialConfig {
    /// Default configuration: 115200 baud, 8N1.
    fn default() -> Self {
        Self::new_8n1(115200)
    }
}

/// Number of data bits per frame.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum DataBits {
    Five,
    Six,
    Seven,
    Eight,
}

/// Parity mode.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Parity {
    /// No parity bit.
    None,
    /// Odd parity.
    Odd,
    /// Even parity.
    Even,
}

/// Number of stop bits.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum StopBits {
    /// One stop bit.
    One,
    /// Two stop bits.
    Two,
}

/// Serial port errors.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SerialError {
    /// Framing error (invalid stop bit).
    Framing,
    /// Parity error (parity check failed).
    Parity,
    /// Overrun error (data received faster than it could be read).
    Overrun,
    /// Break condition detected.
    Break,
    /// Operation would block but non-blocking mode was requested.
    WouldBlock,
    /// Invalid configuration parameter.
    InvalidConfig,
    /// Other platform-specific error.
    Other,
}

// ============================================================================
// Serial Port Trait
// ============================================================================

/// Serial port trait.
///
/// This trait provides the core interface for serial communication.
pub trait SerialPort: Send + Sync {
    /// Error type for this serial port implementation.
    type Error: core::fmt::Debug;

    /// Configure the serial port.
    ///
    /// This must be called before using the serial port.
    fn configure(&mut self, config: SerialConfig) -> Result<(), Self::Error>;

    /// Write a single byte (blocking).
    fn write_byte(&mut self, byte: u8) -> Result<(), Self::Error>;

    /// Write multiple bytes (blocking).
    fn write(&mut self, bytes: &[u8]) -> Result<usize, Self::Error> {
        for &byte in bytes {
            self.write_byte(byte)?;
        }
        Ok(bytes.len())
    }

    /// Read a single byte (blocking).
    fn read_byte(&mut self) -> Result<u8, Self::Error>;

    /// Read multiple bytes (blocking).
    fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        for byte in buffer.iter_mut() {
            *byte = self.read_byte()?;
        }
        Ok(buffer.len())
    }

    /// Flush the write buffer.
    fn flush(&mut self) -> Result<(), Self::Error>;

    /// Check if the serial port is busy transmitting.
    fn is_busy(&self) -> bool;
}

// ============================================================================
// Non-Blocking Extension
// ============================================================================

/// Extension trait for non-blocking operations.
pub trait NonBlockingSerial: SerialPort {
    /// Try to write a byte without blocking.
    fn try_write_byte(&mut self, byte: u8) -> Result<(), Self::Error>;

    /// Try to write multiple bytes without blocking
    fn try_write(&mut self, bytes: &[u8]) -> Result<usize, Self::Error> {
        for &byte in bytes {
            self.try_write_byte(byte)?;
        }
        Ok(bytes.len())
    }

    /// Try to read a byte without blocking.
    fn try_read_byte(&mut self) -> Result<u8, Self::Error>;

    /// Try to read multiple bytes without blocking
    fn try_read(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        for byte in buffer.iter_mut() {
            *byte = self.try_read_byte()?;
        }
        Ok(buffer.len())
    }
}

// ============================================================================
// Type-Erased Serial Port
// ============================================================================

/// Type-erased serial port trait using `SerialError`.
pub trait DynSerialPort: Send + Sync {
    fn configure(&mut self, config: SerialConfig) -> Result<(), SerialError>;
    fn write_byte(&mut self, byte: u8) -> Result<(), SerialError>;
    fn write(&mut self, bytes: &[u8]) -> Result<usize, SerialError> {
        for &byte in bytes {
            self.write_byte(byte)?;
        }
        Ok(bytes.len())
    }
    fn read_byte(&mut self) -> Result<u8, SerialError>;
    fn read(&mut self, buffer: &mut [u8]) -> Result<usize, SerialError> {
        for byte in buffer.iter_mut() {
            *byte = self.read_byte()?;
        }
        Ok(buffer.len())
    }
    fn flush(&mut self) -> Result<(), SerialError>;
    fn is_busy(&self) -> bool;
    fn as_dyn_nonblocking(&mut self) -> Option<&mut dyn DynNonBlockingSerial> {
        None
    }
}

/// Type-erased non-blocking serial port trait using `SerialError`.
pub trait DynNonBlockingSerial: DynSerialPort {
    fn try_write_byte(&mut self, byte: u8) -> Result<(), SerialError>;
    fn try_write(&mut self, bytes: &[u8]) -> Result<usize, SerialError> {
        for &byte in bytes {
            self.try_write_byte(byte)?;
        }
        Ok(bytes.len())
    }
    fn try_read_byte(&mut self) -> Result<u8, SerialError>;
    fn try_read(&mut self, buffer: &mut [u8]) -> Result<usize, SerialError> {
        for byte in buffer.iter_mut() {
            *byte = self.try_read_byte()?;
        }
        Ok(buffer.len())
    }
}

// ============================================================================
// core::fmt::Write Implementation
// ============================================================================

/// Implement core::fmt::Write for any DynSerialPort
impl fmt::Write for dyn DynSerialPort {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            // Convert line endings
            if byte == b'\n' {
                self.write_byte(b'\r').map_err(|_| fmt::Error)?;
            }
            self.write_byte(byte).map_err(|_| fmt::Error)?;
        }
        Ok(())
    }
}

/// Wrapper type to implement core::fmt::Write for SerialPort types.
/// This allows using write!/writeln! macros.
pub struct SerialWriter<T: SerialPort>(pub T);

impl<T> fmt::Write for SerialWriter<T>
where
    T: SerialPort,
{
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            // Convert line endings
            if byte == b'\n' {
                self.0.write_byte(b'\r').map_err(|_| fmt::Error)?;
            }
            self.0.write_byte(byte).map_err(|_| fmt::Error)?;
        }
        Ok(())
    }
}
