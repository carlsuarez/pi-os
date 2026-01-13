use common::arch::arm::irq::ArmIrq;
use common::sync::IrqSpinLock;
use core::ptr::{addr_of, addr_of_mut, read_volatile, write_volatile};

/// Base physical address of the system timer peripheral.
///
/// This corresponds to the BCM2835 system timer block.
pub const TIMER_BASE: usize = 0x2000_3000;

/// System timer compare channels.
///
/// Each channel has an associated compare register (`C0`–`C3`) and a
/// corresponding match bit in the control/status register.
#[derive(Copy, Clone)]
#[repr(usize)]
pub enum TimerChannel {
    Channel0 = 0,
    Channel1 = 1,
    Channel2 = 2,
    Channel3 = 3,
}

impl TimerChannel {
    /// Get the IRQ number associated with this timer channel.
    pub fn irq_number(self) -> u32 {
        match self {
            TimerChannel::Channel0 => common::arch::arm::bcm2835::irq::IRQ_SYSTEM_TIMER_0,
            TimerChannel::Channel1 => common::arch::arm::bcm2835::irq::IRQ_SYSTEM_TIMER_1,
            TimerChannel::Channel2 => common::arch::arm::bcm2835::irq::IRQ_SYSTEM_TIMER_2,
            TimerChannel::Channel3 => common::arch::arm::bcm2835::irq::IRQ_SYSTEM_TIMER_3,
        }
    }

    #[inline]
    /// Get the bitmask for this timer channel's match bit.
    pub fn as_bitmask(self) -> u32 {
        1 << (self as u32)
    }
}

/// Convert usize to TimerChannel
impl core::convert::TryFrom<usize> for TimerChannel {
    type Error = ();

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(TimerChannel::Channel0),
            1 => Ok(TimerChannel::Channel1),
            2 => Ok(TimerChannel::Channel2),
            3 => Ok(TimerChannel::Channel3),
            _ => Err(()),
        }
    }
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

impl TimerRegisters {
    /// Get a pointer to the compare register for the given channel
    unsafe fn compare_reg_ptr(base: *mut TimerRegisters, channel: TimerChannel) -> *mut u32 {
        unsafe {
            match channel {
                TimerChannel::Channel0 => addr_of_mut!((*base).c0),
                TimerChannel::Channel1 => addr_of_mut!((*base).c1),
                TimerChannel::Channel2 => addr_of_mut!((*base).c2),
                TimerChannel::Channel3 => addr_of_mut!((*base).c3),
            }
        }
    }
}

/// Offset (in `u32` words) from the base of `TimerRegisters` to the first
/// compare register (`c0`).
///
/// Additional channels are addressed by adding the channel index.
const CHANNEL_OFFSET: usize = 0x3;

/// High-level interface to the system timer.
///
/// This type is a thin wrapper around a raw pointer to the memory-mapped
/// timer registers. It does not provide synchronization and assumes
/// single-writer or externally synchronized access.
pub struct Timer {
    regs: *mut TimerRegisters,
    channel_locks: [IrqSpinLock<(), ArmIrq>; 4],
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
                IrqSpinLock::new(()),
                IrqSpinLock::new(()),
                IrqSpinLock::new(()),
                IrqSpinLock::new(()),
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
        // Acquire lock FIRST
        let _guard = self.channel_locks[channel as usize].lock();

        unsafe {
            let clo_ptr = addr_of!((*self.regs).clo);
            let cs_ptr = addr_of_mut!((*self.regs).cs);
            let compare_reg = TimerRegisters::compare_reg_ptr(self.regs, channel);

            // Read current timer value
            let now = read_volatile(clo_ptr);

            // Clear any pending match (write-1-to-clear)
            write_volatile(cs_ptr, channel.as_bitmask());

            // Program compare value
            write_volatile(compare_reg, now.wrapping_add(interval_us));
        }
    }

    /// Clear all pending interrupts.
    pub fn clear_interrupt(&self) {
        unsafe {
            let cs_ptr = addr_of!((*self.regs).cs);
            let cs = read_volatile(cs_ptr);

            for i in 0..4 {
                if (cs & (1 << i)) != 0 {
                    let channel = match i {
                        0 => TimerChannel::Channel0,
                        1 => TimerChannel::Channel1,
                        2 => TimerChannel::Channel2,
                        3 => TimerChannel::Channel3,
                        _ => unreachable!(),
                    };
                    self.clear_interrupt_channel(channel);
                }
            }
        }
    }

    /// Clear a pending interrupt for the given channel.
    ///
    /// This acknowledges the interrupt by writing a `1` to the corresponding
    /// match bit in the control/status register.
    pub fn clear_interrupt_channel(&self, channel: TimerChannel) {
        let _guard = self.channel_locks[channel as usize].lock();
        unsafe {
            let cs_ptr = addr_of_mut!((*self.regs).cs);
            write_volatile(cs_ptr, channel.as_bitmask());
        }
    }
}

/// Global Timer instance
static TIMER: Timer = Timer::new();

/// Access the global Timer instance.
pub fn timer() -> &'static Timer {
    &TIMER
}
