//! Timer Hardware Abstraction Layer.
//!
//! This module defines platform-independent traits for hardware timers.

/// Timer operating mode.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TimerMode {
    /// Timer fires once after the specified interval.
    OneShot,
    /// Timer automatically reloads and fires periodically.
    Periodic,
}

// ============================================================================
// Timer Errors
// ============================================================================

/// Timer errors (used for type-erased dyn traits).
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TimerError {
    /// Invalid timer handle.
    InvalidHandle,
    /// Timer interval out of range.
    IntervalOutOfRange,
    /// Timer is not running.
    NotRunning,
    /// Timer is already running.
    AlreadyRunning,
    /// Hardware error.
    Hardware,
    /// Operation not supported by this timer.
    Unsupported,
    /// Other platform-specific error.
    Other,
}

// ============================================================================
// Timer Trait
// ============================================================================

/// Hardware timer trait.
///
/// This trait represents a timer peripheral that can generate interrupts
/// after specified time intervals.
pub trait Timer {
    /// Platform-specific timer handle/identifier.
    ///
    /// This is an opaque type that identifies which timer or channel
    /// to use. It might be a simple integer or a more complex type.
    type Handle: Copy + Clone;

    /// Error type for timer operations.
    type Error: core::fmt::Debug;

    /// Start a timer with the given interval.
    ///
    /// # Arguments
    ///
    /// - `handle`: Which timer/channel to use
    /// - `interval_us`: Interval in microseconds
    ///
    /// # Errors
    ///
    /// Returns an error if the handle is invalid or the interval
    /// is out of range.
    fn start(&mut self, handle: Self::Handle, interval_us: u32) -> Result<(), Self::Error>;

    /// Stop a timer.
    fn stop(&mut self, handle: Self::Handle) -> Result<(), Self::Error>;

    /// Clear a pending interrupt.
    fn clear_interrupt(&mut self, handle: Self::Handle) -> Result<(), Self::Error>;

    /// Check if a timer has a pending interrupt.
    fn is_pending(&self, handle: Self::Handle) -> Result<bool, Self::Error>;
}

// ============================================================================
// Extension Traits
// ============================================================================

/// Extension trait for timers that support reading the current counter value.
pub trait CountingTimer: Timer {
    /// Read the current timer counter value in microseconds.
    ///
    /// This is a free-running counter that increments continuously.
    fn now_us(&self) -> u64;

    /// Busy-wait delay for the specified number of microseconds.
    ///
    /// This blocks the CPU and should only be used for short delays.
    fn delay_us(&self, us: u32) {
        let start = self.now_us();
        let duration = us as u64;
        while self.now_us().wrapping_sub(start) < duration {
            core::hint::spin_loop();
        }
    }

    /// Busy-wait delay for the specified number of milliseconds.
    fn delay_ms(&self, ms: u32) {
        self.delay_us(ms.saturating_mul(1000));
    }
}

/// Extension trait for timers that support periodic mode.
pub trait PeriodicTimer: Timer {
    /// Start a timer in periodic mode.
    ///
    /// The timer will automatically reload and fire repeatedly.
    fn start_periodic(&mut self, handle: Self::Handle, interval_us: u32)
    -> Result<(), Self::Error>;
}

// ============================================================================
// Type-Erased Timer Traits
// ============================================================================

/// Type-erased timer trait using `TimerError`.
pub trait DynTimer: Send + Sync {
    fn start(&mut self, handle: usize, interval_us: u32) -> Result<(), TimerError>;
    fn stop(&mut self, handle: usize) -> Result<(), TimerError>;
    fn clear_interrupt(&mut self, handle: usize) -> Result<(), TimerError>;
    fn is_pending(&self, handle: usize) -> Result<bool, TimerError>;
}

/// Type-erased counting timer trait using `TimerError`.
pub trait DynCountingTimer: DynTimer {
    fn now_us(&self) -> u64;
    fn delay_us(&self, us: u32) {
        let start = self.now_us();
        let duration = us as u64;
        while self.now_us().wrapping_sub(start) < duration {
            core::hint::spin_loop();
        }
    }
    fn delay_ms(&self, ms: u32) {
        self.delay_us(ms.saturating_mul(1000));
    }
}

/// Type-erased periodic timer trait using `TimerError`.
pub trait DynPeriodicTimer: DynTimer {
    fn start_periodic(&mut self, handle: usize, interval_us: u32) -> Result<(), TimerError>;
}
