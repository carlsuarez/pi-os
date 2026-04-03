pub mod arm;
pub mod null;
pub mod x86;

use core::sync::atomic::{AtomicBool, Ordering};
use spin::Mutex;

pub trait BootSink {
    fn write_str(&self, s: &str);
}
