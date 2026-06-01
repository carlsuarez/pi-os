pub mod trap;
pub use trap::TrapFrame;

#[unsafe(no_mangle)]
unsafe extern "C" fn trap_handler(tf: *mut TrapFrame) {
    let tf = unsafe { &mut *tf };
    let vec = tf.trap_number;

    if vec < 32 {
        panic!(
            "CPU exception {} at eip={:#010x} err={:#010x} cs={:#x} eflags={:#010x}",
            vec, tf.eip, tf.error_code, tf.cs, tf.eflags
        );
    } else if vec < 48 {
        let irq = vec - 32;
        crate::irq::dispatch::dispatch(irq, tf);
        // Send EOI to PIC after handler returns.
        // dispatch() already re-disabled CPU interrupts before returning.
        if let Some(irqctl) = crate::subsystems::irq_controller() {
            let _ = irqctl.lock().clear(irq);
        }
    }
}
