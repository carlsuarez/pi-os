//! Timer Hardware Abstraction Layer.

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TimerMode {
    OneShot,
    Periodic,
}

// Canonical error type

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TimerError {
    InvalidHandle,
    IntervalOutOfRange,
    NotRunning,
    AlreadyRunning,
    Hardware,
    Unsupported,
    Other,
}

// Timer: generic concrete trait
//
// `Handle` is the concrete timer/channel identifier (e.g. a newtype wrapping
// u32 for BCM2835, or an enum for a multi-channel PIT).
//
// The blanket impl for DynTimer requires `usize: Into<Self::Handle>` so that
// the type-erased usize handle can be converted back to the concrete type.
// For most drivers this is trivially: `impl From<usize> for MyHandle`.

pub trait Timer: Send + Sync {
    type Handle: Copy + Clone;
    type Error: core::fmt::Debug + Into<TimerError>;

    fn start(&mut self, handle: Self::Handle, interval_us: u32) -> Result<(), Self::Error>;
    fn stop(&mut self, handle: Self::Handle) -> Result<(), Self::Error>;
    fn clear_interrupt(&mut self, handle: Self::Handle) -> Result<(), Self::Error>;
    fn is_pending(&self, handle: Self::Handle) -> Result<bool, Self::Error>;
}

// Extension traits

pub trait CountingTimer: Timer {
    /// Read the free-running counter in microseconds.
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

pub trait PeriodicTimer: Timer {
    fn start_periodic(&mut self, handle: Self::Handle, interval_us: u32)
    -> Result<(), Self::Error>;
}

// DynTimer: object-safe type-erased trait
//
// Handle is erased to `usize`.  The blanket impl converts it back to
// T::Handle via `Into<T::Handle>`, which drivers satisfy with a trivial
// `impl From<usize> for MyHandle { fn from(n: usize) -> Self { ... } }`.

pub trait DynTimer: Send + Sync {
    fn start(&mut self, handle: usize, interval_us: u32) -> Result<(), TimerError>;
    fn stop(&mut self, handle: usize) -> Result<(), TimerError>;
    fn clear_interrupt(&mut self, handle: usize) -> Result<(), TimerError>;
    fn is_pending(&self, handle: usize) -> Result<bool, TimerError>;
}

impl<T> DynTimer for T
where
    T: Timer,
    usize: Into<T::Handle>,
{
    fn start(&mut self, handle: usize, interval_us: u32) -> Result<(), TimerError> {
        Timer::start(self, handle.into(), interval_us).map_err(Into::into)
    }
    fn stop(&mut self, handle: usize) -> Result<(), TimerError> {
        Timer::stop(self, handle.into()).map_err(Into::into)
    }
    fn clear_interrupt(&mut self, handle: usize) -> Result<(), TimerError> {
        Timer::clear_interrupt(self, handle.into()).map_err(Into::into)
    }
    fn is_pending(&self, handle: usize) -> Result<bool, TimerError> {
        Timer::is_pending(self, handle.into()).map_err(Into::into)
    }
}

// DynCountingTimer

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

impl<T> DynCountingTimer for T
where
    T: CountingTimer,
    usize: Into<T::Handle>,
{
    fn now_us(&self) -> u64 {
        CountingTimer::now_us(self)
    }
}

// DynPeriodicTimer

pub trait DynPeriodicTimer: DynTimer {
    fn start_periodic(&mut self, handle: usize, interval_us: u32) -> Result<(), TimerError>;
}

impl<T> DynPeriodicTimer for T
where
    T: PeriodicTimer,
    usize: Into<T::Handle>,
{
    fn start_periodic(&mut self, handle: usize, interval_us: u32) -> Result<(), TimerError> {
        PeriodicTimer::start_periodic(self, handle.into(), interval_us).map_err(Into::into)
    }
}
