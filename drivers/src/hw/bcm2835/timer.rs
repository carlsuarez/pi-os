use common::sync::SpinLock;
use core::ptr::{read_volatile, write_volatile};

/// Base physical address of the system timer peripheral.
///
/// This corresponds to the BCM2835 system timer block.
pub const TIMER_BASE: usize = 0x2000_3000;

/// System timer compare channels.
///
/// Each channel has an associated compare register (`C0`–`C3`) and a
/// corresponding match bit in the control/status register.
#[derive(Copy, Clone)]
pub enum TimerChannel {
    Channel0,
    Channel1,
    Channel2,
    Channel3,
}

/// Memory-mapped register layout of the system timer.
///
/// The layout must exactly match the hardware specification.
/// All fields are accessed using volatile reads/writes.
#[repr(C)]
struct TimerRegisters {
    /// Control/Status register (write-1-to-clear match bits).
    cs: u32,
    /// Counter lower 32 bits (microsecond resolution).
    clo: u32,
    /// Counter upper 32 bits.
    chi: u32,
    /// Compare register for channel 0.
    c0: u32,
    /// Compare register for channel 1.
    c1: u32,
    /// Compare register for channel 2.
    c2: u32,
    /// Compare register for channel 3.
    c3: u32,
}

/// Offset (in `u32` words) from the base of `TimerRegisters` to the first
/// compare register (`c0`).
///
/// Additional channels are addressed by adding the channel index.
const CHANNEL_OFFSET: usize = 0xC;

/// High-level interface to the system timer.
///
/// This type is a thin wrapper around a raw pointer to the memory-mapped
/// timer registers. It does not provide synchronization and assumes
/// single-writer or externally synchronized access.
pub struct Timer {
    regs: *mut TimerRegisters,
    channel_locks: [SpinLock<()>; 4],
}

/// SAFETY: `Timer` provides access to memory-mapped hardware registers.
/// Concurrent access must be synchronized externally to prevent
/// data races.
unsafe impl Sync for Timer {}
unsafe impl Send for Timer {}

impl Timer {
    /// Create a new `Timer` instance bound to the system timer registers.
    ///
    /// # Safety
    /// This does not validate that `TIMER_BASE` is correctly mapped or that
    /// concurrent mutable access is prevented.
    const fn new() -> Self {
        Self {
            regs: TIMER_BASE as *mut TimerRegisters,
            channel_locks: [
                SpinLock::new(()),
                SpinLock::new(()),
                SpinLock::new(()),
                SpinLock::new(()),
            ],
        }
    }

    /// Arm a timer compare interrupt for the given channel.
    ///
    /// The interrupt will fire when the system timer counter reaches
    /// `now + interval_us`.
    ///
    /// # Safety
    /// This method locks the channel to prevent concurrent access.
    /// # Parameters
    /// - `channel`: Timer compare channel to use.
    /// - `interval_us`: Interval in microseconds from the current time.
    ///
    /// # Notes
    /// - Uses wrapping arithmetic to handle counter overflow.
    /// - Any pending match for the channel is cleared before enabling.
    pub fn start(&self, channel: TimerChannel, interval_us: u32) {
        unsafe {
            let r = self.regs;

            // Read current timer value (lower 32 bits).
            let now = read_volatile(&(*r).clo);

            // Compute address of the selected compare register.
            let compare_reg: *mut u32 = (r as *mut u32).add(CHANNEL_OFFSET + (channel as usize));

            // Acquire lock for the channel.
            let _guard = self.channel_locks[channel as usize].lock();

            // Program compare value.
            write_volatile(&mut *compare_reg, now.wrapping_add(interval_us));

            // Clear any pending match (write-1-to-clear).
            write_volatile(&mut (*r).cs, channel as u32);
        }
    }

    /// Clear a pending interrupt for the given channel.
    ///
    /// This acknowledges the interrupt by writing a `1` to the corresponding
    /// match bit in the control/status register.
    pub fn clear_interrupt(&self, channel: TimerChannel) {
        let _guard = self.channel_locks[channel as usize].lock();
        unsafe {
            write_volatile(&mut (*self.regs).cs, channel as u32);
        }
    }
}

/// Global Timer instance
static TIMER: Timer = Timer::new();

/// Access the global Timer instance.
pub fn timer() -> &'static Timer {
    &TIMER
}
