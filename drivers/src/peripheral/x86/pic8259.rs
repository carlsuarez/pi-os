use crate::{
    hal::interrupt::{InterruptController, InterruptError},
    io::{Io, Pio},
};

// ============================================================================
// Constants
// ============================================================================

const PIC1_COMMAND: u16 = 0x20;
const PIC1_DATA: u16 = 0x21;
const PIC2_COMMAND: u16 = 0xA0;
const PIC2_DATA: u16 = 0xA1;

const ICW1_ICW4: u8 = 0x01;
const ICW1_INIT: u8 = 0x10;

const ICW4_8086: u8 = 0x01;

const OCW3_READ_IRR: u8 = 0x0A;
const OCW3_READ_ISR: u8 = 0x0B;

const EOI: u8 = 0x20;

const SPURIOUS_IRQ_MASTER: u8 = 7;
const SPURIOUS_IRQ_SLAVE: u8 = 15;

pub const PIC_MASTER_OFFSET: u8 = 0x20;
pub const PIC_SLAVE_OFFSET: u8 = 0x28;

// ============================================================================
// Error
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PicError {
    InvalidIrq(u8),
    SpuriousIrq(u8),
}

impl From<PicError> for InterruptError {
    fn from(e: PicError) -> Self {
        match e {
            PicError::InvalidIrq(_) => InterruptError::InvalidIrq,
            PicError::SpuriousIrq(_) => InterruptError::Other,
        }
    }
}

// ============================================================================
// Driver
// ============================================================================

pub struct Pic8259 {
    master_offset: u8,
    slave_offset: u8,
}

impl Pic8259 {
    /// Initialize and remap the PIC pair.
    ///
    /// `master_offset` and `slave_offset` are the base CPU vector numbers for
    /// IRQ 0–7 and IRQ 8–15 respectively. Conventionally 0x20 and 0x28.
    ///
    /// # Safety
    /// Must only be called once, before `STI`. Calling this while interrupts
    /// are enabled will produce spurious or lost interrupts mid-sequence.
    pub unsafe fn new(master_offset: u8, slave_offset: u8) -> Self {
        let pic = Self {
            master_offset,
            slave_offset,
        };
        unsafe { pic.remap() };
        pic
    }

    // -------------------------------------------------------------------------
    // Initialization
    // -------------------------------------------------------------------------

    /// Remap both PICs and restore the saved IMR masks.
    ///
    /// Called by `new()`. Saves and restores the IMR so that any masks set
    /// before remapping survive the re-initialization.
    unsafe fn remap(&self) {
        // Save current masks — these are lost during ICW sequence
        let mask1 = Pio::read8(PIC1_DATA as usize);
        let mask2 = Pio::read8(PIC2_DATA as usize);

        // ICW1 — start initialization, declare ICW4 will follow
        Pio::write8(PIC1_COMMAND as usize, ICW1_INIT | ICW1_ICW4);
        Pio::io_wait();
        Pio::write8(PIC2_COMMAND as usize, ICW1_INIT | ICW1_ICW4);
        Pio::io_wait();

        // ICW2 — vector offsets
        Pio::write8(PIC1_DATA as usize, self.master_offset);
        Pio::io_wait();
        Pio::write8(PIC2_DATA as usize, self.slave_offset);
        Pio::io_wait();

        // ICW3 — cascade wiring
        // Master: bitmask of IR lines with a slave attached (IR2 → bit 2 = 0x04)
        Pio::write8(PIC1_DATA as usize, 0x04);
        Pio::io_wait();
        // Slave: binary cascade identity (slave is on master IR2 → value 2)
        Pio::write8(PIC2_DATA as usize, 0x02);
        Pio::io_wait();

        // ICW4 — 8086 mode for both
        Pio::write8(PIC1_DATA as usize, ICW4_8086);
        Pio::io_wait();
        Pio::write8(PIC2_DATA as usize, ICW4_8086);
        Pio::io_wait();

        // Restore saved masks
        Pio::write8(PIC1_DATA as usize, mask1);
        Pio::write8(PIC2_DATA as usize, mask2);
    }

    /// Mask all IRQ lines on both chips. Call this if you are switching to
    /// the APIC and no longer need the PIC.
    pub fn disable_all(&self) {
        Pio::write8(PIC1_DATA as usize, 0xFF);
        Pio::write8(PIC2_DATA as usize, 0xFF);
    }

    // -------------------------------------------------------------------------
    // EOI
    // -------------------------------------------------------------------------

