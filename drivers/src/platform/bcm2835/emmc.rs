use core::ptr::{read_volatile, write_volatile};

use crate::hal::block_device::{
    BlockDevice, BlockDeviceError, BlockDeviceInfo, Cid, Csd, CsdParseError,
    IdentifiableBlockDevice,
};

/// EMMC base address
const EMMC_BASE: usize = 0x2030_0000;

/// Register offsets
const REG_ARG2: usize = 0x00;
const REG_BLKSIZECNT: usize = 0x04;
const REG_ARG1: usize = 0x08;
const REG_CMDTM: usize = 0x0C;
const REG_RESP0: usize = 0x10;
const REG_RESP1: usize = 0x14;
const REG_RESP2: usize = 0x18;
const REG_RESP3: usize = 0x1C;
const REG_DATA: usize = 0x20;
const REG_STATUS: usize = 0x24;
const REG_CONTROL0: usize = 0x28;
const REG_CONTROL1: usize = 0x2C;
const REG_INTERRUPT: usize = 0x30;
const REG_IRPT_MASK: usize = 0x34;
const REG_IRPT_EN: usize = 0x38;
const REG_CONTROL2: usize = 0x3C;
const REG_SLOTISR_VER: usize = 0xFC;

/// Status register bits
const STATUS_CMD_INHIBIT: u32 = 1 << 0;
const STATUS_DAT_INHIBIT: u32 = 1 << 1;
const STATUS_DAT_ACTIVE: u32 = 1 << 2;
const STATUS_WRITE_TRANSFER: u32 = 1 << 8;
const STATUS_READ_TRANSFER: u32 = 1 << 9;
const STATUS_BUFFER_WRITE_READY: u32 = 1 << 10;
const STATUS_BUFFER_READ_READY: u32 = 1 << 11;
const STATUS_CARD_INSERTED: u32 = 1 << 16;
const STATUS_CARD_STATE_STABLE: u32 = 1 << 17;
const STATUS_DAT_LEVEL0: u32 = 1 << 20;

/// Interrupt register bits
const INT_CMD_DONE: u32 = 1 << 0;
const INT_DATA_DONE: u32 = 1 << 1;
const INT_BLOCK_GAP: u32 = 1 << 2;
const INT_WRITE_READY: u32 = 1 << 4;
const INT_READ_READY: u32 = 1 << 5;
const INT_ERROR: u32 = 1 << 15;
const INT_TIMEOUT: u32 = 1 << 16;
const INT_CRC: u32 = 1 << 17;
const INT_END_BIT: u32 = 1 << 18;
const INT_INDEX: u32 = 1 << 19;
const INT_DATA_TIMEOUT: u32 = 1 << 20;
const INT_DATA_CRC: u32 = 1 << 21;
const INT_DATA_END_BIT: u32 = 1 << 22;
const INT_ACMD_ERR: u32 = 1 << 24;

/// Command register bits
const CMD_RESPONSE_NONE: u32 = 0 << 16;
const CMD_RESPONSE_136: u32 = 1 << 16;
const CMD_RESPONSE_48: u32 = 2 << 16;
const CMD_RESPONSE_48_BUSY: u32 = 3 << 16;
const CMD_CRCCHK_EN: u32 = 1 << 19;
const CMD_IXCHK_EN: u32 = 1 << 20;
const CMD_ISDATA: u32 = 1 << 21;
const CMD_TYPE_NORMAL: u32 = 0 << 22;
const CMD_TYPE_SUSPEND: u32 = 1 << 22;
const CMD_TYPE_RESUME: u32 = 2 << 22;
const CMD_TYPE_ABORT: u32 = 3 << 22;

/// Control1 register bits
const CLK_INTLEN: u32 = 1 << 0; // Internal clock enable
const CLK_STABLE: u32 = 1 << 1; // Clock stable (read-only)
const CLK_EN: u32 = 1 << 2; // SD clock enable
const CLK_GENSEL: u32 = 1 << 5; // Programmable mode
const SRST_HC: u32 = 1 << 24;
const SRST_CMD: u32 = 1 << 25;
const SRST_DATA: u32 = 1 << 26;

