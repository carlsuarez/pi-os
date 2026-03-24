//! Interrupt Controller Hardware Abstraction Layer.
//!
//! This module defines platform-independent traits for interrupt management.

/// Interrupt number type.
pub type IrqNumber = u32;

/// Interrupt priority level.
///
/// Higher values indicate higher priority.
pub type Priority = u8;

/// IRQ Numbers
pub const IRQ_SYSTEM_TIMER_0: u32 = 0;
pub const IRQ_SYSTEM_TIMER_1: u32 = 1;
pub const IRQ_SYSTEM_TIMER_2: u32 = 2;
pub const IRQ_SYSTEM_TIMER_3: u32 = 3;

pub const IRQ_AUX: u32 = 29; // UART1 / SPI1
pub const IRQ_UART0: u32 = 57; // PL011

// ============================================================================
// Interrupt Controller Errors
// ============================================================================

/// Interrupt controller errors (used for type-erased dyn traits).
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum InterruptError {
    /// Invalid IRQ number
    InvalidIrq,
    /// IRQ already enabled
    AlreadyEnabled,
    /// IRQ already disabled
    AlreadyDisabled,
    /// Hardware error
    Hardware,
    /// Operation not supported
    Unsupported,
    /// Other platform-specific error
    Other,
}

// ============================================================================
// Interrupt Controller Trait (Generic)
// ============================================================================

/// Interrupt controller trait.
///
/// This trait represents the system's interrupt controller.
pub trait InterruptController: Send + Sync {
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
    /// Default implementation does nothing.
    fn clear(&mut self, irq: IrqNumber) -> Result<(), Self::Error> {
        let _ = irq;
        Ok(())
    }
}

// ============================================================================
// Extension Traits (Generic)
// ============================================================================

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

// ============================================================================
// Type-Erased Interrupt Controller Traits
// ============================================================================

/// Type-erased interrupt controller trait using `InterruptError`.
pub trait DynInterruptController: Send + Sync {
    fn enable(&mut self, irq: IrqNumber) -> Result<(), InterruptError>;
    fn disable(&mut self, irq: IrqNumber) -> Result<(), InterruptError>;
    fn is_pending(&self, irq: IrqNumber) -> Result<bool, InterruptError>;
    fn next_pending(&self) -> Option<IrqNumber>;
    fn clear(&mut self, irq: IrqNumber) -> Result<(), InterruptError> {
        let _ = irq;
        Ok(())
    }
}

/// Type-erased priority interrupt controller trait using `InterruptError`.
pub trait DynPriorityInterruptController: DynInterruptController {
    fn set_priority(&mut self, irq: IrqNumber, priority: Priority) -> Result<(), InterruptError>;
    fn get_priority(&self, irq: IrqNumber) -> Result<Priority, InterruptError>;
}

/// Type-erased configurable interrupt controller trait using `InterruptError`.
pub trait DynConfigurableInterruptController: DynInterruptController {
    fn configure_trigger(
        &mut self,
        irq: IrqNumber,
        mode: TriggerMode,
    ) -> Result<(), InterruptError>;
}
