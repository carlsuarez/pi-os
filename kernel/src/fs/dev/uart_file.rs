use super::super::file::{File, FileStat, FileType};
use crate::fs::fd::FdError;
use crate::subsystems::device_manager;
use alloc::string::String;
use drivers::hal::serial::DynSerialPort;

/// UART device file - provides file interface to serial ports
pub struct UartFile {
    index: usize,
}

impl UartFile {
    /// Create a new UART file for the given index
    ///
    /// # Arguments
    /// - `index`: 0 for console/uart0, 1+ for other UARTs if available
    pub fn new(index: usize) -> Self {
        Self { index }
    }

    /// Get the device name for this UART
    fn device_name(&self) -> String {
        if self.index == 0 {
            "console".into()
        } else {
            alloc::format!("uart{}", self.index)
        }
    }
}

impl File for UartFile {
    fn read(&self, buf: &mut [u8], _offset: usize) -> Result<usize, FdError> {
        let device_mgr = device_manager().lock();
        let serial = device_mgr
            .serial(&self.device_name().as_str())
            .ok_or(FdError::IoError)?;

        if let Some(nb) = serial.lock().as_dyn_nonblocking() {
            return nb.try_read(buf).map_err(|_| FdError::IoError);
        } else {
            return serial.lock().read(buf).map_err(|_| FdError::IoError);
        }
    }

    fn write(&self, buf: &[u8], _offset: usize) -> Result<usize, FdError> {
        let device_mgr = device_manager().lock();
        let serial = device_mgr
            .serial(&self.device_name().as_str())
            .ok_or(FdError::IoError)?;

        let mut uart = serial.lock();
        uart.write(buf).map_err(|_| FdError::IoError)?;
        Ok(buf.len())
    }

    fn stat(&self) -> Result<FileStat, FdError> {
        Ok(FileStat {
            file_type: FileType::CharDevice,
            size: 0,
            name: self.device_name(),
        })
    }
}
