use crate::fs::file::File;
use crate::fs::{FileSystem, FsError};
use alloc::format;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec;
use drivers::platform::{CurrentPlatform, Platform};
pub use uart_file::UartFile;
pub mod uart_file;

pub struct DevFs;

impl DevFs {
    pub const fn new() -> Self {
        Self
    }
}

impl FileSystem for DevFs {
    fn open(&self, path: &str) -> Result<Arc<dyn File>, FsError> {
        if path.starts_with("/dev/uart") {
            if let Ok(index) = path[9..].parse::<usize>() {
                return CurrentPlatform::with_uart(index, |_| {
                    Ok(Arc::new(UartFile::new(index)) as Arc<dyn File>)
                })
                .ok_or(FsError::NotFound)
                .and_then(|x| x);
            }
        }

        Err(FsError::NotFound)
    }

    fn ls(&self, _path: &str) -> Result<vec::Vec<String>, FsError> {
        let mut devices = vec![];

        let mut i = 0;
        while CurrentPlatform::with_uart(i, |_| ()).is_some() {
            devices.push(format!("uart{}", i));
            i += 1;
        }

        Ok(devices)
    }

    fn create(&self, _path: &str) -> Result<Arc<dyn File>, FsError> {
        Err(FsError::PermissionDenied)
    }
    fn delete(&self, _path: &str) -> Result<(), FsError> {
        Err(FsError::PermissionDenied)
    }
    fn stat(&self, path: &str) -> Result<crate::fs::file::FileStat, FsError> {
        if path.starts_with("/dev/uart") {
            let index = path[9..].parse::<usize>().ok();
            if let Some(idx) = index {
                if CurrentPlatform::with_uart(idx, |_| ()).is_some() {
                    return Ok(crate::fs::file::FileStat {
                        size: 0,
                        is_dir: false,
                    });
                }
            }
        }
        Err(FsError::NotFound)
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
