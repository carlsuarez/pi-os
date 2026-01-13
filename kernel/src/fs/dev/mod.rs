pub mod uart_file;

use crate::fs::{FileSystem, FsError, file::File};
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec;
use drivers::uart::UART0;
pub use uart_file::UartFile;

pub struct DevFs;

impl DevFs {
    pub const fn new() -> Self {
        Self
    }
}

impl FileSystem for DevFs {
    fn open(&self, path: &str) -> Result<Arc<dyn File>, FsError> {
        match path {
            "/dev/uart0" => Ok(Arc::new(UartFile::new(&UART0))),
            _ => Err(FsError::NotFound),
        }
    }

    fn create(&self, _path: &str) -> Result<Arc<dyn File>, FsError> {
        Err(FsError::PermissionDenied)
    }

    fn delete(&self, _path: &str) -> Result<(), FsError> {
        Err(FsError::PermissionDenied)
    }

    fn stat(&self, path: &str) -> Result<crate::fs::file::FileStat, FsError> {
        match path {
            "/dev/uart0" => Ok(crate::fs::file::FileStat {
                size: 0,
                is_dir: false,
            }),
            _ => Err(FsError::NotFound),
        }
    }

    fn ls(&self, _path: &str) -> Result<vec::Vec<String>, FsError> {
        Ok(vec![String::from("uart0")])
    }

    fn mkdir(&self, _path: &str) -> Result<(), FsError> {
        Err(FsError::PermissionDenied)
    }

    fn rmdir(&self, _path: &str) -> Result<(), FsError> {
        Err(FsError::PermissionDenied)
    }

    fn mount(&self) -> Result<(), FsError> {
        Ok(())
    }
}
