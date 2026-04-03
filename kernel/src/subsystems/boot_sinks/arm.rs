use crate::subsystems::boot_sinks::BootSink;

pub struct ArmBootSink;

impl BootSink for ArmBootSink {
    fn write_str(&self, s: &str) {
        let _ = s;
        todo!("Implement ARM boot sink (e.g. UART)");
    }
}
