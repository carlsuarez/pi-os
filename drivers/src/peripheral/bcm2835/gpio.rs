//! BCM2835 GPIO Controller Driver
//!
//! This module provides both raw hardware access and HAL implementations
//! for the BCM2835 GPIO controller.

use crate::hal::gpio::{
    EdgeDetect, GpioController, GpioInterrupts, LevelDetect, PinLevel, PullMode,
};
use core::ptr::{read_volatile, write_volatile};

/// GPIO base address.
pub const GPIO_BASE: usize = 0x2020_0000;

/// GPIO function selection.
#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Function {
    Input = 0b000,
    Output = 0b001,
    Alt0 = 0b100,
    Alt1 = 0b101,
    Alt2 = 0b110,
    Alt3 = 0b111,
    Alt4 = 0b011,
    Alt5 = 0b010,
}

/// Internal pull resistor configuration.
#[repr(u32)]
#[derive(Copy, Clone, Debug)]
pub enum Pull {
    Off = 0b00,
    Down = 0b01,
    Up = 0b10,
}

impl From<PullMode> for Pull {
    fn from(mode: PullMode) -> Self {
        match mode {
            PullMode::None => Pull::Off,
            PullMode::Up => Pull::Up,
            PullMode::Down => Pull::Down,
        }
    }
}

/// GPIO event detection type.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Event {
    Rising,
    Falling,
    High,
    Low,
    AsyncRising,
    AsyncFalling,
}

/// Memory-mapped register layout.
#[repr(C)]
struct Registers {
    gpfsel: [u32; 6],
    _r0: u32,
    gpset: [u32; 2],
    _r1: u32,
    gpclr: [u32; 2],
    _r2: u32,
    gplev: [u32; 2],
    _r3: u32,
    gped: [u32; 2],
    _r4: u32,
    gpren: [u32; 2],
    _r5: u32,
    gpfen: [u32; 2],
    _r6: u32,
    gphen: [u32; 2],
    _r7: u32,
    gplen: [u32; 2],
    _r8: u32,
    gparen: [u32; 2],
    _r9: u32,
    gpafen: [u32; 2],
    _r10: u32,
    gppud: u32,
    gppudclk: [u32; 2],
}

#[inline(always)]
fn regs() -> *mut Registers {
    GPIO_BASE as *mut Registers
}

fn check_pin(pin: u8) -> Result<(), GpioError> {
    if pin <= 53 {
        Ok(())
    } else {
        Err(GpioError::InvalidPin)
    }
}

fn pin_reg_and_bit(pin: u8) -> (usize, u32) {
    let reg = (pin / 32) as usize;
    let bit = 1u32 << (pin % 32);
    (reg, bit)
}

fn delay_cycles(mut count: u32) {
    while count != 0 {
        unsafe { core::arch::asm!("nop") };
        count -= 1;
    }
}

// ============================================================================
// Raw Hardware Functions
// ============================================================================

/// Set the function of a GPIO pin.
pub fn set_function(pin: u8, func: Function) -> Result<(), GpioError> {
    check_pin(pin)?;

    let reg = (pin / 10) as usize;
    let shift = (pin % 10) * 3;
    let mask = 0b111 << shift;

    unsafe {
        let fsel = &mut (*regs()).gpfsel[reg];
        let val = read_volatile(fsel);
        let val = (val & !mask) | ((func as u32) << shift);
        write_volatile(fsel, val);
    }

    Ok(())
}

/// Drive a GPIO pin high.
pub fn set(pin: u8) -> Result<(), GpioError> {
    check_pin(pin)?;
    let (reg, bit) = pin_reg_and_bit(pin);

    unsafe {
        write_volatile(&mut (*regs()).gpset[reg], bit);
    }

    Ok(())
}

/// Drive a GPIO pin low.
pub fn clear(pin: u8) -> Result<(), GpioError> {
    check_pin(pin)?;
    let (reg, bit) = pin_reg_and_bit(pin);

    unsafe {
        write_volatile(&mut (*regs()).gpclr[reg], bit);
    }

    Ok(())
}

/// Read the current logic level of a pin.
pub fn level(pin: u8) -> Result<PinLevel, GpioError> {
    check_pin(pin)?;
    let (reg, bit) = pin_reg_and_bit(pin);

    unsafe {
        let val = read_volatile(&(*regs()).gplev[reg]);
        Ok(if val & bit != 0 {
            PinLevel::High
        } else {
            PinLevel::Low
        })
    }
}

/// Configure the internal pull resistor.
pub fn set_pull(pin: u8, pull: Pull) -> Result<(), GpioError> {
    check_pin(pin)?;
    let (reg, bit) = pin_reg_and_bit(pin);

    unsafe {
        let gppud = &mut (*regs()).gppud;
        let clk = &mut (*regs()).gppudclk[reg];

        write_volatile(gppud, pull as u32);
        delay_cycles(150);

        write_volatile(clk, bit);
        delay_cycles(150);

        write_volatile(gppud, 0);
        write_volatile(clk, 0);
    }

    Ok(())
}