/// Transfer mode bits
const TM_MULTI_BLOCK: u32 = 1 << 5;
const TM_DAT_DIR_READ: u32 = 1 << 4;
const TM_AUTO_CMD_EN_NONE: u32 = 0 << 2;
const TM_AUTO_CMD_EN_CMD12: u32 = 1 << 2;
const TM_AUTO_CMD_EN_CMD23: u32 = 2 << 2;
const TM_BLKCNT_EN: u32 = 1 << 1;
const TM_DMA_EN: u32 = 1 << 0;

/// SD Commands
const CMD0: u32 = 0;
const CMD2: u32 = 2;
const CMD3: u32 = 3;
const CMD7: u32 = 7;
const CMD8: u32 = 8;
const CMD9: u32 = 9;
const CMD12: u32 = 12;
const CMD16: u32 = 16;
const CMD17: u32 = 17;
const CMD18: u32 = 18;
const CMD24: u32 = 24;
const CMD25: u32 = 25;
const CMD55: u32 = 55;
const ACMD6: u32 = 6;
const ACMD41: u32 = 41;
const ACMD51: u32 = 51;

/// Block size
const BLOCK_SIZE: usize = 512;

/// BCM2835 EMMC driver
pub struct Emmc {
    base: usize,
    cid: Cid, // Card Identification
    csd: Csd, // Card Specific Data
    rca: u32, // Relative Card Address
}

impl Emmc {
    /// Create new EMMC driver
    pub const unsafe fn new() -> Self {
        Self {
            base: EMMC_BASE,
            cid: Cid::default(),
            csd: Csd::default(),
            rca: 0,
        }
    }

    /// Read a 32-bit register
    #[inline]
    fn read_reg(&self, offset: usize) -> u32 {
        unsafe { read_volatile((self.base + offset) as *const u32) }
    }

    /// Write a 32-bit register
    #[inline]
    fn write_reg(&self, offset: usize, value: u32) {
        unsafe { write_volatile((self.base + offset) as *mut u32, value) }
    }

    /// Wait for command to complete
    fn wait_cmd_done(&self) -> Result<(), EmmcError> {
        let timeout = 1_000_000;
        for _ in 0..timeout {
            let interrupt = self.read_reg(REG_INTERRUPT);

            if interrupt & INT_ERROR != 0 {
                return Err(EmmcError::CommandError);
            }

            if interrupt & INT_TIMEOUT != 0 {
                return Err(EmmcError::Timeout);
            }

            if interrupt & INT_CMD_DONE != 0 {
                // Clear interrupt
                self.write_reg(REG_INTERRUPT, INT_CMD_DONE);
                return Ok(());
            }
        }

        Err(EmmcError::Timeout)
    }

    /// Send a command
    fn send_cmd(&self, cmd: u32, arg: u32) -> Result<(), EmmcError> {
        // Wait for CMD line to be ready
        let timeout = 1_000_000;
        for _ in 0..timeout {
            let status = self.read_reg(REG_STATUS);
            if status & STATUS_CMD_INHIBIT == 0 {
                break;
            }
        }

        // Clear interrupts
        self.write_reg(REG_INTERRUPT, 0xFFFF_FFFF);

        // Set argument
        self.write_reg(REG_ARG1, arg);

        // Determine response type
        let cmd_reg = match cmd {
            CMD0 => cmd | CMD_RESPONSE_NONE,
            CMD2 | CMD9 => cmd | CMD_RESPONSE_136 | CMD_CRCCHK_EN,
            CMD3 | CMD7 | CMD8 | CMD16 | CMD17 | CMD18 | CMD24 | CMD25 => {
                cmd | CMD_RESPONSE_48 | CMD_CRCCHK_EN | CMD_IXCHK_EN
            }
            CMD55 => cmd | CMD_RESPONSE_48 | CMD_CRCCHK_EN,
            _ => cmd | CMD_RESPONSE_48,
        };

        // Send command
        self.write_reg(REG_CMDTM, cmd_reg);

        // Wait for completion
        self.wait_cmd_done()
    }

