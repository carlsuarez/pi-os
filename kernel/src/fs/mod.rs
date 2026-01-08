use alloc::{string::String, vec::Vec};

pub mod fat;
pub mod fd;
pub mod file;

pub trait FileSystem: Send + Sync {
    /// Open a file
    fn open(&self, path: &str, flags: file::OpenFlags) -> Result<fd::Fd, &'static str>;

    /// Create a file
    fn create(&self, path: &str, flags: file::OpenFlags) -> Result<fd::Fd, &'static str>;

    /// Delete a file
    fn delete(&self, path: &str) -> Result<(), &'static str>;

    /// Get file statistics
    fn stat(&self, path: &str) -> Result<file::FileStat, &'static str>;

    /// List directory contents
    fn ls(&self, path: &str) -> Result<Vec<String>, &'static str>;

    /// Make a directory
    fn mkdir(&self, path: &str) -> Result<(), &'static str>;

    /// Remove a directory
    fn rmdir(&self, path: &str) -> Result<(), &'static str>;

    /// Mount the filesystem
    fn mount(&self) -> Result<(), &'static str>;
}
