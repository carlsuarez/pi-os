//! Interrupt Controller Hardware Abstraction Layer.
//!
//! This module defines platform-independent traits for interrupt management.

/// Interrupt number type.
pub type IrqNumber = u32;

/// Interrupt priority level.
///
/// Higher values indicate higher priority.
pub type Priority = u8;

/// Interrupt controller trait.
///
/// This trait represents the system's interrupt controller.
pub trait InterruptController {
    /// Error type for interrupt controller operations.
    type Error: core::fmt::Debug;

    /// Enable (unmask) an interrupt line.
    fn enable(&mut self, irq: IrqNumber) -> Result<(), Self::Error>;

    /// Disable (mask) an interrupt line.
    fn disable(&mut self, irq: IrqNumber) -> Result<(), Self::Error>;

    /// Check if an interrupt is currently pending.
    fn is_pending(&self, irq: IrqNumber) -> Result<bool, Self::Error>;

    /// Get the next pending interrupt.
    ///
    /// Returns the highest-priority pending interrupt, or `None`
    /// if no interrupts are pending.
    fn next_pending(&self) -> Option<IrqNumber>;

    /// Clear a pending interrupt.
    ///
    /// Some controllers require explicit acknowledgment.
    fn clear(&mut self, irq: IrqNumber) -> Result<(), Self::Error> {
        let _ = irq;
        Ok(())
    }
}

/// Extension trait for interrupt controllers with priority support.
pub trait PriorityInterruptController: InterruptController {
    /// Set the priority of an interrupt line.
    fn set_priority(&mut self, irq: IrqNumber, priority: Priority) -> Result<(), Self::Error>;

    /// Get the priority of an interrupt line.
    fn get_priority(&self, irq: IrqNumber) -> Result<Priority, Self::Error>;
}

/// Interrupt trigger mode.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TriggerMode {
    /// Interrupt triggers on a rising edge.
    RisingEdge,
    /// Interrupt triggers on a falling edge.
    FallingEdge,
    /// Interrupt is active when the signal is high.
    LevelHigh,
    /// Interrupt is active when the signal is low.
    LevelLow,
}

/// Extension trait for interrupt controllers with trigger mode configuration.
pub trait ConfigurableInterruptController: InterruptController {
    /// Configure an interrupt's trigger mode.
    fn configure_trigger(&mut self, irq: IrqNumber, mode: TriggerMode) -> Result<(), Self::Error>;
}
