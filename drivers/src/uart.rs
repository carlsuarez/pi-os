use crate::hw::pl011::*;
use core::ptr::{read_volatile, write_volatile};
use core::{cell::UnsafeCell, ptr::NonNull};

#[derive(Debug)]
pub enum UartError {
    InvalidBaudRate,
}

pub struct Uart {
    regs: UnsafeCell<NonNull<Pl011>>,
}

unsafe impl Sync for Uart {}

impl Uart {
    /// # Safety
    /// `base` must be a valid PL011 MMIO base address and must not be
    /// instantiated more than once.
    const unsafe fn new(base: usize) -> Self {
        unsafe {
            Self {
                regs: UnsafeCell::new(NonNull::new_unchecked(base as *mut Pl011)),
            }
        }
    }

    #[inline(always)]
    fn regs(&self) -> *mut Pl011 {
        unsafe { self.regs.get().read().as_ptr() }
    }

    pub fn init(&self, baud_rate: u32) -> Result<(), UartError> {
        unsafe {
            let r = self.regs();

            // Disable UART
            let mut cr = read_volatile(&(*r).cr);
            cr &= !UART_CR_UARTEN;
            write_volatile(&mut (*r).cr, cr);

            while read_volatile(&(*r).fr) & UART_FR_BUSY != 0 {}

            // Disable FIFOs
            let mut lcrh = read_volatile(&(*r).lcrh);
            lcrh &= !UART_LCRH_FEN;
            write_volatile(&mut (*r).lcrh, lcrh);

            // Set baud rate
            let (ibrd, fbrd) = calculate_divisors(baud_rate)?;
            write_volatile(&mut (*r).ibrd, ibrd);
            write_volatile(&mut (*r).fbrd, fbrd);

            // 8N1
            write_volatile(&mut (*r).lcrh, UART_LCRH_WLEN_8);

            // Clear interrupts
            write_volatile(&mut (*r).icr, 0x03FF);

            // Enable RX interrupt
            write_volatile(&mut (*r).imsc, UART_IMSC_RXIM);

            // FIFO trigger level
            write_volatile(&mut (*r).ifls, UART_IFLS_RXIFLSEL_7_8);

            // Enable TX, RX, UART
            write_volatile(&mut (*r).cr, UART_CR_TXE | UART_CR_RXE | UART_CR_UARTEN);

            Ok(())
        }
    }

    pub fn putc(&self, c: u8) {
        unsafe {
            let r = self.regs();
            while read_volatile(&(*r).fr) & UART_FR_BUSY != 0 {}
            write_volatile(&mut (*r).dr, c as u32);
        }
    }

    pub fn puts(&self, s: &str) {
        for b in s.bytes() {
            self.putc(b);
        }
    }

    pub fn puthex(&self, val: u32) {
        for i in (0..8).rev() {
            let nibble = (val >> (i * 4)) & 0xF;
            let c = match nibble {
                0..=9 => b'0' + nibble as u8,
                _ => b'A' + (nibble as u8 - 10),
            };
            self.putc(c);
        }
    }
}

static UART0: Uart = unsafe { Uart::new(UART0_BASE) };

pub fn uart0() -> &'static Uart {
    &UART0
}

fn calculate_divisors(baud_rate: u32) -> Result<(u32, u32), UartError> {
    let temp = (4u64 * PL011_CLOCK as u64) << 6;
    let div = temp / baud_rate as u64;

    let integer = ((div >> 6) & 0xffff) as u32;
    let fractional = (div & 0x3f) as u32;

    if integer == 0 {
        return Err(UartError::InvalidBaudRate);
    }

    Ok((integer, fractional))
}