    /// Get response
    fn get_response(&self, index: usize) -> u32 {
        match index {
            0 => self.read_reg(REG_RESP0),
            1 => self.read_reg(REG_RESP1),
            2 => self.read_reg(REG_RESP2),
            3 => self.read_reg(REG_RESP3),
            _ => 0,
        }
    }

    /// Initialize the SD card
    pub fn init(&mut self) -> Result<(), EmmcError> {
        // Check if card is inserted
        let status = self.read_reg(REG_STATUS);
        if status & STATUS_CARD_INSERTED == 0 {
            return Err(EmmcError::NoCard);
        }

        // Reset controller
        self.reset()?;

        // Set clock to 400 kHz for initialization
        self.set_clock(400_000)?;

        // Wait for card to stabilize
        self.delay_ms(10);

        // CMD0: Reset card
        self.send_cmd(CMD0, 0)?;
        self.delay_ms(2);

        // CMD8: Check voltage (SD v2.0+)
        let cmd8_arg = 0x1AA; // 2.7-3.6V, check pattern 0xAA
        self.send_cmd(CMD8, cmd8_arg)?;
        let resp = self.get_response(0);

        if resp != cmd8_arg {
            return Err(EmmcError::UnsupportedCard);
        }

        // ACMD41: Initialize card (loop until ready)
        let mut retries = 1000;
        loop {
            // CMD55: Next command is application-specific
            self.send_cmd(CMD55, 0)?;

            // ACMD41: Send operating conditions
            let acmd41_arg = 0x5030_0000; // SDHC, 3.3V
            self.send_cmd(ACMD41, acmd41_arg)?;

            let resp = self.get_response(0);
            if resp & 0x8000_0000 != 0 {
                // Card is ready!
                break;
            }

            retries -= 1;
            if retries == 0 {
                return Err(EmmcError::InitFailed);
            }

            self.delay_ms(10);
        }

        // CMD2: Get CID (Card Identification)
        self.send_cmd(CMD2, 0)?;
        let cid_u128 = (self.get_response(0) as u128)
            | ((self.get_response(1) as u128) << 32)
            | ((self.get_response(2) as u128) << 64)
            | ((self.get_response(3) as u128) << 96);
        let cid: [u8; 16] = cid_u128.to_be_bytes();

        self.cid = Cid::parse(&cid);

        // CMD3: Get RCA (Relative Card Address)
        self.send_cmd(CMD3, 0)?;
        self.rca = self.get_response(0) & 0xFFFF_0000;

        // CMD9: Get CSD (Card Specific Data)
        self.send_cmd(CMD9, self.rca)?;
        let csd_128 = (self.get_response(0) as u128)
            | ((self.get_response(1) as u128) << 32)
            | ((self.get_response(2) as u128) << 64)
            | ((self.get_response(3) as u128) << 96);
        let csd: [u8; 16] = csd_128.to_be_bytes();
        self.csd = Csd::parse(&csd)?;

        // CMD7: Select card
        self.send_cmd(CMD7, self.rca)?;

        // Set block size to 512 bytes
        self.send_cmd(CMD16, BLOCK_SIZE as u32)?;

        // Increase clock speed to 25 MHz
        self.set_clock(25_000_000)?;

        Ok(())
    }

    /// Read a single block
    pub fn read_block_emmc(&self, lba: u32, buf: &mut [u8]) -> Result<(), EmmcError> {
        if buf.len() < BLOCK_SIZE {
            return Err(EmmcError::BufferTooSmall);
        }

        // Set block size and count
        self.write_reg(REG_BLKSIZECNT, (1 << 16) | BLOCK_SIZE as u32);

        // CMD17: Read single block
        let cmd = CMD17 | CMD_RESPONSE_48 | CMD_CRCCHK_EN | CMD_IXCHK_EN | CMD_ISDATA;
        let tm = TM_DAT_DIR_READ;
        self.write_reg(REG_CMDTM, cmd | (tm << 16));
        self.write_reg(REG_ARG1, lba);

        // Wait for data
        self.wait_data_ready()?;

        // Read data
        for chunk in buf[..BLOCK_SIZE].chunks_mut(4) {
            let word = self.read_reg(REG_DATA);
            chunk.copy_from_slice(&word.to_le_bytes()[..chunk.len()]);
        }

        Ok(())
    }

