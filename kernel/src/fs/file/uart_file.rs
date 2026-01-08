use super::FdError;
use super::File;
/// UART file implementation (for stdio)
pub struct UartFile;

impl File for UartFile {
    fn read(&self, buf: &mut [u8], _offset: usize) -> Result<usize, FdError> {
        // Read bytes from UART
        let mut count = 0;
        for slot in buf.iter_mut() {
            if let Some(byte) = drivers::uart::with_uart0(|uart| uart.try_read_byte()) {
                *slot = byte;
                count += 1;
            } else {
                break; // No more data available
            }
        }
        Ok(count)
    }

    fn write(&self, buf: &[u8], _offset: usize) -> Result<usize, FdError> {
        for &byte in buf {
            drivers::uart::with_uart0(|uart| uart.write_byte(byte));
        }
        Ok(buf.len())
    }
}
