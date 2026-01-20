use crate::fs::fd::FdError;
use crate::fs::file::File;
use drivers::platform::{CurrentPlatform, Platform};

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
        let n = CurrentPlatform::with_uart(self.index, |uart| {
            if let Some(nb) = uart.as_nonblocking() {
                nb.try_read(buf).ok()
            } else {
                uart.read(buf).ok()
            }
        })
        .flatten()
        .unwrap_or(0);

        Ok(n)
    }

    fn write(&self, buf: &[u8], _offset: usize) -> Result<usize, FdError> {
        let n = CurrentPlatform::with_uart(self.index, |uart| {
            if let Some(nb) = uart.as_nonblocking() {
                nb.try_write(buf).ok()
            } else {
                uart.write(buf).ok()
            }
        })
        .flatten()
        .unwrap_or(0);

        Ok(n)
    }
}
