use drivers::platform::{CurrentPlatform, Platform};

#[repr(C)]
pub struct TrapFrame {
    pub spsr: u32,
    pub r0: u32,
    pub r1: u32,
    pub r2: u32,
    pub r3: u32,
    pub r4: u32,
    pub r5: u32,
    pub r6: u32,
    pub r7: u32,
    pub r8: u32,
    pub r9: u32,
    pub r10: u32,
    pub r11: u32,
    pub r12: u32,
    pub lr: u32,
}

#[unsafe(no_mangle)]
pub extern "C" fn irq_entry_rust(tf: &mut TrapFrame) {
    if let Some(irq) = CurrentPlatform::next_pending_irq() {
        crate::irq::dispatch(irq, tf);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn svc_entry_rust(tf: &mut TrapFrame) {
    crate::syscall::dispatch(tf)
}