    /// Send End-Of-Interrupt.
    ///
    /// Returns `Err(SpuriousIrq)` if the IRQ was spurious and the EOI should
    /// NOT be sent. The caller should log and discard spurious IRQs.
    ///
    /// - IRQ 7  (spurious master): no EOI at all.
    /// - IRQ 15 (spurious slave):  EOI to master only (slave never latched it).
    pub fn end_of_interrupt(&self, irq: u8) -> Result<(), PicError> {
        if irq > 15 {
            return Err(PicError::InvalidIrq(irq));
        }

        if irq == SPURIOUS_IRQ_MASTER {
            // Check ISR bit 7 on master — if clear, this is spurious
            if self.read_isr() & (1 << 7) == 0 {
                return Err(PicError::SpuriousIrq(irq));
            }
        }

        if irq == SPURIOUS_IRQ_SLAVE {
            // Check ISR bit 7 on slave — if clear, this is spurious
            if self.read_isr() & (1 << 15) == 0 {
                // Still need to EOI the master since it did see the cascade
                Pio::write8(PIC1_COMMAND as usize, EOI);
                return Err(PicError::SpuriousIrq(irq));
            }
        }

        // Real IRQ — EOI slave first if it came from there, then master
        if irq >= 8 {
            Pio::write8(PIC2_COMMAND as usize, EOI);
        }
        Pio::write8(PIC1_COMMAND as usize, EOI);

        Ok(())
    }

    // -------------------------------------------------------------------------
    // Masking
    // -------------------------------------------------------------------------

    pub fn mask_irq(&self, irq: u8) -> Result<(), PicError> {
        if irq > 15 {
            return Err(PicError::InvalidIrq(irq));
        }
        let (port, bit) = Self::irq_to_port_bit(irq);
        let current = Pio::read8(port as usize);
        Pio::write8(port as usize, current | (1 << bit));
        Ok(())
    }

    pub fn unmask_irq(&self, irq: u8) -> Result<(), PicError> {
        if irq > 15 {
            return Err(PicError::InvalidIrq(irq));
        }
        let (port, bit) = Self::irq_to_port_bit(irq);
        let current = Pio::read8(port as usize);
        Pio::write8(port as usize, current & !(1 << bit));

        Ok(())
    }

    // -------------------------------------------------------------------------
    // IRR / ISR
    // -------------------------------------------------------------------------

    /// Read the Interrupt Request Register (what is pending at the PIC inputs).
    /// Returns a 16-bit value: bits 0–7 = master IRQ 0–7, bits 8–15 = slave.
    pub fn read_irr(&self) -> u16 {
        self.read_register(OCW3_READ_IRR)
    }

    /// Read the In-Service Register (what the CPU is currently handling).
    /// Returns a 16-bit value: bits 0–7 = master IRQ 0–7, bits 8–15 = slave.
    pub fn read_isr(&self) -> u16 {
        self.read_register(OCW3_READ_ISR)
    }

    /// Send an OCW3 command to both chips and read back the selected register.
    fn read_register(&self, ocw3: u8) -> u16 {
        Pio::write8(PIC1_COMMAND as usize, ocw3);
        Pio::write8(PIC2_COMMAND as usize, ocw3);
        let lo = Pio::read8(PIC1_COMMAND as usize) as u16;
        let hi = Pio::read8(PIC2_COMMAND as usize) as u16;
        (hi << 8) | lo
    }

    // -------------------------------------------------------------------------
    // Helpers
    // -------------------------------------------------------------------------

    /// Returns `(data_port, bit_index)` for a given IRQ line.
    #[inline]
    fn irq_to_port_bit(irq: u8) -> (u16, u8) {
        if irq < 8 {
            (PIC1_DATA, irq)
        } else {
            (PIC2_DATA, irq - 8)
        }
    }

    /// Translate a CPU vector number back to a PIC IRQ line (0–15).
    /// Returns `None` if the vector is outside the remapped range.
    pub fn vector_to_irq(&self, vector: u8) -> Option<u8> {
        if vector >= self.master_offset && vector < self.master_offset + 8 {
            Some(vector - self.master_offset)
        } else if vector >= self.slave_offset && vector < self.slave_offset + 8 {
            Some(vector - self.slave_offset + 8)
        } else {
            None
        }
    }
}

// ============================================================================
// InterruptController impl
// ============================================================================

impl InterruptController for Pic8259 {
    type Error = PicError;

    fn enable(&mut self, irq: crate::hal::interrupt::IrqNumber) -> Result<(), Self::Error> {
        self.unmask_irq(irq as u8)
    }

    fn disable(&mut self, irq: crate::hal::interrupt::IrqNumber) -> Result<(), Self::Error> {
        self.mask_irq(irq as u8)
    }

    fn clear(&mut self, irq: crate::hal::interrupt::IrqNumber) -> Result<(), Self::Error> {
        self.end_of_interrupt(irq as u8)
    }

    fn is_pending(&self, irq: crate::hal::interrupt::IrqNumber) -> Result<bool, Self::Error> {
        if irq > 15 {
            return Err(PicError::InvalidIrq(irq as u8));
        }
        Ok(self.read_irr() & (1 << irq) != 0)
    }

    fn next_pending(&self) -> Option<crate::hal::interrupt::IrqNumber> {
        let irr = self.read_irr();
        (0u16..16).find(|&i| irr & (1 << i) != 0).map(|i| i as _)
    }
}
