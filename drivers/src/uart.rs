use crate::hw::pl011::{Pl011, UART0_BASE, UartError};
use common::sync::SpinLock;

/// Shared UART type
pub type Uart = SpinLock<Pl011>;

/// Global UART0
pub static UART0: Uart = SpinLock::new(unsafe { Pl011::new(UART0_BASE) });

/// Initialize UART
pub fn init_uart(uart: &Uart, baud_rate: u32) -> Result<(), UartError> {
    uart.lock().init(baud_rate)
}

/// Execute a closure with exclusive access
pub fn with_uart<F, R>(uart: &Uart, f: F) -> R
where
    F: FnOnce(&mut Pl011) -> R,
{
    let mut guard = uart.lock();
    f(&mut guard)
}

/// Convenience printing to UART0
pub fn print(s: &str) {
    with_uart(&UART0, |uart| uart.write_str(s));
}

/// Write a formatted string to UART0
#[macro_export]
macro_rules! uart_print {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        let _ = write!($crate::uart::UartWriter, $($arg)*);
    }};
}

/// Write a formatted string with newline to UART0
#[macro_export]
macro_rules! uart_println {
    () => { $crate::uart_print!("\n") };
    ($($arg:tt)*) => {{
        $crate::uart_print!($($arg)*);
        $crate::uart_print!("\n");
    }};
}

/// Writer adapter for `core::fmt::Write` trait
pub struct UartWriter;

impl core::fmt::Write for UartWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        with_uart(&UART0, |uart| uart.write_str(s));
        Ok(())
    }
}
