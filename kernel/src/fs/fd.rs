use super::dev::UartFile;
use super::file::{File, SeekWhence};
use crate::fs::FsError;
use alloc::string::String;
use alloc::{sync::Arc, vec::Vec};
use bitflags::bitflags;
use core::fmt;

// ---------------------------------------------------------------------------
// Flag types
// ---------------------------------------------------------------------------

bitflags! {
    /// File descriptor flags (the `FD_*` / `fcntl(F_SETFD)` family).
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct FdFlags : u32 {
        /// Close this fd on `exec`.
        const CLOEXEC = 1 << 0;
    }
}

bitflags! {
    /// File access-mode flags (mirrors `O_RDONLY`, `O_WRONLY`, `O_RDWR`,
    /// `O_APPEND` from POSIX `open(2)`).
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct AccessMode : u32 {
        const READ   = 1 << 0;
        const WRITE  = 1 << 1;
        const APPEND = 1 << 2;
    }
}

impl AccessMode {
    /// Read-only  (`O_RDONLY`)
    pub const RDONLY: Self = Self::READ;
    /// Write-only (`O_WRONLY`)
    pub const WRONLY: Self = Self::WRITE;
    /// Read-write (`O_RDWR`)
    pub const RDWR: Self = Self::READ.union(Self::WRITE);
    /// Append mode — write + append flag (`O_WRONLY | O_APPEND`)
    pub const APPEND_MODE: Self = Self::WRITE.union(Self::APPEND);
}

// ---------------------------------------------------------------------------
// File descriptor number
// ---------------------------------------------------------------------------

/// File descriptor number (index into a process's fd table).
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

// ---------------------------------------------------------------------------
// FileDescriptor
// ---------------------------------------------------------------------------

/// A single entry in a process's file descriptor table.
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
        if !self.access.contains(AccessMode::READ) {
            return Err(FdError::PermissionDenied);
        }
        let n = self.file.read(buf, self.offset)?;
        self.offset += n;
        Ok(n)
    }

    pub fn write(&mut self, buf: &[u8]) -> Result<usize, FdError> {
        if !self.access.contains(AccessMode::WRITE) {
            return Err(FdError::PermissionDenied);
        }
        if self.access.contains(AccessMode::APPEND) {
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
            .field("access", &self.access)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// FileDescriptorTable
// ---------------------------------------------------------------------------

/// Per-process file descriptor table.
pub struct FileDescriptorTable {
    fds: Vec<Option<FileDescriptor>>,
}

impl FileDescriptorTable {
    /// Creates a new table with stdin/stdout/stderr wired to platform UART 0.
    pub fn new() -> Self {
        let mut table = Self { fds: Vec::new() };

        let stdio_file = Arc::new(UartFile::new(0));

        table.fds.push(Some(FileDescriptor::new(
            stdio_file.clone(),
            FdFlags::empty(),
            AccessMode::RDONLY,
        )));
        table.fds.push(Some(FileDescriptor::new(
            stdio_file.clone(),
            FdFlags::empty(),
            AccessMode::WRONLY,
        )));
        table.fds.push(Some(FileDescriptor::new(
            stdio_file.clone(),
            FdFlags::empty(),
            AccessMode::WRONLY,
        )));

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
                *slot = Some(FileDescriptor::new(file, flags, access));
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
        let entry = self.get(oldfd)?;
        self.alloc(Arc::clone(entry.file()), entry.flags(), entry.access())
    }

    pub fn dup2(&mut self, oldfd: Fd, newfd: Fd) -> Result<Fd, FdError> {
        if oldfd == newfd {
            return Ok(newfd);
        }
        let entry = self.get(oldfd)?;
        let file = Arc::clone(entry.file());
        let flags = entry.flags();
        let access = entry.access();

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

// ---------------------------------------------------------------------------
// FdError
// ---------------------------------------------------------------------------

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
            _ => FsError::Unknown,
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
