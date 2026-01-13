use crate::fs::fd::FdError;
use crate::fs::file::File;
use drivers::uart::{Uart, with_uart};

/// UART file implementation - wraps a reference to a static UART
pub struct UartFile {
    uart: &'static Uart,
}

impl UartFile {
    pub const fn new(uart: &'static Uart) -> Self {
        Self { uart }
    }
}

impl File for UartFile {
    fn read(&self, buf: &mut [u8], _offset: usize) -> Result<usize, FdError> {
        let mut count = 0;
        for slot in buf.iter_mut() {
            if let Some(byte) = self.uart.lock().try_read_byte() {
                *slot = byte;
                count += 1;
            } else {
                break;
            }
        }
        Ok(count)
    }

    fn write(&self, buf: &[u8], _offset: usize) -> Result<usize, FdError> {
        with_uart(self.uart, |uart| {
            for &byte in buf {
                uart.write_byte(byte);
            }
        });
        Ok(buf.len())
    }
}
