/// PL011 UART base address for UART0 on BCM2835
pub const UART0_BASE: usize = 0x2020_1000;

/// PL011 clock frequency (48 MHz on BCM2835)
const PL011_CLOCK_HZ: u32 = 48_000_000;

// Flag Register (FR) bits
const FR_BUSY: u32 = 1 << 3;
const FR_TXFF: u32 = 1 << 5; // Transmit FIFO full
const FR_RXFE: u32 = 1 << 4; // Receive FIFO empty

// Control Register (CR) bits
const CR_UARTEN: u32 = 1 << 0; // UART enable
const CR_TXE: u32 = 1 << 8; // Transmit enable
const CR_RXE: u32 = 1 << 9; // Receive enable

// Line Control Register (LCRH) bits
const LCRH_WLEN_8: u32 = 0b11 << 5; // 8-bit word length
const LCRH_FEN: u32 = 1 << 4; // FIFO enable
const LCRH_STP2: u32 = 1 << 3; // Two stop bits

// Interrupt Mask Set/Clear Register (IMSC) bits
const IMSC_RXIM: u32 = 1 << 4; // Receive interrupt mask

// Interrupt FIFO Level Select Register (IFLS) bits
const IFLS_RXIFLSEL_7_8: u32 = 0b110 << 3; // RX FIFO 7/8 full

/// Memory-mapped PL011 UART register block
#[repr(C)]
pub struct Pl011Registers {
    pub dr: u32,      // 0x00: Data Register
    pub rsr_ecr: u32, // 0x04: Receive Status / Error Clear
    _reserved0: [u32; 4],
    pub fr: u32, // 0x18: Flag Register
    _reserved1: u32,
    pub ilpr: u32,  // 0x20: IrDA Low-Power Counter
    pub ibrd: u32,  // 0x24: Integer Baud Rate Divisor
    pub fbrd: u32,  // 0x28: Fractional Baud Rate Divisor
    pub lcrh: u32,  // 0x2C: Line Control Register
    pub cr: u32,    // 0x30: Control Register
    pub ifls: u32,  // 0x34: Interrupt FIFO Level Select
    pub imsc: u32,  // 0x38: Interrupt Mask Set/Clear
    pub ris: u32,   // 0x3C: Raw Interrupt Status
    pub mis: u32,   // 0x40: Masked Interrupt Status
    pub icr: u32,   // 0x44: Interrupt Clear
    pub dmacr: u32, // 0x48: DMA Control
}

// SAFETY: PL011 registers are memory-mapped hardware at a fixed address.
// Access is synchronized externally via spinlock.
unsafe impl Send for Pl011Registers {}
unsafe impl Sync for Pl011Registers {}

/// PL011 UART driver
pub struct Pl011 {
    regs: *mut Pl011Registers,
    initialized: bool,
}

impl Pl011 {
    /// Create a new PL011 UART instance
    ///
    /// # Safety
    /// - `base` must point to a valid PL011 UART peripheral
    /// - Only one instance should exist per UART hardware
    /// - Caller must ensure proper memory mapping is configured
    pub const unsafe fn new(base: usize) -> Self {
        Self {
            regs: base as *mut Pl011Registers,
            initialized: false,
        }
    }

    /// Initialize the UART with the given baud rate
    ///
    /// This configures the UART for 8N1 (8 data bits, no parity, 1 stop bit)
    /// and enables receive interrupts.
    pub fn init(&mut self, baud_rate: u32) -> Result<(), UartError> {
        if self.initialized {
            return Ok(());
        }

        unsafe {
            // Disable UART before configuration
            let cr_ptr = core::ptr::addr_of_mut!((*self.regs).cr);
            let mut cr = core::ptr::read_volatile(cr_ptr);
            cr &= !CR_UARTEN;
            core::ptr::write_volatile(cr_ptr, cr);

            // Wait for any ongoing transmission to complete
            while self.is_busy() {
                core::hint::spin_loop();
            }

            // Flush FIFOs by temporarily disabling them
            let lcrh_ptr = core::ptr::addr_of_mut!((*self.regs).lcrh);
            let mut lcrh = core::ptr::read_volatile(lcrh_ptr);
            lcrh &= !LCRH_FEN;
            core::ptr::write_volatile(lcrh_ptr, lcrh);

            // Calculate and set baud rate divisors
            let (ibrd, fbrd) = Self::calculate_divisors(baud_rate)?;
            let ibrd_ptr = core::ptr::addr_of_mut!((*self.regs).ibrd);
            let fbrd_ptr = core::ptr::addr_of_mut!((*self.regs).fbrd);
            core::ptr::write_volatile(ibrd_ptr, ibrd);
            core::ptr::write_volatile(fbrd_ptr, fbrd);

            // Configure line control: 8N1 with FIFOs enabled
            core::ptr::write_volatile(lcrh_ptr, LCRH_WLEN_8 | LCRH_FEN);

            // Clear all pending interrupts
            let icr_ptr = core::ptr::addr_of_mut!((*self.regs).icr);
            core::ptr::write_volatile(icr_ptr, 0x07FF);

            // Enable receive interrupt at 7/8 FIFO level
            let imsc_ptr = core::ptr::addr_of_mut!((*self.regs).imsc);
            let ifls_ptr = core::ptr::addr_of_mut!((*self.regs).ifls);
            core::ptr::write_volatile(imsc_ptr, IMSC_RXIM);
            core::ptr::write_volatile(ifls_ptr, IFLS_RXIFLSEL_7_8);

            // Enable UART, transmitter, and receiver
            core::ptr::write_volatile(cr_ptr, CR_UARTEN | CR_TXE | CR_RXE);
        }

        self.initialized = true;
        Ok(())
    }