    /// Write a single block
    pub fn write_block_emmc(&self, lba: u32, buf: &[u8]) -> Result<(), EmmcError> {
        if buf.len() < BLOCK_SIZE {
            return Err(EmmcError::BufferTooSmall);
        }

        // Set block size and count
        self.write_reg(REG_BLKSIZECNT, (1 << 16) | BLOCK_SIZE as u32);

        // CMD24: Write single block
        let cmd = CMD24 | CMD_RESPONSE_48 | CMD_CRCCHK_EN | CMD_IXCHK_EN | CMD_ISDATA;
        let tm = 0; // Write direction
        self.write_reg(REG_CMDTM, cmd | (tm << 16));
        self.write_reg(REG_ARG1, lba);

        // Wait for buffer ready
        self.wait_write_ready()?;

        // Write data
        for chunk in buf[..BLOCK_SIZE].chunks(4) {
            let mut word = [0u8; 4];
            word[..chunk.len()].copy_from_slice(chunk);
            self.write_reg(REG_DATA, u32::from_le_bytes(word));
        }

        // Wait for completion
        self.wait_data_done()?;

        Ok(())
    }

    // ============================================================================
    // Helper methods
    // ============================================================================

    fn reset(&mut self) -> Result<(), EmmcError> {
        // Set reset bit in CONTROL1 (not CONTROL0!)
        let mut ctrl1 = self.read_reg(REG_CONTROL1);
        ctrl1 |= SRST_HC;
        self.write_reg(REG_CONTROL1, ctrl1);

        // Wait for hardware to clear bit (with timeout)
        for _ in 0..10_000 {
            ctrl1 = self.read_reg(REG_CONTROL1);
            if ctrl1 & SRST_HC == 0 {
                self.delay_us(100);
                return Ok(());
            }
            self.delay_us(10);
        }

        // Timeout if reset doesn't complete
        Err(EmmcError::Timeout)
    }

    fn set_clock(&self, freq: u32) -> Result<(), EmmcError> {
        const BASE_CLOCK: u32 = 250_000_000;

        // Disable SD clock
        let mut ctrl1 = self.read_reg(REG_CONTROL1);
        ctrl1 &= !CLK_EN;
        self.write_reg(REG_CONTROL1, ctrl1);

        // Calculate divisor: SD_CLK = BASE_CLK / (2 × divisor)
        let mut divisor = BASE_CLOCK / (2 * freq);
        if BASE_CLOCK % (2 * freq) != 0 {
            divisor += 1;
        }
        divisor = divisor.max(1).min(1023);

        // Encode divisor properly
        let divisor_ms = ((divisor >> 2) & 0xFF) << 8; // Bits 15-8
        let divisor_ls = (divisor & 0x3) << 6; // Bits 7-6

        // Enable internal clock with programmable mode
        ctrl1 = divisor_ms | divisor_ls | CLK_GENSEL | CLK_INTLEN;
        self.write_reg(REG_CONTROL1, ctrl1);

        // Wait for clock to stabilize
        for _ in 0..10_000 {
            ctrl1 = self.read_reg(REG_CONTROL1);
            if ctrl1 & CLK_STABLE != 0 {
                break;
            }
            self.delay_us(1);
        }

        if ctrl1 & CLK_STABLE == 0 {
            return Err(EmmcError::Timeout);
        }

        // Enable SD clock output
        ctrl1 |= CLK_EN;
        self.write_reg(REG_CONTROL1, ctrl1);

        Ok(())
    }

    fn delay_us(&self, us: u32) {
        // Use timer for delay
        for _ in 0..us {
            core::hint::spin_loop();
        }
    }

    fn delay_ms(&self, ms: u32) {
        // Use timer for delay
        for _ in 0..(ms * 1000) {
            core::hint::spin_loop();
        }
    }

