//! Interrupt Controller Hardware Abstraction Layer.

pub type IrqNumber = u32;
pub type Priority = u8;

pub const IRQ_SYSTEM_TIMER_0: u32 = 0;
pub const IRQ_SYSTEM_TIMER_1: u32 = 1;
pub const IRQ_SYSTEM_TIMER_2: u32 = 2;
pub const IRQ_SYSTEM_TIMER_3: u32 = 3;
pub const IRQ_AUX: u32 = 29;
pub const IRQ_UART0: u32 = 57;

// Canonical error type

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum InterruptError {
    InvalidIrq,
    AlreadyEnabled,
    AlreadyDisabled,
    Hardware,
    Unsupported,
    Other,
}

// InterruptController: generic concrete trait

pub trait InterruptController: Send + Sync {
    type Error: core::fmt::Debug + Into<InterruptError>;

    fn enable(&mut self, irq: IrqNumber) -> Result<(), Self::Error>;
    fn disable(&mut self, irq: IrqNumber) -> Result<(), Self::Error>;
    fn is_pending(&self, irq: IrqNumber) -> Result<bool, Self::Error>;
    fn next_pending(&self) -> Option<IrqNumber>;

    /// Clear a pending interrupt. Default: no-op for controllers that
    /// auto-clear on read.
    fn clear(&mut self, irq: IrqNumber) -> Result<(), Self::Error> {
        let _ = irq;
        Ok(())
    }
}

// Extension traits

pub trait PriorityInterruptController: InterruptController {
    fn set_priority(&mut self, irq: IrqNumber, priority: Priority) -> Result<(), Self::Error>;
    fn get_priority(&self, irq: IrqNumber) -> Result<Priority, Self::Error>;
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TriggerMode {
    RisingEdge,
    FallingEdge,
    LevelHigh,
    LevelLow,
}

pub trait ConfigurableInterruptController: InterruptController {
    fn configure_trigger(&mut self, irq: IrqNumber, mode: TriggerMode) -> Result<(), Self::Error>;
}

// DynInterruptController: object-safe type-erased trait

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

impl<T: InterruptController> DynInterruptController for T {
    fn enable(&mut self, irq: IrqNumber) -> Result<(), InterruptError> {
        InterruptController::enable(self, irq).map_err(Into::into)
    }
    fn disable(&mut self, irq: IrqNumber) -> Result<(), InterruptError> {
        InterruptController::disable(self, irq).map_err(Into::into)
    }
    fn is_pending(&self, irq: IrqNumber) -> Result<bool, InterruptError> {
        InterruptController::is_pending(self, irq).map_err(Into::into)
    }
    fn next_pending(&self) -> Option<IrqNumber> {
        InterruptController::next_pending(self)
    }
    fn clear(&mut self, irq: IrqNumber) -> Result<(), InterruptError> {
        InterruptController::clear(self, irq).map_err(Into::into)
    }
}

// DynPriorityInterruptController

pub trait DynPriorityInterruptController: DynInterruptController {
    fn set_priority(&mut self, irq: IrqNumber, priority: Priority) -> Result<(), InterruptError>;
    fn get_priority(&self, irq: IrqNumber) -> Result<Priority, InterruptError>;
}

impl<T: PriorityInterruptController> DynPriorityInterruptController for T {
    fn set_priority(&mut self, irq: IrqNumber, priority: Priority) -> Result<(), InterruptError> {
        PriorityInterruptController::set_priority(self, irq, priority).map_err(Into::into)
    }
    fn get_priority(&self, irq: IrqNumber) -> Result<Priority, InterruptError> {
        PriorityInterruptController::get_priority(self, irq).map_err(Into::into)
    }
}

// DynConfigurableInterruptController

pub trait DynConfigurableInterruptController: DynInterruptController {
    fn configure_trigger(
        &mut self,
        irq: IrqNumber,
        mode: TriggerMode,
    ) -> Result<(), InterruptError>;
}

impl<T: ConfigurableInterruptController> DynConfigurableInterruptController for T {
    fn configure_trigger(
        &mut self,
        irq: IrqNumber,
        mode: TriggerMode,
    ) -> Result<(), InterruptError> {
        ConfigurableInterruptController::configure_trigger(self, irq, mode).map_err(Into::into)
    }
}
