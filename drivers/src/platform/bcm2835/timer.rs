//! BCM2835 System Timer Driver
//!
//! The BCM2835 has a 64-bit free-running counter at 1MHz and
//! four compare channels that can generate interrupts.

use crate::hal::timer::{CountingTimer, Timer};
use core::ptr::{read_volatile, write_volatile};

/// System timer base address.
pub const TIMER_BASE: usize = 0x2000_3000;

/// System timer compare channels.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum Channel {
    Channel0 = 0,
    Channel1 = 1,
    Channel2 = 2,
    Channel3 = 3,
}

impl Channel {
    /// Get the IRQ number for this channel.
    pub fn irq_number(self) -> u32 {
        match self {
            Channel::Channel0 => 0,
            Channel::Channel1 => 1,
            Channel::Channel2 => 2,
            Channel::Channel3 => 3,
        }
    }

    fn bitmask(self) -> u32 {
        1 << (self as u32)
    }
}

/// Memory-mapped system timer registers.
#[repr(C)]
struct Registers {
    cs: u32,
    clo: u32,
    chi: u32,
    c0: u32,
    c1: u32,
    c2: u32,
    c3: u32,
}

#[inline(always)]
fn regs() -> *mut Registers {
    TIMER_BASE as *mut Registers
}

fn compare_reg_ptr(channel: Channel) -> *mut u32 {
    unsafe {
        match channel {
            Channel::Channel0 => &mut (*regs()).c0,
            Channel::Channel1 => &mut (*regs()).c1,
            Channel::Channel2 => &mut (*regs()).c2,
            Channel::Channel3 => &mut (*regs()).c3,
        }
    }
}

// ============================================================================
// Raw Hardware Functions
// ============================================================================

/// Read the 64-bit free-running counter.
pub fn read_counter() -> u64 {
    unsafe {
        // Read high word first for consistency
        let hi1 = read_volatile(&(*regs()).chi);
        let lo = read_volatile(&(*regs()).clo);
        let hi2 = read_volatile(&(*regs()).chi);

        // If high word changed, re-read low word
        let (hi, lo) = if hi1 != hi2 {
            (hi2, read_volatile(&(*regs()).clo))
        } else {
            (hi1, lo)
        };

        ((hi as u64) << 32) | (lo as u64)
    }
}

/// Arm a timer compare interrupt.
pub fn start_timer(channel: Channel, interval_us: u32) {
    unsafe {
        let clo = read_volatile(&(*regs()).clo);
        let cmp_ptr = compare_reg_ptr(channel);

        // Clear pending match
        write_volatile(&mut (*regs()).cs, channel.bitmask());

        // Program compare register
        write_volatile(cmp_ptr, clo.wrapping_add(interval_us));
    }
}

/// Clear a pending interrupt.
pub fn clear_interrupt(channel: Channel) {
    unsafe {
        write_volatile(&mut (*regs()).cs, channel.bitmask());
    }
}

/// Check if an interrupt is pending.
pub fn is_pending(channel: Channel) -> bool {
    unsafe { read_volatile(&(*regs()).cs) & channel.bitmask() != 0 }
}

// ============================================================================
// HAL Implementation
// ============================================================================

/// BCM2835 system timer.
#[derive(Debug)]
pub struct Bcm2835Timer;

impl Bcm2835Timer {
    /// Create a new timer.
    ///
    /// # Safety
    ///
    /// Timer registers must be properly mapped.
    pub const unsafe fn new() -> Self {
        Self
    }
}

/// Timer errors (BCM2835 timer operations are infallible).
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TimerError {}

impl Timer for Bcm2835Timer {
    type Handle = Channel;
    type Error = TimerError;

    fn start(&mut self, handle: Self::Handle, interval_us: u32) -> Result<(), Self::Error> {
        start_timer(handle, interval_us);
        Ok(())
    }

    fn stop(&mut self, handle: Self::Handle) -> Result<(), Self::Error> {
        clear_interrupt(handle);
        Ok(())
    }

    fn clear_interrupt(&mut self, handle: Self::Handle) -> Result<(), Self::Error> {
        clear_interrupt(handle);
        Ok(())
    }

    fn is_pending(&self, handle: Self::Handle) -> Result<bool, Self::Error> {
        Ok(is_pending(handle))
    }
}

impl CountingTimer for Bcm2835Timer {
    fn now_us(&self) -> u64 {
        read_counter()
    }
}