    fn wait_data_ready(&self) -> Result<(), EmmcError> {
        loop {
            let interrupt = self.read_reg(REG_INTERRUPT);
            if interrupt & INT_ERROR != 0 {
                return Err(EmmcError::ReadError);
            }

            if interrupt & INT_TIMEOUT != 0 {
                return Err(EmmcError::Timeout);
            }

            if interrupt & INT_READ_READY != 0 {
                // Clear interrupt
                self.write_reg(REG_INTERRUPT, INT_READ_READY);
                return Ok(());
            }

            self.delay_us(10);
        }
    }

    fn wait_write_ready(&self) -> Result<(), EmmcError> {
        loop {
            let interrupt = self.read_reg(REG_INTERRUPT);
            if interrupt & INT_ERROR != 0 {
                return Err(EmmcError::WriteError);
            }

            if interrupt & INT_TIMEOUT != 0 {
                return Err(EmmcError::Timeout);
            }

            if interrupt & INT_WRITE_READY != 0 {
                // Clear interrupt
                self.write_reg(REG_INTERRUPT, INT_WRITE_READY);
                return Ok(());
            }

            self.delay_us(10);
        }
    }

    fn wait_data_done(&self) -> Result<(), EmmcError> {
        loop {
            let interrupt = self.read_reg(REG_INTERRUPT);
            if interrupt & INT_ERROR != 0 {
                return Err(EmmcError::ReadError);
            }

            if interrupt & INT_TIMEOUT != 0 {
                return Err(EmmcError::Timeout);
            }

            if interrupt & INT_DATA_DONE != 0 {
                // Clear interrupt
                self.write_reg(REG_INTERRUPT, INT_DATA_DONE);
                return Ok(());
            }
        }
    }
}

impl BlockDevice for Emmc {
    fn info(&self) -> BlockDeviceInfo {
        BlockDeviceInfo::new(self.csd.capacity / 512)
    }

    fn read_block(&self, block: u64, buffer: &mut [u8]) -> Result<(), BlockDeviceError> {
        self.read_block_emmc(block as u32, buffer)
            .map_err(|e| e.into())
    }

    fn write_block(&mut self, block: u64, buffer: &[u8]) -> Result<(), BlockDeviceError> {
        self.write_block_emmc(block as u32, buffer)
            .map_err(|e| e.into())
    }

    fn flush(&mut self) -> Result<(), BlockDeviceError> {
        Ok(())
    }

    fn is_ready(&self) -> bool {
        true
    }

    fn read_blocks(
        &self,
        start_block: u64,
        buffer: &mut [&mut [u8]],
    ) -> Result<(), BlockDeviceError> {
        for (i, buf_slice) in buffer.iter_mut().enumerate() {
            self.read_block_emmc((start_block + i as u64) as u32, buf_slice)?;
        }

        Ok(())
    }

    fn write_blocks(&mut self, start_block: u64, buffer: &[&[u8]]) -> Result<(), BlockDeviceError> {
        for (i, buf_slice) in buffer.iter().enumerate() {
            self.write_block_emmc((start_block + i as u64) as u32, buf_slice)?;
        }

        Ok(())
    }
}

impl IdentifiableBlockDevice for Emmc {
    fn cid(&self) -> Option<&Cid> {
        Some(&self.cid)
    }

    fn csd(&self) -> Option<&Csd> {
        Some(&self.csd)
    }
}

#[derive(Debug)]
pub enum EmmcError {
    NoCard,
    UnsupportedCard,
    InitFailed,
    CommandError,
    Timeout,
    BufferTooSmall,
    ReadError,
    WriteError,
}

impl From<EmmcError> for BlockDeviceError {
    fn from(err: EmmcError) -> Self {
        match err {
            EmmcError::NoCard => BlockDeviceError::DeviceRemoved,
            EmmcError::UnsupportedCard => BlockDeviceError::UnsupportedDevice,
            EmmcError::Timeout => BlockDeviceError::Timeout,
            EmmcError::BufferTooSmall => BlockDeviceError::DataError,
            EmmcError::ReadError => BlockDeviceError::ReadError,
            EmmcError::WriteError => BlockDeviceError::WriteError,
            _ => BlockDeviceError::Other,
        }
    }
}

impl From<CsdParseError> for EmmcError {
    fn from(_err: CsdParseError) -> Self {
        EmmcError::UnsupportedCard
    }
}
