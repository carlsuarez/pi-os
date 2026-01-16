use crate::fs::fd::FdError;
use crate::fs::file::File;
use drivers::{
    SerialPort,
    hal::serial::NonBlockingSerial,
    platform::{CurrentPlatform, PlatformExt},
};

pub struct UartFile {
    index: usize,
}

impl UartFile {
    pub const fn new(index: usize) -> Self {
        Self { index }
    }
}

impl File for UartFile {
    fn read(&self, buf: &mut [u8], _offset: usize) -> Result<usize, FdError> {
        let mut count = 0;

        for slot in buf.iter_mut() {
            // Access the UART via the platform closure
            let byte = CurrentPlatform::with_uart(self.index, |uart| {
                // Try reading a byte; return Some(u8) on success, None on error
                uart.try_read_byte().ok()
            })
            .flatten();

            if let Some(b) = byte {
                *slot = b;
                count += 1;
            } else {
                // Either UART not present or no byte available
                break;
            }
        }

        Ok(count)
    }

    fn write(&self, buf: &[u8], _offset: usize) -> Result<usize, FdError> {
        for &byte in buf {
            CurrentPlatform::with_uart(0, |uart| uart.write_byte(byte));
        }
        Ok(buf.len())
    }
}
