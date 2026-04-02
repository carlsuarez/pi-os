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
    type Error: core::fmt::Debug + Into<SerialError>;

    fn configure(&mut self, config: SerialConfig) -> Result<(), Self::Error>;
    fn write_byte(&mut self, byte: u8) -> Result<(), Self::Error>;
    fn read_byte(&mut self) -> Result<u8, Self::Error>;
    fn flush(&mut self) -> Result<(), Self::Error>;
    fn is_busy(&self) -> bool;

    /// Write multiple bytes (blocking). Default impl calls write_byte.
    fn write(&mut self, bytes: &[u8]) -> Result<usize, Self::Error> {
        for &b in bytes {
            self.write_byte(b)?;
        }
        Ok(bytes.len())
    }

    /// Read multiple bytes (blocking). Default impl calls read_byte.
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        for slot in buf.iter_mut() {
            *slot = self.read_byte()?;
        }
        Ok(buf.len())
    }
}

// NonBlockingSerial: optional extension
pub trait NonBlockingSerial: SerialPort {
    fn try_write_byte(&mut self, byte: u8) -> Result<(), Self::Error>;
    fn try_read_byte(&mut self) -> Result<u8, Self::Error>;

    fn try_write(&mut self, bytes: &[u8]) -> Result<usize, Self::Error> {
        for &b in bytes {
            self.try_write_byte(b)?;
        }
        Ok(bytes.len())
    }
    fn try_read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        for slot in buf.iter_mut() {
            *slot = self.try_read_byte()?;
        }
        Ok(buf.len())
    }
}

// ============================================================================
// Type-Erased Serial Port
// ============================================================================

/// Type-erased serial port trait using `SerialError`.
pub trait DynSerialPort: Send + Sync {
    fn configure(&mut self, config: SerialConfig) -> Result<(), SerialError>;
    fn write_byte(&mut self, byte: u8) -> Result<(), SerialError>;
    fn write(&mut self, bytes: &[u8]) -> Result<usize, SerialError>;
    fn read_byte(&mut self) -> Result<u8, SerialError>;
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, SerialError>;
    fn flush(&mut self) -> Result<(), SerialError>;
    fn is_busy(&self) -> bool;

    fn as_nonblocking(&mut self) -> Option<&mut dyn DynNonBlockingSerial> {
        None
    }
}

/// Type-erased non-blocking serial port trait using `SerialError`.
pub trait DynNonBlockingSerial: DynSerialPort {
    fn try_write_byte(&mut self, byte: u8) -> Result<(), SerialError>;
    fn try_read_byte(&mut self) -> Result<u8, SerialError>;
    fn try_write(&mut self, bytes: &[u8]) -> Result<usize, SerialError>;
    fn try_read(&mut self, buf: &mut [u8]) -> Result<usize, SerialError>;
}

/// Blanket impl for types that implement SerialPort.
impl<T> DynSerialPort for T
where
    T: SerialPort,
{
    fn configure(&mut self, config: SerialConfig) -> Result<(), SerialError> {
        SerialPort::configure(self, config).map_err(Into::into)
    }
    fn write_byte(&mut self, byte: u8) -> Result<(), SerialError> {
        SerialPort::write_byte(self, byte).map_err(Into::into)
    }
    fn write(&mut self, bytes: &[u8]) -> Result<usize, SerialError> {
        SerialPort::write(self, bytes).map_err(Into::into)
    }
    fn read_byte(&mut self) -> Result<u8, SerialError> {
        SerialPort::read_byte(self).map_err(Into::into)
    }
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, SerialError> {
        SerialPort::read(self, buf).map_err(Into::into)
    }
    fn flush(&mut self) -> Result<(), SerialError> {
        SerialPort::flush(self).map_err(Into::into)
    }
    fn is_busy(&self) -> bool {
        SerialPort::is_busy(self)
    }
}

/// Blanket impl for types that implement both SerialPort and NonBlockingSerial.
impl<T> DynNonBlockingSerial for T
where
    T: NonBlockingSerial,
{
    fn try_write_byte(&mut self, byte: u8) -> Result<(), SerialError> {
        NonBlockingSerial::try_write_byte(self, byte).map_err(Into::into)
    }
    fn try_read_byte(&mut self) -> Result<u8, SerialError> {
        NonBlockingSerial::try_read_byte(self).map_err(Into::into)
    }
    fn try_write(&mut self, bytes: &[u8]) -> Result<usize, SerialError> {
        NonBlockingSerial::try_write(self, bytes).map_err(Into::into)
    }
    fn try_read(&mut self, buf: &mut [u8]) -> Result<usize, SerialError> {
        NonBlockingSerial::try_read(self, buf).map_err(Into::into)
    }
}

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

impl<T: SerialPort> fmt::Write for SerialWriter<T> {
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
