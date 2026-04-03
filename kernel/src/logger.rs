//! Kernel logger (two-phase design)
//!
//! Phase 1: Boot logging via BootSink (UART/VGA)
//! Phase 2: Runtime logging via dynamic LogSink fanout
use crate::subsystems::boot_console;
use crate::subsystems::boot_sinks::BootSink;
use core::fmt::Write;
use core::sync::atomic::{AtomicU8, Ordering};
use log::{LevelFilter, Log, Metadata, Record};
use spin::Mutex;

/// ----------------------------
/// Runtime sink (post-init)
/// ----------------------------
pub trait LogSink: Send + Sync {
    fn write_str(&self, s: &str);
}

/// ----------------------------
/// Logger mode
/// ----------------------------
pub enum LoggerMode {
    Boot, // uses boot_console() directly — avoids the static init chicken-and-egg problem
    Runtime {
        sinks: alloc::vec::Vec<&'static dyn LogSink>,
    },
}

/// ----------------------------
/// Kernel logger
/// ----------------------------
pub struct KernelLogger {
    mode: Mutex<LoggerMode>,
    max_level: AtomicU8,
}

// SAFETY: KernelLogger only contains Mutex<LoggerMode> (Mutex: Sync) and AtomicU8 (Sync).
// LoggerMode::Boot carries no data; Runtime sinks are &'static dyn LogSink: Send+Sync.
unsafe impl Sync for KernelLogger {}

/// Global logger instance
pub static LOGGER: KernelLogger = KernelLogger {
    mode: Mutex::new(LoggerMode::Boot),
    max_level: AtomicU8::new(LevelFilter::Info as u8),
};

/// ----------------------------
/// Initialization (boot phase)
/// ----------------------------
pub fn init(level: LevelFilter) {
    LOGGER.max_level.store(level as u8, Ordering::Relaxed);
    *LOGGER.mode.lock() = LoggerMode::Boot;
    log::set_logger(&LOGGER).expect("logger already set");
    log::set_max_level(level);
}

/// ----------------------------
/// Transition to runtime phase
/// ----------------------------
pub fn attach_runtime(sinks: alloc::vec::Vec<&'static dyn LogSink>) {
    *LOGGER.mode.lock() = LoggerMode::Runtime { sinks };
}

/// ----------------------------
/// Log implementation
/// ----------------------------
impl Log for KernelLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        let max = level_from_u8(self.max_level.load(Ordering::Relaxed));
        metadata.level() <= max
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let mut buf = FmtBuf::<512>::new();
        let _ = write!(
            buf,
            "[{:<5} {}] {}\n",
            record.level(),
            record.target(),
            record.args()
        );
        let s = buf.as_str();

        let mode = self.mode.lock();
        match &*mode {
            LoggerMode::Boot => {
                boot_console().write_str(s);
            }
            LoggerMode::Runtime { sinks } => {
                for sink in sinks.iter() {
                    sink.write_str(s);
                }
            }
        }
    }

    fn flush(&self) {}
}

fn level_from_u8(v: u8) -> LevelFilter {
    match v {
        0 => LevelFilter::Off,
        1 => LevelFilter::Error,
        2 => LevelFilter::Warn,
        3 => LevelFilter::Info,
        4 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    }
}

/// ----------------------------
/// Fixed-size formatter buffer
/// ----------------------------
pub struct FmtBuf<const N: usize> {
    buf: [u8; N],
    pos: usize,
}

impl<const N: usize> FmtBuf<N> {
    pub const fn new() -> Self {
        Self {
            buf: [0u8; N],
            pos: 0,
        }
    }

    pub fn as_str(&self) -> &str {
        // SAFETY: we only ever write valid UTF-8 (from &str slices)
        unsafe { core::str::from_utf8_unchecked(&self.buf[..self.pos]) }
    }
}

impl<const N: usize> Write for FmtBuf<N> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let bytes = s.as_bytes();
        let space = N.saturating_sub(self.pos);
        let n = bytes.len().min(space);
        self.buf[self.pos..self.pos + n].copy_from_slice(&bytes[..n]);
        self.pos += n;
        Ok(())
    }
}
