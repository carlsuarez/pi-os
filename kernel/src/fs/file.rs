use super::fd::FdError;
use alloc::sync::Arc;
use core::fmt;

/// A file handle
#[derive(Clone)]
pub struct File {
    inner: Arc<dyn FileOperations>,
}

impl File {
    /// Create a new file from a type implementing FileOperations
    pub fn new(ops: Arc<dyn FileOperations>) -> Self {
        Self { inner: ops }
    }

    /// Create a UART file (for stdin/stdout/stderr)
    pub fn new_uart() -> Self {
        Self::new(Arc::new(UartFile))
    }

    /// Read from the file
    pub fn read(&self, buf: &mut [u8], offset: usize) -> Result<usize, FdError> {
        self.inner.read(buf, offset)
    }

    /// Write to the file
    pub fn write(&self, buf: &[u8], offset: usize) -> Result<usize, FdError> {
        self.inner.write(buf, offset)
    }

    /// Get file size
    pub fn size(&self) -> Result<usize, FdError> {
        self.inner.size()
    }
}

impl fmt::Debug for File {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "File {{ ... }}")
    }
}

/// File operations trait
pub trait FileOperations: Send + Sync {
    /// Read from the file
    fn read(&self, buf: &mut [u8], offset: usize) -> Result<usize, FdError>;

    /// Write to the file
    fn write(&self, buf: &[u8], offset: usize) -> Result<usize, FdError>;

    /// Get file size
    fn size(&self) -> Result<usize, FdError> {
        Err(FdError::NotSupported)
    }

    /// Seek (optional, default not supported)
    fn seek(&self, _whence: SeekWhence, _offset: isize) -> Result<usize, FdError> {
        Err(FdError::NotSupported)
    }
}

/// UART file implementation (for stdio)
struct UartFile;

impl FileOperations for UartFile {
    fn read(&self, buf: &mut [u8], _offset: usize) -> Result<usize, FdError> {
        // Read bytes from UART
        let mut count = 0;
        for slot in buf.iter_mut() {
            if let Some(byte) = drivers::uart::with_uart0(|uart| uart.try_read_byte()) {
                *slot = byte;
                count += 1;
            } else {
                break; // No more data available
            }
        }
        Ok(count)
    }

    fn write(&self, buf: &[u8], _offset: usize) -> Result<usize, FdError> {
        for &byte in buf {
            drivers::uart::with_uart0(|uart| uart.write_byte(byte));
        }
        Ok(buf.len())
    }
}

/// Open flags for files
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenFlags(u32);

impl OpenFlags {
    pub const RDONLY: Self = Self(0);
    pub const WRONLY: Self = Self(1);
    pub const RDWR: Self = Self(2);
    pub const CREATE: Self = Self(1 << 6);
    pub const TRUNC: Self = Self(1 << 9);
    pub const APPEND: Self = Self(1 << 10);
}

/// Seek whence
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeekWhence {
    /// Seek from start of file
    Start,
    /// Seek from current position
    Current,
    /// Seek from end of file
    End,
}
