use super::fd::FdError;

/// File operations trait
pub trait File: Send + Sync {
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

/// File statistics
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileStat {
    /// File size in bytes
    pub size: usize,
    /// Is directory
    pub is_dir: bool,
}
