use crate::subsystems::boot_sinks::BootSink;

struct NullSink;

impl BootSink for NullSink {
    fn write_str(&self, _s: &str) {}
}
