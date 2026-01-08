use core::fmt::Debug;

/// Architecture-specific interrupt masking interface.
///
/// Implemented by the kernel architecture layer.
pub trait IrqControl {
    /// Saved interrupt state
    type State: Copy + Debug;

    /// Disable interrupts and return the previous state.
    fn disable() -> Self::State;

    /// Restore interrupts to a previous state.
    fn restore(state: Self::State);
}
