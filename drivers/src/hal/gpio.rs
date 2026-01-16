//! GPIO (General Purpose Input/Output) Hardware Abstraction Layer.
//!
//! This module defines platform-independent traits for GPIO control.

/// Pin logic level.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PinLevel {
    /// Logic low (0V or ground).
    Low,
    /// Logic high (VDD or 3.3V/5V depending on system).
    High,
}

impl From<bool> for PinLevel {
    fn from(value: bool) -> Self {
        if value {
            PinLevel::High
        } else {
            PinLevel::Low
        }
    }
}

impl From<PinLevel> for bool {
    fn from(level: PinLevel) -> bool {
        matches!(level, PinLevel::High)
    }
}

/// Internal pull resistor configuration.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PullMode {
    /// No pull resistor (high impedance).
    None,
    /// Enable internal pull-up resistor.
    Up,
    /// Enable internal pull-down resistor.
    Down,
}

/// Event detection configuration for GPIO pins.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum EdgeDetect {
    /// Detect rising edge (low-to-high transition).
    Rising,
    /// Detect falling edge (high-to-low transition).
    Falling,
    /// Detect both rising and falling edges.
    Both,
}

/// Level detection for GPIO interrupts.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum LevelDetect {
    /// Detect when pin is high.
    High,
    /// Detect when pin is low.
    Low,
}

/// GPIO controller trait.
///
/// This trait represents a GPIO controller capable of configuring
/// and controlling multiple GPIO pins.
///
/// # Type Parameters
///
/// - `Pin`: Platform-specific pin identifier (typically `u8` or typed)
/// - `Error`: Error type for operations that can fail
pub trait GpioController {
    /// Platform-specific pin identifier.
    type Pin: Copy + Clone;

    /// Error type for GPIO operations.
    type Error: core::fmt::Debug;

    /// Configure the internal pull resistor for a pin.
    fn set_pull(&mut self, pin: Self::Pin, pull: PullMode) -> Result<(), Self::Error>;

    /// Set a pin to logic high.
    fn set_high(&mut self, pin: Self::Pin) -> Result<(), Self::Error>;

    /// Set a pin to logic low.
    fn set_low(&mut self, pin: Self::Pin) -> Result<(), Self::Error>;

    /// Read the current logic level of a pin.
    fn read(&self, pin: Self::Pin) -> Result<PinLevel, Self::Error>;

    /// Set the pin to a specific level.
    fn set_level(&mut self, pin: Self::Pin, level: PinLevel) -> Result<(), Self::Error> {
        match level {
            PinLevel::High => self.set_high(pin),
            PinLevel::Low => self.set_low(pin),
        }
    }

    /// Toggle the output state of a pin.
    fn toggle(&mut self, pin: Self::Pin) -> Result<(), Self::Error> {
        let level = self.read(pin)?;
        self.set_level(
            pin,
            if level == PinLevel::High {
                PinLevel::Low
            } else {
                PinLevel::High
            },
        )
    }
}

/// Extension trait for GPIO controllers that support interrupt/event detection.
pub trait GpioInterrupts: GpioController {
    /// Enable edge detection for a pin.
    fn enable_edge_detect(&mut self, pin: Self::Pin, edge: EdgeDetect) -> Result<(), Self::Error>;

    /// Disable edge detection for a pin.
    fn disable_edge_detect(&mut self, pin: Self::Pin) -> Result<(), Self::Error>;

    /// Enable level detection for a pin.
    fn enable_level_detect(
        &mut self,
        pin: Self::Pin,
        level: LevelDetect,
    ) -> Result<(), Self::Error>;

    /// Disable level detection for a pin.
    fn disable_level_detect(&mut self, pin: Self::Pin) -> Result<(), Self::Error>;

    /// Check if an event is pending for a pin.
    fn event_pending(&self, pin: Self::Pin) -> Result<bool, Self::Error>;

    /// Clear a pending event for a pin.
    fn clear_event(&mut self, pin: Self::Pin) -> Result<(), Self::Error>;
}

/// Input pin trait.
///
/// This trait represents a GPIO pin configured as an input.
pub trait InputPin {
    /// Error type for read operations.
    type Error: core::fmt::Debug;

    /// Read the pin state.
    fn read(&self) -> Result<PinLevel, Self::Error>;

    /// Check if the pin is currently high.
    fn is_high(&self) -> Result<bool, Self::Error> {
        Ok(self.read()? == PinLevel::High)
    }

    /// Check if the pin is currently low.
    fn is_low(&self) -> Result<bool, Self::Error> {
        Ok(self.read()? == PinLevel::Low)
    }
}

/// Output pin trait.
///
/// This trait represents a GPIO pin configured as an output.
pub trait OutputPin {
    /// Error type for write operations.
    type Error: core::fmt::Debug;

    /// Set the pin to logic high.
    fn set_high(&mut self) -> Result<(), Self::Error>;

    /// Set the pin to logic low.
    fn set_low(&mut self) -> Result<(), Self::Error>;

    /// Set the pin to a specific level.
    fn set_level(&mut self, level: PinLevel) -> Result<(), Self::Error> {
        match level {
            PinLevel::High => self.set_high(),
            PinLevel::Low => self.set_low(),
        }
    }

    /// Set the pin state based on a boolean value.
    fn set_state(&mut self, state: bool) -> Result<(), Self::Error> {
        self.set_level(state.into())
    }
}

/// Stateful output pin that can be toggled.
pub trait StatefulOutputPin: OutputPin {
    /// Read back the current output state.
    fn read(&self) -> Result<PinLevel, Self::Error>;

    /// Toggle the output state.
    fn toggle(&mut self) -> Result<(), Self::Error> {
        let level = self.read()?;
        self.set_level(if level == PinLevel::High {
            PinLevel::Low
        } else {
            PinLevel::High
        })
    }

    /// Check if the pin is currently driven high.
    fn is_set_high(&self) -> Result<bool, Self::Error> {
        Ok(self.read()? == PinLevel::High)
    }

    /// Check if the pin is currently driven low.
    fn is_set_low(&self) -> Result<bool, Self::Error> {
        Ok(self.read()? == PinLevel::Low)
    }
}
