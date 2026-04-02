/// A trait for drivers that can emit text to a display (VGA, framebuffer
/// text layer, etc.).  Separate from SerialPort so that the two can coexist
/// and console_write() can prefer one over the other per platform.
pub trait ConsoleOutput: Send {
    fn write_byte(&mut self, byte: u8);

    fn write_str(&mut self, s: &str) {
        for byte in s.bytes() {
            self.write_byte(byte);
        }
    }

    fn clear(&mut self);
    fn set_cursor(&mut self, col: usize, row: usize);
}

/// Type-erased version, mirroring the Dyn* pattern used for DynSerialPort,
/// DynTimer, etc. throughout the HAL.
pub trait DynConsoleOutput: Send {
    fn write_str(&mut self, s: &str);
    fn clear(&mut self);
}

/// Blanket impl: anything that implements ConsoleOutput gets DynConsoleOutput
/// for free.
impl<T: ConsoleOutput> DynConsoleOutput for T {
    fn write_str(&mut self, s: &str) {
        ConsoleOutput::write_str(self, s);
    }
    fn clear(&mut self) {
        ConsoleOutput::clear(self);
    }
}