    /// Write a single byte to the UART
    #[inline]
    pub fn write_byte(&self, byte: u8) {
        unsafe {
            let fr_ptr = core::ptr::addr_of!((*self.regs).fr);
            let dr_ptr = core::ptr::addr_of_mut!((*self.regs).dr);

            // Wait for TX FIFO to have space
            while (core::ptr::read_volatile(fr_ptr) & FR_TXFF) != 0 {
                core::hint::spin_loop();
            }

            core::ptr::write_volatile(dr_ptr, byte as u32);
        }
    }

    /// Write a string to the UART
    pub fn write_str(&self, s: &str) {
        for byte in s.bytes() {
            if byte == b'\n' {
                self.write_byte(b'\r'); // Convert LF to CRLF
            }
            self.write_byte(byte);
        }
    }

    /// Read a single byte from the UART (blocking)
    ///
    /// Returns the received byte
    pub fn read_byte(&self) -> u8 {
        unsafe {
            let fr_ptr = core::ptr::addr_of!((*self.regs).fr);
            let dr_ptr = core::ptr::addr_of!((*self.regs).dr);

            // Wait for data to be available
            while (core::ptr::read_volatile(fr_ptr) & FR_RXFE) != 0 {
                core::hint::spin_loop();
            }

            (core::ptr::read_volatile(dr_ptr) & 0xFF) as u8
        }
    }

    /// Try to read a byte from the UART (non-blocking)
    ///
    /// Returns `Some(byte)` if data is available, `None` otherwise
    pub fn try_read_byte(&self) -> Option<u8> {
        unsafe {
            let fr_ptr = core::ptr::addr_of!((*self.regs).fr);
            let dr_ptr = core::ptr::addr_of!((*self.regs).dr);

            if (core::ptr::read_volatile(fr_ptr) & FR_RXFE) != 0 {
                None
            } else {
                Some((core::ptr::read_volatile(dr_ptr) & 0xFF) as u8)
            }
        }
    }

    /// Check if the UART is busy transmitting
    #[inline]
    pub fn is_busy(&self) -> bool {
        unsafe {
            let fr_ptr = core::ptr::addr_of!((*self.regs).fr);
            (core::ptr::read_volatile(fr_ptr) & FR_BUSY) != 0
        }
    }

    /// Calculate integer and fractional baud rate divisors
    ///
    /// Formula: BAUDDIV = (FUARTCLK / (16 × Baud rate))
    /// Where BAUDDIV = IBRD + FBRD (FBRD is 6-bit fractional part)
    fn calculate_divisors(baud_rate: u32) -> Result<(u32, u32), UartError> {
        if baud_rate == 0 {
            return Err(UartError::InvalidBaudRate);
        }

        // Calculate divisor as fixed-point: (clock / (16 * baud)) * 64
        let divisor = ((PL011_CLOCK_HZ as u64) << 6) / (16 * baud_rate as u64);

        let integer = (divisor >> 6) as u32;
        let fractional = (divisor & 0x3F) as u32;

        if integer == 0 || integer > 0xFFFF {
            return Err(UartError::InvalidBaudRate);
        }

        Ok((integer, fractional))
    }
}

// SAFETY: Pl011 wraps memory-mapped hardware that can be safely
// accessed from any thread when protected by synchronization.
unsafe impl Send for Pl011 {}
unsafe impl Sync for Pl011 {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UartError {
    InvalidBaudRate,
}
