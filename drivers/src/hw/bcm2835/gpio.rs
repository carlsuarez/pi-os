use core::ptr::{addr_of, addr_of_mut, read_volatile, write_volatile};

/// Base physical address of the GPIO controller.
///
/// This corresponds to the BCM2835 peripheral base for GPIO
/// The caller must ensure this address is correctly mapped into the
/// kernel’s virtual address space before use.
pub const GPIO_BASE: usize = 0x2020_0000;

/// GPIO pin function selection.
///
/// Each GPIO pin can be configured as an input, output,
/// or one of several alternate functions (ALT0–ALT5),
/// depending on the SoC peripheral muxing.
#[repr(u32)]
#[derive(Copy, Clone)]
pub enum FuncSelect {
    /// Pin is configured as an input.
    Input = 0b000,
    /// Pin is configured as a push-pull output.
    Output = 0b001,
    /// Alternate function 0.
    Alt0 = 0b100,
    /// Alternate function 1.
    Alt1 = 0b101,
    /// Alternate function 2.
    Alt2 = 0b110,
    /// Alternate function 3.
    Alt3 = 0b111,
    /// Alternate function 4.
    Alt4 = 0b011,
    /// Alternate function 5.
    Alt5 = 0b010,
}

/// GPIO internal pull-up / pull-down resistor configuration.
#[repr(u32)]
#[derive(Copy, Clone)]
pub enum Pull {
    /// No pull-up or pull-down resistor.
    Off = 0b00,
    /// Enable pull-down resistor.
    Down = 0b01,
    /// Enable pull-up resistor.
    Up = 0b10,
}

/// Memory-mapped register layout of the GPIO controller.
///
/// This structure mirrors the hardware register block as described
/// in the BCM2835 ARM Peripherals documentation. Reserved fields are
/// included to preserve correct offsets between registers.
///
/// # Safety
/// This struct must only be accessed through volatile reads/writes,
/// as the memory region represents hardware registers.
#[repr(C)]
struct GpioRegs {
    /// GPIO Function Select registers (GPFSEL0–GPFSEL5).
    gpfsel: [u32; 6],
    _reserved0: u32,

    /// GPIO Pin Output Set registers (GPSET0–GPSET1).
    gpset: [u32; 2],
    _reserved1: u32,

    /// GPIO Pin Output Clear registers (GPCLR0–GPCLR1).
    gpclr: [u32; 2],
    _reserved2: u32,

    /// GPIO Pin Level registers (GPLEV0–GPLEV1).
    gplev: [u32; 2],
    _reserved3: u32,

    /// GPIO Pin Rising Edge Detect Enable registers.
    gpren: [u32; 2],
    _reserved4: u32,

    /// GPIO Pin Falling Edge Detect Enable registers.
    gpfen: [u32; 2],
    _reserved5: u32,

    /// GPIO Pin High Detect Enable registers.
    gphen: [u32; 2],
    _reserved6: u32,

    /// GPIO Pin Low Detect Enable registers.
    gplen: [u32; 2],
    _reserved7: u32,

    /// GPIO Pin Asynchronous Rising Edge Detect Enable registers.
    gparen: [u32; 2],
    _reserved8: u32,

    /// GPIO Pull-up/down Enable register.
    gppud: u32,

    /// GPIO Pull-up/down Enable Clock registers.
    gppudclk: [u32; 2],
}

/// GPIO controller abstraction.
///
/// Provides safe, typed accessors for configuring and controlling
/// GPIO pins. All hardware interaction is performed using volatile
/// memory accesses.
pub struct Gpio {
    /// Pointer to the memory-mapped GPIO register block.
    regs: *mut GpioRegs,
}

impl Gpio {
    /// Create a new GPIO controller instance.
    ///
    /// # Safety
    /// The caller must ensure that `base` is the correct physical or
    /// virtual address of the GPIO register block and that it is
    /// valid for the lifetime of this object.
    pub const unsafe fn new(base: usize) -> Self {
        Self {
            regs: base as *mut GpioRegs,
        }
    }

    /// Validate that a GPIO pin number is in range.
    ///
    /// The BCM2835 exposes GPIO pins 0–53.
    fn check_pin(pin: u8) -> Result<(), GpioError> {
        if pin > 53 {
            Err(GpioError::BadPin)
        } else {
            Ok(())
        }
    }

