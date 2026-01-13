use super::dev::UartFile;
use super::file::{File, SeekWhence};
use alloc::{sync::Arc, vec::Vec};
use core::fmt;
use drivers::uart::UART0;

/// File descriptor number (index into process's fd table)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Fd(pub usize);

impl Fd {
    /// Standard input
    pub const STDIN: Fd = Fd(0);
    /// Standard output
    pub const STDOUT: Fd = Fd(1);
    /// Standard error
    pub const STDERR: Fd = Fd(2);

    pub fn is_standard(self) -> bool {
        self == Fd::STDIN || self == Fd::STDOUT || self == Fd::STDERR
    }
}

/// A file descriptor entry in a process's file descriptor table
pub struct FileDescriptor {
    /// The underlying file
    file: Arc<dyn File>,
    /// Flags (close-on-exec, etc.)
    flags: FdFlags,
    /// Access mode (rd, wr, etc.)
    access: AccessMode,
    /// Current file offset (for seekable files)
    offset: usize,
}

impl FileDescriptor {
    /// Create a new file descriptor
    pub fn new(file: Arc<dyn File>, flags: FdFlags, access: AccessMode) -> Self {
        Self {
            file,
            flags,
            access,
            offset: 0,
        }
    }

    /// Read from the file
    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, FdError> {
        let n = self.file.read(buf, self.offset)?;
        self.offset += n;
        Ok(n)
    }

    /// Write to the file
    pub fn write(&mut self, buf: &[u8]) -> Result<usize, FdError> {
        let n = self.file.write(buf, self.offset)?;
        self.offset += n;
        Ok(n)
    }

    /// Seek to a position
    pub fn seek(&mut self, whence: SeekWhence, offset: isize) -> Result<usize, FdError> {
        use SeekWhence::*;

        let new_offset = match whence {
            Start => offset.max(0) as usize,
            Current => (self.offset as isize + offset).max(0) as usize,
            End => {
                let size = self.file.size()?;
                (size as isize + offset).max(0) as usize
            }
        };

        self.offset = new_offset;
        Ok(self.offset)
    }

    /// Get current offset
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Get file reference
    pub fn file(&self) -> &Arc<dyn File> {
        &self.file
    }

    /// Get flags
    pub fn flags(&self) -> FdFlags {
        self.flags
    }

    /// Get access mode
    pub fn access(&self) -> AccessMode {
        self.access
    }

    /// Set close-on-exec flag
    pub fn set_cloexec(&mut self, enabled: bool) {
        self.flags.set(FdFlags::CLOEXEC, enabled);
    }
}

impl fmt::Debug for FileDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileDescriptor")
            .field("file", &format_args!("<file>"))
            .field("offset", &self.offset)
            .field("flags", &self.flags)
            .finish()
    }
}

/// File descriptor flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FdFlags(u32);

impl FdFlags {
    /// No flags
    pub const NONE: Self = Self(0);
    /// Close on exec
    pub const CLOEXEC: Self = Self(1 << 0);

    /// Check if a flag is set
    pub fn contains(self, flag: Self) -> bool {
        self.0 & flag.0 != 0
    }

    /// Set or clear a flag
    pub fn set(&mut self, flag: Self, enabled: bool) {
        if enabled {
            self.0 |= flag.0;
        } else {
            self.0 &= !flag.0;
        }
    }
}

/// Access mode
#[derive(Debug, Clone, Copy)]
pub struct AccessMode {
    pub read: bool,
    pub write: bool,
    pub append: bool,
}

impl AccessMode {
    pub const fn default() -> Self {
        Self {
            read: false,
            write: false,
            append: false,
        }
    }

    pub fn set_readable(&mut self, flag: bool) {
        self.read = flag;
    }
    pub fn set_writeable(&mut self, flag: bool) {
        self.write = flag;
    }
    pub fn set_appendable(&mut self, flag: bool) {
        self.append = flag;
    }
}

/// Per-process file descriptor table
pub struct FileDescriptorTable {
    fds: Vec<Option<FileDescriptor>>,
    next_fd: usize,
}

