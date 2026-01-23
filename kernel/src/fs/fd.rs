use super::dev::UartFile;
use super::file::{File, SeekWhence};
use crate::fs::FsError;
use alloc::string::String;
use alloc::{sync::Arc, vec::Vec};
use core::fmt;

/// File descriptor number (index into process's fd table)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Fd(pub usize);

impl Fd {
    pub const STDIN: Fd = Fd(0);
    pub const STDOUT: Fd = Fd(1);
    pub const STDERR: Fd = Fd(2);

    pub fn is_standard(self) -> bool {
        matches!(self, Fd::STDIN | Fd::STDOUT | Fd::STDERR)
    }
}

/// A file descriptor entry in a process's file descriptor table
pub struct FileDescriptor {
    file: Arc<dyn File>,
    flags: FdFlags,
    access: AccessMode,
    offset: usize,
}

impl FileDescriptor {
    pub fn new(file: Arc<dyn File>, flags: FdFlags, access: AccessMode) -> Self {
        Self {
            file,
            flags,
            access,
            offset: 0,
        }
    }

    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, FdError> {
        if self.access.read == false {
            return Err(FdError::PermissionDenied);
        }

        let n = self.file.read(buf, self.offset)?;
        self.offset += n;
        Ok(n)
    }

    pub fn write(&mut self, buf: &[u8]) -> Result<usize, FdError> {
        if self.access.write == false {
            return Err(FdError::PermissionDenied);
        }

        if self.access.append {
            self.offset = self.file.stat()?.size;
        }

        let n = self.file.write(buf, self.offset)?;
        self.offset += n;
        Ok(n)
    }

    pub fn seek(&mut self, whence: SeekWhence, offset: isize) -> Result<usize, FdError> {
        use SeekWhence::*;
        let new_offset = match whence {
            Start => offset.max(0) as usize,
            Current => (self.offset as isize + offset).max(0) as usize,
            End => (self.file.stat()?.size as isize + offset).max(0) as usize,
        };
        self.offset = new_offset;
        Ok(self.offset)
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn file(&self) -> &Arc<dyn File> {
        &self.file
    }

    pub fn flags(&self) -> FdFlags {
        self.flags
    }

    pub fn access(&self) -> AccessMode {
        self.access
    }

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
    pub const NONE: Self = Self(0);
    pub const CLOEXEC: Self = Self(1 << 0);

    pub fn contains(self, flag: Self) -> bool {
        self.0 & flag.0 != 0
    }

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

impl Default for AccessMode {
    fn default() -> Self {
        Self {
            read: false,
            write: false,
            append: false,
        }
    }
}

/// Per-process file descriptor table
pub struct FileDescriptorTable {
    fds: Vec<Option<FileDescriptor>>,
}

impl FileDescriptorTable {
    /// Create a new table with stdio mapped to platform UARTs
    pub fn new() -> Self {
        let mut table = Self { fds: Vec::new() };

        // Wrap platform UART 0 for STDIN, STDOUT, STDERR
        let stdio_file = Arc::new(UartFile::new(0)); // index 0

        let stdin = FileDescriptor::new(
            stdio_file.clone(),
            FdFlags::NONE,
            AccessMode {
                read: true,
                write: false,
                append: false,
            },
        );
        table.fds.push(Some(stdin));

        let stdout = FileDescriptor::new(
            stdio_file.clone(),
            FdFlags::NONE,
            AccessMode {
                read: false,
                write: true,
                append: false,
            },
        );
        table.fds.push(Some(stdout));

        let stderr = FileDescriptor::new(
            stdio_file.clone(),
            FdFlags::NONE,
            AccessMode {
                read: false,
                write: true,
                append: false,
            },
        );
        table.fds.push(Some(stderr));

        table
    }

    pub fn alloc(
        &mut self,
        file: Arc<dyn File>,
        flags: FdFlags,
        access: AccessMode,
    ) -> Result<Fd, FdError> {
        for (i, slot) in self.fds.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(FileDescriptor::new(file.clone(), flags, access));
                return Ok(Fd(i));
            }
        }
        let fd = Fd(self.fds.len());
        self.fds
            .push(Some(FileDescriptor::new(file, flags, access)));
        Ok(fd)
    }

    pub fn get(&self, fd: Fd) -> Result<&FileDescriptor, FdError> {
        self.fds
            .get(fd.0)
            .and_then(|opt| opt.as_ref())
            .ok_or(FdError::BadFd)
    }

    pub fn get_mut(&mut self, fd: Fd) -> Result<&mut FileDescriptor, FdError> {
        self.fds
            .get_mut(fd.0)
            .and_then(|opt| opt.as_mut())
            .ok_or(FdError::BadFd)
    }

    pub fn close(&mut self, fd: Fd) -> Result<(), FdError> {
        if fd.0 >= self.fds.len() || self.fds[fd.0].is_none() {
            return Err(FdError::BadFd);
        }
        self.fds[fd.0] = None;
        Ok(())
    }

    pub fn dup(&mut self, oldfd: Fd) -> Result<Fd, FdError> {
        let fd_entry = self.get(oldfd)?;
        self.alloc(fd_entry.file().clone(), fd_entry.flags(), fd_entry.access())
    }

    pub fn dup2(&mut self, oldfd: Fd, newfd: Fd) -> Result<Fd, FdError> {
        if oldfd == newfd {
            return Ok(newfd);
        }
        let fd_entry = self.get(oldfd)?;
        let file = fd_entry.file().clone();
        let flags = fd_entry.flags();
        let access = fd_entry.access();

        if newfd.0 < self.fds.len() && self.fds[newfd.0].is_some() {
            self.close(newfd)?;
        }

        while self.fds.len() <= newfd.0 {
            self.fds.push(None);
        }

        self.fds[newfd.0] = Some(FileDescriptor::new(file, flags, access));
        Ok(newfd)
    }

    pub fn close_on_exec(&mut self) {
        for slot in self.fds.iter_mut() {
            if let Some(fd) = slot {
                if fd.flags().contains(FdFlags::CLOEXEC) {
                    *slot = None;
                }
            }
        }
    }

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
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FdError {
    BadFd,
    TooManyFiles,
    IoError,
    InvalidSeek,
    NotSupported,
    PermissionDenied,
    Other(String),
}

impl From<FdError> for FsError {
    fn from(err: FdError) -> Self {
        match err {
            FdError::BadFd => FsError::NotFound,
            FdError::IoError => FsError::IoError,
            FdError::NotSupported => FsError::NotSupported,
            FdError::PermissionDenied => FsError::PermissionDenied,
            other => FsError::Unknown,
        }
    }
}

impl fmt::Display for FdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FdError::BadFd => write!(f, "bad file descriptor"),
            FdError::TooManyFiles => write!(f, "too many open files"),
            FdError::IoError => write!(f, "I/O error"),
            FdError::InvalidSeek => write!(f, "invalid seek"),
            FdError::NotSupported => write!(f, "operation not supported"),
            FdError::PermissionDenied => write!(f, "permission denied"),
            FdError::Other(code) => write!(f, "unknown error: {}", code),
        }
    }
}
