use super::file::{File, FileStat};
use super::{FileSystem, FsError};
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use common::sync::SpinLock;
pub use uart_file::UartFile;
pub mod framebuffer_file;
pub mod uart_file;
pub use framebuffer_file::FrameBufferFile;

pub struct DevFs {
    devices: SpinLock<BTreeMap<String, Arc<dyn File>>>,
}

impl DevFs {
    pub fn new() -> Self {
        Self {
            devices: SpinLock::new(BTreeMap::new()),
        }
    }

    pub fn register_device(&self, name: &str, device: Arc<dyn File>) {
        self.devices.lock().insert(name.into(), device);
    }
}

impl FileSystem for DevFs {
    fn open(&self, path: &str) -> Result<Arc<dyn File>, FsError> {
        let path = path.trim_start_matches('/');
        self.devices
            .lock()
            .get(path)
            .cloned()
            .ok_or(FsError::NotFound)
    }

    fn create(&self, _path: &str) -> Result<Arc<dyn File>, FsError> {
        Err(FsError::PermissionDenied) // Can't create devices dynamically
    }

    fn delete(&self, _path: &str) -> Result<(), FsError> {
        Err(FsError::PermissionDenied)
    }

    fn ls(&self, path: &str) -> Result<Vec<String>, FsError> {
        if path == "/" || path.is_empty() {
            Ok(self.devices.lock().keys().cloned().collect())
        } else {
            Err(FsError::NotADirectory)
        }
    }

    fn mkdir(&self, _path: &str) -> Result<(), FsError> {
        Err(FsError::PermissionDenied)
    }

    fn rmdir(&self, _path: &str) -> Result<(), FsError> {
        Err(FsError::PermissionDenied)
    }

    fn stat(&self, path: &str) -> Result<FileStat, FsError> {
        let path = path.trim_start_matches('/');
        let devices = self.devices.lock();
        let device = devices.get(path).ok_or(FsError::NotFound)?;
        device.stat().map_err(|e| FsError::from(e))
    }
}
