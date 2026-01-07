use crate::hw::pl011::{Pl011, UART0_BASE, UartError};
use common::sync::SpinLock;

/// Global UART0 instance protected by a spinlock
static UART0: SpinLock<Pl011> = SpinLock::new(unsafe { Pl011::new(UART0_BASE) });

/// Initialize UART0 with the specified baud rate
///
/// This should be called once during system initialization.
/// Subsequent calls are safe but have no effect.
pub fn init_uart0(baud_rate: u32) -> Result<(), UartError> {
    UART0.lock().init(baud_rate)
}

/// Execute a closure with exclusive access to UART0
///
/// # Example
/// ```
/// with_uart0(|uart| {
///     uart.write_str("Hello, world!\n");
/// });
/// ```
pub fn with_uart0<F, R>(f: F) -> R
where
    F: FnOnce(&mut Pl011) -> R,
{
    let mut uart = UART0.lock();
    f(&mut uart)
}

/// Write a string to UART0
pub fn print(s: &str) {
    with_uart0(|uart| uart.write_str(s));
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
        with_uart0(|uart| uart.write_str(s));
        Ok(())
    }
}
