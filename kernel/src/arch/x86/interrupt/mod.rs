use common::sync::irq::IrqControl;

const IF_BIT: u16 = 1 << 9;

pub struct X86Irq;

/// Implementation of interrupt control for x86 architecture.
///
/// This trait enables safe management of CPU interrupts through the interrupt flag (IF).
/// The state is tracked as a boolean where `true` indicates interrupts were enabled
/// before the operation.
///
/// # Examples
///
/// ```ignore
/// // Save the current interrupt state and disable interrupts
/// let was_enabled = X86Irq::disable();
///
/// // Critical section with interrupts disabled
///
/// // Restore the previous interrupt state
/// X86Irq::restore(was_enabled);
/// ```
///
/// # Safety
///
/// The inline assembly operations directly manipulate CPU flags and should only be used
/// in contexts where interrupt state management is necessary and safe. The `nomem`
/// and `preserves_flags` options ensure that no memory operations are optimized across
/// these boundaries and that only the IF flag is modified.
impl IrqControl for X86Irq {
    type State = bool; // true if interrupts were enabled

    fn save_and_disable() -> Self::State {
        let state: u16;
        unsafe {
            core::arch::asm!(
                "pushf",
                "pop {0:x}",
                "cli",
                out(reg) state,
                options(nomem, preserves_flags)
            );
        }
        state & IF_BIT != 0 // Check IF flag
    }

    fn restore(state: Self::State) {
        if state {
            unsafe { core::arch::asm!("sti", options(nomem, preserves_flags)) };
        }
    }

    fn enable() {
        unsafe { core::arch::asm!("sti", options(nomem, preserves_flags)) };
    }

    fn disable() {
        unsafe { core::arch::asm!("cli", options(nomem, preserves_flags)) };
    }

    fn wait_for_interrupt() {
        unsafe { core::arch::asm!("hlt", options(nomem, preserves_flags)) };
    }
}
