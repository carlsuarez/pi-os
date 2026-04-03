use crate::hal::timer::{CountingTimer, PeriodicTimer, Timer, TimerError};

const PIT_CHANNEL_0: u16 = 0x40;
const PIT_CHANNEL_1: u16 = 0x41;
const PIT_CHANNEL_2: u16 = 0x42;
const PIT_MODE_CMD: u16 = 0x43;

const PIT_FREQUENCY_HZ: u32 = 1_193_182;

const ACCESS_LOHI: u8 = 0b11 << 4;
const MODE_RATE: u8 = 0b010 << 1; // periodic
const MODE_ONESHOT: u8 = 0b001 << 1;
const BCD_BINARY: u8 = 0;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PitChannel {
    Channel0 = 0, // IRQ0
    Channel1 = 1, // legacy, avoid
    Channel2 = 2, // PC speaker
}

impl From<usize> for PitChannel {
    fn from(n: usize) -> Self {
        match n {
            1 => Self::Channel1,
            2 => Self::Channel2,
            _ => Self::Channel0,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum I8254PITError {
    InvalidHandle,
    HardwareAccessError,
}

impl From<I8254PITError> for TimerError {
    fn from(e: I8254PITError) -> Self {
        match e {
            I8254PITError::InvalidHandle => Self::InvalidHandle,
            I8254PITError::HardwareAccessError => Self::Hardware,
        }
    }
}

pub struct I8254PIT {
    /// Accumulated microseconds for CountingTimer, updated each time
    /// we program the PIT.  Not a free-running hardware counter — just
    /// good enough for delay_us / delay_ms.
    elapsed_us: u64,
}

impl I8254PIT {
    pub const fn new() -> Self {
        Self { elapsed_us: 0 }
    }

    fn data_port(channel: PitChannel) -> u16 {
        match channel {
            PitChannel::Channel0 => PIT_CHANNEL_0,
            PitChannel::Channel1 => PIT_CHANNEL_1,
            PitChannel::Channel2 => PIT_CHANNEL_2,
        }
    }

    fn program(channel: PitChannel, mode_bits: u8, divisor: u16) {
        let sel = (channel as u8) << 6;
        let cmd = sel | ACCESS_LOHI | mode_bits | BCD_BINARY;
        unsafe {
            x86::io::outb(PIT_MODE_CMD, cmd);
            x86::io::outb(Self::data_port(channel), (divisor & 0xFF) as u8);
            x86::io::outb(Self::data_port(channel), (divisor >> 8) as u8);
        }
    }

    fn us_to_divisor(interval_us: u32) -> u16 {
        let ticks = (interval_us as u64 * PIT_FREQUENCY_HZ as u64) / 1_000_000;
        ticks.clamp(1, 65535) as u16
    }

    /// Read the current count from a channel using the latch command.
    /// Returns the raw 16-bit counter value (counts down from divisor).
    fn read_count(channel: PitChannel) -> u16 {
        let sel = (channel as u8) << 6;
        unsafe {
            // Latch command: access=00, mode bits don't matter
            x86::io::outb(PIT_MODE_CMD, sel);
            let lo = x86::io::inb(Self::data_port(channel)) as u16;
            let hi = x86::io::inb(Self::data_port(channel)) as u16;
            lo | (hi << 8)
        }
    }
}

impl Timer for I8254PIT {
    type Handle = PitChannel;
    type Error = I8254PITError;

    fn start(&mut self, handle: PitChannel, interval_us: u32) -> Result<(), I8254PITError> {
        let divisor = Self::us_to_divisor(interval_us);
        Self::program(handle, MODE_ONESHOT, divisor);
        Ok(())
    }

    fn stop(&mut self, _handle: PitChannel) -> Result<(), I8254PITError> {
        // No true halt in the i8254 — write max divisor to slow to minimum rate.
        // Caller should also mask IRQ0 at the PIC if they want silence.
        Self::program(PitChannel::Channel0, MODE_ONESHOT, 0);
        Ok(())
    }

    fn clear_interrupt(&mut self, _handle: PitChannel) -> Result<(), I8254PITError> {
        // PIT channel 0 interrupt is cleared by EOI to the PIC, not the PIT.
        // Nothing to do here.
        Ok(())
    }

    fn is_pending(&self, handle: PitChannel) -> Result<bool, I8254PITError> {
        // Read-back command latches status byte for the selected channel.
        // Bit 7 of the status = output pin state (1 = expired / IRQ asserted).
        let sel = 0xE0u8 | (1 << (handle as u8 + 1));
        unsafe {
            x86::io::outb(PIT_MODE_CMD, sel);
            let status = x86::io::inb(Self::data_port(handle));
            Ok((status & 0x80) != 0)
        }
    }
}

impl PeriodicTimer for I8254PIT {
    fn start_periodic(
        self: &mut Self,
        handle: PitChannel,
        interval_us: u32,
    ) -> Result<(), I8254PITError> {
        let divisor = Self::us_to_divisor(interval_us);
        Self::program(handle, MODE_RATE, divisor);
        Ok(())
    }
}

impl CountingTimer for I8254PIT {
    /// Approximate microsecond timestamp derived from Channel 0's
    /// current count.  Programs Channel 0 in rate-generator mode at
    /// 1MHz-equivalent and reads back the latch.
    ///
    /// For a proper free-running clock you'd accumulate ticks in the
    /// IRQ0 handler; this is a lightweight polling alternative.
    fn now_us(&self) -> u64 {
        // Latch and convert the current Channel 0 count to microseconds.
        // The count decrements at PIT_FREQUENCY_HZ ticks/sec.
        let count = Self::read_count(PitChannel::Channel0) as u64;
        // Invert: count=65535 means just started, count=0 means expired.
        let ticks_elapsed = 65535u64.saturating_sub(count);
        self.elapsed_us + (ticks_elapsed * 1_000_000 / PIT_FREQUENCY_HZ as u64)
    }
}

unsafe impl Send for I8254PIT {}
unsafe impl Sync for I8254PIT {}
