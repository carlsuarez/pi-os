use super::fd::FdError;

/// File operations trait
pub trait File: Send + Sync {
    /// Read from the file
    fn read(&self, buf: &mut [u8], offset: usize) -> Result<usize, FdError>;

    /// Write to the file
    fn write(&self, buf: &[u8], offset: usize) -> Result<usize, FdError>;

    /// Get file statistics
    fn stat(&self) -> Result<FileStat, FdError> {
        Err(FdError::NotSupported)
    }
}

/// Type of file in the filesystem
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    /// Regular file (data file)
    Regular,
    /// Directory
    Directory,
    /// Character device (e.g., UART, console)
    CharDevice,
    /// Block device (e.g., disk, SD card)
    BlockDevice,
    /// Symbolic link
    Symlink,
    /// Named pipe (FIFO)
    Pipe,
    /// Unix domain socket
    Socket,
}

impl FileType {
    /// Check if this is a regular file
    pub fn is_regular(&self) -> bool {
        matches!(self, FileType::Regular)
    }

    /// Check if this is a directory
    pub fn is_dir(&self) -> bool {
        matches!(self, FileType::Directory)
    }

    /// Check if this is a device (block or character)
    pub fn is_device(&self) -> bool {
        matches!(self, FileType::CharDevice | FileType::BlockDevice)
    }

    /// Check if this is a character device
    pub fn is_char_device(&self) -> bool {
        matches!(self, FileType::CharDevice)
    }

    /// Check if this is a block device
    pub fn is_block_device(&self) -> bool {
        matches!(self, FileType::BlockDevice)
    }

    /// Get a single character representation (like `ls -l`)
    pub fn to_char(&self) -> char {
        match self {
            FileType::Regular => '-',
            FileType::Directory => 'd',
            FileType::CharDevice => 'c',
            FileType::BlockDevice => 'b',
            FileType::Symlink => 'l',
            FileType::Pipe => 'p',
            FileType::Socket => 's',
        }
    }
}

impl core::fmt::Display for FileType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            FileType::Regular => write!(f, "regular file"),
            FileType::Directory => write!(f, "directory"),
            FileType::CharDevice => write!(f, "character device"),
            FileType::BlockDevice => write!(f, "block device"),
            FileType::Symlink => write!(f, "symbolic link"),
            FileType::Pipe => write!(f, "named pipe"),
            FileType::Socket => write!(f, "socket"),
        }
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileStat {
    /// File size in bytes
    pub size: usize,
    /// Type of file
    pub file_type: FileType,
    /// File name
    pub name: alloc::string::String,
}
