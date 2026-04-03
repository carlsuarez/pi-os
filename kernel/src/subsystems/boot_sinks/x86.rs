use crate::subsystems::boot_sinks::BootSink;
use drivers::{hal::console::ConsoleOutput, peripheral::x86::vga_text::VgaText};

pub struct X86BootSink;

impl X86BootSink {
    pub const fn new() -> Self {
        // VGA is the only boot sink for x86, so we ignore the argument
        Self
    }
}

impl BootSink for X86BootSink {
    fn write_str(&self, s: &str) {
        let mut vga = unsafe { VgaText::new() };
        vga.write_str(s);

        // TODO add serial fallback
    }
}
