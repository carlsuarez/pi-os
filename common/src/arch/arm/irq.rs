use crate::sync::irq::IrqControl;

const CPSR_I_BIT: u32 = 1 << 7;

pub struct ArmIrq;

/// Implementation of interrupt control for ARM architecture.
///
/// Provides low-level interrupt enable/disable functionality using ARM's CPSR
/// (Current Program Status Register).
///
/// # State Management
/// The `State` type is `bool`, representing the previous I bit state.
///
/// # Methods
///
/// - `disable()`: Disables IRQ interrupts by setting the I (IRQ disable) bit in CPSR
///   and returns if the I bit was previously set.
/// - `restore(prev_enabled: bool)`: Restores the I bit state.
///
/// # Safety
///
/// Both methods use inline ARM assembly to directly manipulate CPU control registers.
/// They are marked `unsafe` as they modify critical CPU state. Callers must ensure
/// these are used in appropriate contexts (e.g., interrupt handling, critical sections).
///
/// # Assembly Details
///
/// - `mrs {0}, cpsr`: Move from CPSR to general purpose register
/// - `cpsid i`: Change Processor State - disable IRQ interrupts
/// - `cpsie i`: Change Processor State - enable IRQ interrupts
impl IrqControl for ArmIrq {
    type State = bool;

    #[inline(always)]
    fn disable() -> bool {
        let cpsr: u32;
        unsafe {
            // Save current CPSR and disable IRQs
            core::arch::asm!(
                "mrs {0}, cpsr",
                "cpsid i",
                out(reg) cpsr,
                options(nomem, nostack)
            );
        }
        cpsr & CPSR_I_BIT == 0 // Return true if IRQs were previously enabled
    }

    #[inline(always)]
    fn restore(prev_enabled: bool) {
        if prev_enabled {
            unsafe {
                core::arch::asm!("cpsie i", options(nomem, nostack));
            }
        }
    }
}