/// Check if an event is pending.
pub fn event_status(pin: u8) -> Result<bool, GpioError> {
    check_pin(pin)?;
    let (reg, bit) = pin_reg_and_bit(pin);

    unsafe { Ok(read_volatile(&(*regs()).gped[reg]) & bit != 0) }
}

/// Clear a pending event.
pub fn clear_event(pin: u8) -> Result<(), GpioError> {
    check_pin(pin)?;
    let (reg, bit) = pin_reg_and_bit(pin);

    unsafe {
        write_volatile(&mut (*regs()).gped[reg], bit);
    }

    Ok(())
}

/// Configure event detection.
pub fn configure_event_detect(pin: u8, event: Event, enable: bool) -> Result<(), GpioError> {
    check_pin(pin)?;
    let (reg, bit) = pin_reg_and_bit(pin);

    unsafe {
        let reg_ptr = match event {
            Event::Rising => &mut (*regs()).gpren[reg],
            Event::Falling => &mut (*regs()).gpfen[reg],
            Event::High => &mut (*regs()).gphen[reg],
            Event::Low => &mut (*regs()).gplen[reg],
            Event::AsyncRising => &mut (*regs()).gparen[reg],
            Event::AsyncFalling => &mut (*regs()).gpafen[reg],
        };

        let mut val = read_volatile(reg_ptr);

        if enable {
            val |= bit;
        } else {
            val &= !bit;
        }

        write_volatile(reg_ptr, val);
    }

    Ok(())
}

// ============================================================================
// HAL Implementation
// ============================================================================

/// BCM2835 GPIO controller.
#[derive(Debug)]
pub struct Bcm2835Gpio;

impl Bcm2835Gpio {
    /// Create a new GPIO controller.
    ///
    /// # Safety
    ///
    /// GPIO registers must be properly mapped.
    pub const unsafe fn new() -> Self {
        Self
    }

    /// Configure a pin for a specific alternate function.
    pub fn set_alt_function(&mut self, pin: u8, alt: u8) -> Result<(), GpioError> {
        let func = match alt {
            0 => Function::Alt0,
            1 => Function::Alt1,
            2 => Function::Alt2,
            3 => Function::Alt3,
            4 => Function::Alt4,
            5 => Function::Alt5,
            _ => return Err(GpioError::InvalidFunction),
        };
        set_function(pin, func)
    }

    /// Configure a pin as input.
    pub fn set_input(&mut self, pin: u8) -> Result<(), GpioError> {
        set_function(pin, Function::Input)
    }

    /// Configure a pin as output.
    pub fn set_output(&mut self, pin: u8) -> Result<(), GpioError> {
        set_function(pin, Function::Output)
    }
}

impl GpioController for Bcm2835Gpio {
    type Pin = u8;
    type Error = GpioError;

    fn set_pull(&mut self, pin: Self::Pin, pull: PullMode) -> Result<(), Self::Error> {
        set_pull(pin, pull.into())
    }

    fn set_high(&mut self, pin: Self::Pin) -> Result<(), Self::Error> {
        set(pin)
    }

    fn set_low(&mut self, pin: Self::Pin) -> Result<(), Self::Error> {
        clear(pin)
    }

    fn read(&self, pin: Self::Pin) -> Result<PinLevel, Self::Error> {
        level(pin)
    }
}

impl GpioInterrupts for Bcm2835Gpio {
    fn enable_edge_detect(&mut self, pin: Self::Pin, edge: EdgeDetect) -> Result<(), Self::Error> {
        match edge {
            EdgeDetect::Rising => configure_event_detect(pin, Event::Rising, true),
            EdgeDetect::Falling => configure_event_detect(pin, Event::Falling, true),
            EdgeDetect::Both => {
                configure_event_detect(pin, Event::Rising, true)?;
                configure_event_detect(pin, Event::Falling, true)
            }
        }
    }

    fn disable_edge_detect(&mut self, pin: Self::Pin) -> Result<(), Self::Error> {
        configure_event_detect(pin, Event::Rising, false)?;
        configure_event_detect(pin, Event::Falling, false)
    }

    fn enable_level_detect(
        &mut self,
        pin: Self::Pin,
        level: LevelDetect,
    ) -> Result<(), Self::Error> {
        match level {
            LevelDetect::High => configure_event_detect(pin, Event::High, true),
            LevelDetect::Low => configure_event_detect(pin, Event::Low, true),
        }
    }

    fn disable_level_detect(&mut self, pin: Self::Pin) -> Result<(), Self::Error> {
        configure_event_detect(pin, Event::High, false)?;
        configure_event_detect(pin, Event::Low, false)
    }

    fn event_pending(&self, pin: Self::Pin) -> Result<bool, Self::Error> {
        event_status(pin)
    }

    fn clear_event(&mut self, pin: Self::Pin) -> Result<(), Self::Error> {
        clear_event(pin)
    }
}

/// GPIO errors.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum GpioError {
    InvalidPin,
    InvalidFunction,
}
