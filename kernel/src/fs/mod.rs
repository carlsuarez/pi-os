use alloc::{string::String, sync::Arc, vec::Vec};

use crate::fs::file::File;

pub mod dev;
pub mod fat;
pub mod fd;
pub mod file;
pub mod vfs;

#[derive(Debug)]
pub enum FsError {
    NotFound,
    AlreadyExists,
    NotADirectory,
    IsADirectory,
    PermissionDenied,
    NotSupported,
    IoError,
    Unknown,
}

pub trait FileSystem: Send + Sync {
    /// Open a file
    fn open(&self, path: &str) -> Result<Arc<dyn File>, FsError>;

    /// Create a file
    fn create(&self, path: &str) -> Result<Arc<dyn File>, FsError>;

    /// Delete a file
    fn delete(&self, path: &str) -> Result<(), FsError>;

    /// Get file statistics
    fn stat(&self, path: &str) -> Result<file::FileStat, FsError>;

    /// List directory contents
    fn ls(&self, path: &str) -> Result<Vec<String>, FsError>;

    /// Make a directory
    fn mkdir(&self, path: &str) -> Result<(), FsError>;

    /// Remove a directory
    fn rmdir(&self, path: &str) -> Result<(), FsError>;
}
