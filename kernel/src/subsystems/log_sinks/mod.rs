use crate::logger::{self, LogSink};
use alloc::sync::Arc;
use alloc::vec;
use spin::Mutex;

/// Wraps the runtime serial console as a LogSink.
/// Held as a &'static so it can be registered with the logger.
pub struct SerialLogSink;

// SAFETY: SerialLogSink has no fields; all state is behind a global Mutex.
unsafe impl Sync for SerialLogSink {}
unsafe impl Send for SerialLogSink {}

impl LogSink for SerialLogSink {
    fn write_str(&self, s: &str) {
        // serial_console() returns Option<Arc<Mutex<dyn DynSerialPort>>>
        if let Some(serial) = crate::subsystems::serial_console() {
            let mut port = serial.lock();
            // DynSerialPort exposes write_str; convert str to bytes
            let _ = port.write(s.as_bytes());
        }
    }
}

pub static SERIAL_SINK: SerialLogSink = SerialLogSink;
