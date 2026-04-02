use crate::hal::serial::{
    DataBits, NonBlockingSerial, Parity, SerialConfig, SerialError, SerialPort, StopBits,
};
use core::marker::PhantomData;
use core::ptr::{read_volatile, write_volatile};

// ============================================================================
// I/O Abstraction
// ============================================================================

pub trait Io {
    fn read8(addr: usize) -> u8;
    fn write8(addr: usize, val: u8);
}

// ---------------- MMIO (portable) ----------------

pub struct Mmio;

impl Io for Mmio {
    #[inline]
    fn read8(addr: usize) -> u8 {
        unsafe { read_volatile(addr as *const u8) }
    }

    #[inline]
    fn write8(addr: usize, val: u8) {
        unsafe { write_volatile(addr as *mut u8, val) }
    }
}

// ---------------- PIO (x86 only) ----------------

#[cfg(target_arch = "x86")]
pub struct Pio;

#[cfg(target_arch = "x86")]
impl Io for Pio {
    #[inline]
    fn read8(addr: usize) -> u8 {
        unsafe { x86::io::inb(addr as u16) }
    }

    #[inline]
    fn write8(addr: usize, val: u8) {
        unsafe { x86::io::outb(addr as u16, val) }
    }
}

// ============================================================================
// Constants
// ============================================================================

const UART_CLOCK_HZ: u32 = 1_843_200;

// Registers
const RBR: usize = 0;
const THR: usize = 0;
const DLL: usize = 0;
const DLM: usize = 1;
const IER: usize = 1;
const FCR: usize = 2;
const LCR: usize = 3;
const MCR: usize = 4;
const LSR: usize = 5;

// Bits
const LCR_DLAB: u8 = 1 << 7;

const FCR_ENABLE: u8 = 1 << 0;
const FCR_CLEAR_RX: u8 = 1 << 1;
const FCR_CLEAR_TX: u8 = 1 << 2;

const MCR_DTR: u8 = 1 << 0;
const MCR_RTS: u8 = 1 << 1;
const MCR_OUT2: u8 = 1 << 3;

const LSR_DR: u8 = 1 << 0;
const LSR_OE: u8 = 1 << 1;
const LSR_PE: u8 = 1 << 2;
const LSR_FE: u8 = 1 << 3;
const LSR_BI: u8 = 1 << 4;
const LSR_THRE: u8 = 1 << 5;
const LSR_TEMT: u8 = 1 << 6;

// ============================================================================
// Error
// ============================================================================

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Uart16550Error {
    Framing,
    Parity,
    Overrun,
    Break,
    WouldBlock,
    InvalidConfig,
}

impl From<Uart16550Error> for SerialError {
    fn from(e: Uart16550Error) -> Self {
        match e {
            Uart16550Error::Framing => SerialError::Framing,
            Uart16550Error::Parity => SerialError::Parity,
            Uart16550Error::Overrun => SerialError::Overrun,
            Uart16550Error::Break => SerialError::Break,
            Uart16550Error::WouldBlock => SerialError::WouldBlock,
            Uart16550Error::InvalidConfig => SerialError::InvalidConfig,
        }
    }
}

// ============================================================================
// Driver
// ============================================================================

pub struct Uart16550<I: Io> {
    base: usize,
    _io: PhantomData<I>,
}

impl<I: Io> Uart16550<I> {
    pub const fn new(base: usize) -> Self {
        Self {
            base,
            _io: PhantomData,
        }
    }

    #[inline]
    fn read_reg(&self, offset: usize) -> u8 {
        I::read8(self.base + offset)
    }

    #[inline]
    fn write_reg(&mut self, offset: usize, val: u8) {
        I::write8(self.base + offset, val)
    }

    fn set_baud(&mut self, baud: u32) -> Result<(), Uart16550Error> {
        if baud == 0 {
            return Err(Uart16550Error::InvalidConfig);
        }

        let divisor = UART_CLOCK_HZ / (16 * baud);
        if divisor == 0 || divisor > 0xFFFF {
            return Err(Uart16550Error::InvalidConfig);
        }

        let lcr = self.read_reg(LCR);

        self.write_reg(LCR, lcr | LCR_DLAB);
        self.write_reg(DLL, (divisor & 0xFF) as u8);
        self.write_reg(DLM, (divisor >> 8) as u8);
        self.write_reg(LCR, lcr & !LCR_DLAB);

        Ok(())
    }

    fn check_errors(lsr: u8) -> Result<(), Uart16550Error> {
        if lsr & LSR_OE != 0 {
            return Err(Uart16550Error::Overrun);
        }
        if lsr & LSR_PE != 0 {
            return Err(Uart16550Error::Parity);
        }
        if lsr & LSR_FE != 0 {
            return Err(Uart16550Error::Framing);
        }
        if lsr & LSR_BI != 0 {
            return Err(Uart16550Error::Break);
        }
        Ok(())
    }
}

// ============================================================================
// SerialPort
// ============================================================================

impl<I: Io> SerialPort for Uart16550<I> {
    type Error = Uart16550Error;

    fn configure(&mut self, config: SerialConfig) -> Result<(), Self::Error> {
        if !matches!(config.data_bits, DataBits::Eight)
            || !matches!(config.parity, Parity::None)
            || !matches!(config.stop_bits, StopBits::One)
        {
            return Err(Uart16550Error::InvalidConfig);
        }

        self.write_reg(IER, 0x00);
        self.write_reg(LCR, 0x03);

        self.set_baud(config.baud_rate)?;

        self.write_reg(FCR, FCR_ENABLE | FCR_CLEAR_RX | FCR_CLEAR_TX);
        self.write_reg(MCR, MCR_DTR | MCR_RTS | MCR_OUT2);

        Ok(())
    }

    fn write_byte(&mut self, byte: u8) -> Result<(), Self::Error> {
        while self.read_reg(LSR) & LSR_THRE == 0 {
            core::hint::spin_loop();
        }

        self.write_reg(THR, byte);
        Ok(())
    }

    fn read_byte(&mut self) -> Result<u8, Self::Error> {
        loop {
            let lsr = self.read_reg(LSR);

            if lsr & LSR_DR != 0 {
                Self::check_errors(lsr)?;
                return Ok(self.read_reg(RBR));
            }

            core::hint::spin_loop();
        }
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        while self.read_reg(LSR) & LSR_TEMT == 0 {
            core::hint::spin_loop();
        }
        Ok(())
    }

    fn is_busy(&self) -> bool {
        self.read_reg(LSR) & LSR_TEMT == 0
    }
}

// ============================================================================
// Non-blocking
// ============================================================================

impl<I: Io> NonBlockingSerial for Uart16550<I> {
    fn try_write_byte(&mut self, byte: u8) -> Result<(), Self::Error> {
        if self.read_reg(LSR) & LSR_THRE == 0 {
            return Err(Uart16550Error::WouldBlock);
        }

        self.write_reg(THR, byte);
        Ok(())
    }

    fn try_read_byte(&mut self) -> Result<u8, Self::Error> {
        let lsr = self.read_reg(LSR);

        if lsr & LSR_DR == 0 {
            return Err(Uart16550Error::WouldBlock);
        }

        Self::check_errors(lsr)?;
        Ok(self.read_reg(RBR))
    }
}

// ============================================================================
// Safety
// ============================================================================

unsafe impl<I: Io> Send for Uart16550<I> {}
unsafe impl<I: Io> Sync for Uart16550<I> {}