    /// Configure the function of a GPIO pin.
    ///
    /// This updates the appropriate `GPFSELn` register to set the pin
    /// as an input, output, or alternate function.
    pub fn set_function(&self, pin: u8, func: FuncSelect) -> Result<(), GpioError> {
        Self::check_pin(pin)?;

        let reg = (pin / 10) as usize;
        let shift = (pin % 10) * 3;
        let mask = 0b111 << shift;

        unsafe {
            let fsel_ptr = addr_of!((*self.regs).gpfsel).cast::<u32>().add(reg);

            let fsel = read_volatile(fsel_ptr);
            let new = (fsel & !mask) | ((func as u32) << shift);
            write_volatile(fsel_ptr as *mut u32, new);
        }

        Ok(())
    }

    /// Drive a GPIO pin high.
    ///
    /// The pin must be configured as an output for this to have an effect.
    pub fn set(&self, pin: u8) -> Result<(), GpioError> {
        Self::check_pin(pin)?;

        let reg = (pin / 32) as usize;
        let bit = 1u32 << (pin % 32);

        unsafe {
            let gpset_ptr = addr_of!((*self.regs).gpset).cast::<u32>().add(reg);
            write_volatile(gpset_ptr as *mut u32, bit);
        }

        Ok(())
    }

    /// Drive a GPIO pin low.
    ///
    /// The pin must be configured as an output for this to have an effect.
    pub fn clear(&self, pin: u8) -> Result<(), GpioError> {
        Self::check_pin(pin)?;

        let reg = (pin / 32) as usize;
        let bit = 1u32 << (pin % 32);

        unsafe {
            let gpclr_ptr = addr_of!((*self.regs).gpclr).cast::<u32>().add(reg);
            write_volatile(gpclr_ptr as *mut u32, bit);
        }

        Ok(())
    }

    /// Read the current logic level of a GPIO pin.
    ///
    /// Returns [`PinLevel::High`] if the pin is asserted, otherwise
    /// [`PinLevel::Low`].
    pub fn level(&self, pin: u8) -> Result<PinLevel, GpioError> {
        Self::check_pin(pin)?;

        let reg = (pin / 32) as usize;
        let bit = 1u32 << (pin % 32);

        unsafe {
            let gplev_ptr = addr_of!((*self.regs).gplev).cast::<u32>().add(reg);
            let val = read_volatile(gplev_ptr);

            Ok(if val & bit != 0 {
                PinLevel::High
            } else {
                PinLevel::Low
            })
        }
    }

    /// Configure the internal pull-up or pull-down resistor for a pin.
    ///
    /// This follows the required GPIO pull-up/down programming sequence:
    /// 1. Write the desired pull state to `GPPUD`.
    /// 2. Wait for the control signal to settle.
    /// 3. Clock the setting into the target pin via `GPPUDCLK`.
    /// 4. Clear both registers.
    pub fn set_pull(&self, pin: u8, pull: Pull) -> Result<(), GpioError> {
        Self::check_pin(pin)?;

        let reg = (pin / 32) as usize;
        let bit = 1u32 << (pin % 32);

        unsafe {
            let gppud_ptr = addr_of_mut!((*self.regs).gppud);
            let gppudclk_ptr = addr_of!((*self.regs).gppudclk).cast::<u32>().add(reg);

            write_volatile(gppud_ptr, pull as u32);
            delay_cycles(150);

            write_volatile(gppudclk_ptr as *mut u32, bit);
            delay_cycles(150);

            write_volatile(gppud_ptr, 0);
            write_volatile(gppudclk_ptr as *mut u32, 0);
        }

        Ok(())
    }
}

/// Errors that can occur when operating on GPIO pins.
#[derive(Debug)]
pub enum GpioError {
    /// The requested pin number is outside the valid range.
    BadPin,
}

/// Logical level of a GPIO pin.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PinLevel {
    /// Logic low (0).
    Low,
    /// Logic high (1).
    High,
}

/// Simple busy-wait delay loop.
///
/// This is used for short hardware timing requirements and
/// provides no guarantees about real-time accuracy.
#[inline(always)]
fn delay_cycles(mut count: u32) {
    while count != 0 {
        unsafe { core::arch::asm!("nop") };
        count -= 1;
    }
}
