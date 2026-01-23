use crate::device_manager::devices;

pub fn console_write(s: &str) {
    if let Some(console) = devices().lock().console() {
        let mut port = console.lock();

        let _ = port.write(s.as_bytes()).unwrap();
    }
}

// ============================================================================
// Print Macros
// ============================================================================

/// Print to console without newline
#[macro_export]
macro_rules! kprint {
    ($($arg:tt)*) => {{
        use alloc::format;
        let s = format!($($arg)*);
        let _ = $crate::console::console_write(&s);
    }};
}

#[macro_export]
macro_rules! kprintln {
    () => { $crate::kprint!("\n") };
    ($($arg:tt)*) => {{
        $crate::kprint!($($arg)*);
        $crate::kprint!("\n");
    }};
}