impl FileDescriptorTable {
    /// Create a new file descriptor table with stdio
    pub fn new() -> Self {
        let mut table = Self {
            fds: Vec::new(),
            next_fd: 3, // Start after stdin/stdout/stderr
        };

        // Initialize standard streams
        // For now, use UART for all three
        let stdin = FileDescriptor::new(
            Arc::new(UartFile::new(&UART0)),
            FdFlags::NONE,
            AccessMode {
                read: true,
                write: true,
                append: false,
            },
        );
        let stdout = FileDescriptor::new(
            Arc::new(UartFile::new(&UART0)),
            FdFlags::NONE,
            AccessMode {
                read: true,
                write: true,
                append: false,
            },
        );
        let stderr = FileDescriptor::new(
            Arc::new(UartFile::new(&UART0)),
            FdFlags::NONE,
            AccessMode {
                read: true,
                write: true,
                append: false,
            },
        );

        table.fds.push(Some(stdin));
        table.fds.push(Some(stdout));
        table.fds.push(Some(stderr));

        table
    }

    /// Allocate a new file descriptor
    pub fn alloc(
        &mut self,
        file: Arc<dyn File>,
        flags: FdFlags,
        access: AccessMode,
    ) -> Result<Fd, FdError> {
        // Try to find a free slot
        for (i, slot) in self.fds.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(FileDescriptor::new(file, flags, access));
                return Ok(Fd(i));
            }
        }

        // No free slots, append
        let fd = Fd(self.fds.len());
        self.fds
            .push(Some(FileDescriptor::new(file, flags, access)));
        Ok(fd)
    }

    /// Get a file descriptor
    pub fn get(&self, fd: Fd) -> Result<&FileDescriptor, FdError> {
        self.fds
            .get(fd.0)
            .and_then(|opt| opt.as_ref())
            .ok_or(FdError::BadFd)
    }

    /// Get a mutable file descriptor
    pub fn get_mut(&mut self, fd: Fd) -> Result<&mut FileDescriptor, FdError> {
        self.fds
            .get_mut(fd.0)
            .and_then(|opt| opt.as_mut())
            .ok_or(FdError::BadFd)
    }

    /// Close a file descriptor
    pub fn close(&mut self, fd: Fd) -> Result<(), FdError> {
        if fd.0 >= self.fds.len() {
            return Err(FdError::BadFd);
        }

        if self.fds[fd.0].is_none() {
            return Err(FdError::BadFd);
        }

        self.fds[fd.0] = None;
        Ok(())
    }

    /// Duplicate a file descriptor
    pub fn dup(&mut self, oldfd: Fd) -> Result<Fd, FdError> {
        let fd_entry = self.get(oldfd)?;
        let file = fd_entry.file().clone();
        let flags = fd_entry.flags();
        let access = fd_entry.access();

        self.alloc(file, flags, access)
    }

    /// Duplicate a file descriptor to a specific fd number
    pub fn dup2(&mut self, oldfd: Fd, newfd: Fd) -> Result<Fd, FdError> {
        if oldfd == newfd {
            return Ok(newfd);
        }

        // Get the old file descriptor
        let fd_entry = self.get(oldfd)?;
        let file = fd_entry.file().clone();
        let flags = fd_entry.flags();
        let access = fd_entry.access();

        // Close newfd if it's open
        if newfd.0 < self.fds.len() && self.fds[newfd.0].is_some() {
            self.close(newfd)?;
        }

        // Extend table if needed
        while self.fds.len() <= newfd.0 {
            self.fds.push(None);
        }

        // Set the new fd
        self.fds[newfd.0] = Some(FileDescriptor::new(file, flags, access));
        Ok(newfd)
    }

    /// Close all file descriptors marked with CLOEXEC
    pub fn close_on_exec(&mut self) {
        for slot in self.fds.iter_mut() {
            if let Some(fd) = slot {
                if fd.flags().contains(FdFlags::CLOEXEC) {
                    *slot = None;
                }
            }
        }
    }

    /// Get count of open file descriptors
    pub fn count(&self) -> usize {
        self.fds.iter().filter(|fd| fd.is_some()).count()
    }
}

impl fmt::Debug for FileDescriptorTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileDescriptorTable")
            .field("open_fds", &self.count())
            .field("capacity", &self.fds.len())
            .finish()
    }
}

/// File descriptor errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FdError {
    /// Bad file descriptor
    BadFd,
    /// Too many open files
    TooManyFiles,
    /// I/O error
    IoError,
    /// Invalid seek
    InvalidSeek,
    /// Not supported
    NotSupported,
}

impl fmt::Display for FdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FdError::BadFd => write!(f, "bad file descriptor"),
            FdError::TooManyFiles => write!(f, "too many open files"),
            FdError::IoError => write!(f, "I/O error"),
            FdError::InvalidSeek => write!(f, "invalid seek"),
            FdError::NotSupported => write!(f, "operation not supported"),
        }
    }
}
