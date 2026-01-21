use core::ptr::{read_volatile, write_volatile};

use crate::hal::block_device::{
    BlockDevice, BlockDeviceError, BlockDeviceInfo, CardType, Cid, Csd, CsdParseError, CsdVersion,
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
const REG_FORCE_IRPT: usize = 0x50;
const REG_BOOT_TIMEOUT: usize = 0x70;
const REG_DBG_SEL: usize = 0x74;
const REG_EXRDFIFO_CFG: usize = 0x80;
const REG_EXRDFIFO_EN: usize = 0x84;
const REG_TUNE_STEP: usize = 0x88;
const REG_TUNE_STEPS_STD: usize = 0x8C;
const REG_TUNE_STEPS_DDR: usize = 0x90;
const REG_SPI_INT_SPT: usize = 0xF0;
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

/// Command index shift
const CMD_INDEX_SHIFT: u32 = 24;

/// SD Commands
const CMD0: u32 = 0;
const CMD1: u32 = 1; // MMC init
const CMD2: u32 = 2;
const CMD3: u32 = 3;
const CMD6: u32 = 6;
const CMD7: u32 = 7;
const CMD8: u32 = 8;
const CMD9: u32 = 9;
const CMD12: u32 = 12;
const CMD13: u32 = 13; // Send status
const CMD16: u32 = 16;
const CMD17: u32 = 17;
const CMD18: u32 = 18;
const CMD24: u32 = 24;
const CMD25: u32 = 25;
const CMD55: u32 = 55;
const ACMD6: u32 = 6;
const ACMD41: u32 = 41;
const ACMD51: u32 = 51;

/// Block size (fixed to 512 bytes)
const BLOCK_SIZE: usize = 512;

/// BCM2835 EMMC driver
pub struct Emmc {
    base: usize,
    cid: Cid, // Card Identification
    csd: Csd, // Card Specific Data
    rca: u32, // Relative Card Address
    card_type: CardType,
}

impl Emmc {
    /// Create new EMMC driver
    pub const unsafe fn new() -> Self {
        Self {
            base: EMMC_BASE,
            cid: Cid::default(),
            csd: Csd::default(),
            rca: 0,
            card_type: CardType::Unknown,
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
        let timeout = 100_000;
        for _ in 0..timeout {
            let interrupt = self.read_reg(REG_INTERRUPT);

            if interrupt & INT_ERROR != 0 {
                // Check specific error bits
                if interrupt & INT_TIMEOUT != 0 {
                    self.write_reg(REG_INTERRUPT, INT_TIMEOUT);
                    return Err(EmmcError::Timeout);
                }
                if interrupt & INT_CRC != 0 {
                    self.write_reg(REG_INTERRUPT, INT_CRC);
                }
                if interrupt & INT_INDEX != 0 {
                    self.write_reg(REG_INTERRUPT, INT_INDEX);
                }
                self.write_reg(REG_INTERRUPT, INT_ERROR);
                return Err(EmmcError::CommandError);
            }

            if interrupt & INT_CMD_DONE != 0 {
                // Clear interrupt
                self.write_reg(REG_INTERRUPT, INT_CMD_DONE);
                return Ok(());
            }
            self.delay_us(10);
        }

        Err(EmmcError::Timeout)
    }

    /// Send a command with custom flags
    fn send_cmd(&self, cmd_index: u32, arg: u64, flags: u32) -> Result<(), EmmcError> {
        // Wait for CMD line to be ready
        let timeout = 100_000;
        for _ in 0..timeout {
            let status = self.read_reg(REG_STATUS);
            if status & STATUS_CMD_INHIBIT == 0 {
                break;
            }
            self.delay_us(1);
        }

        // Clear interrupts
        self.write_reg(REG_INTERRUPT, 0xFFFF_FFFF);

        // Set argument
        self.write_reg(REG_ARG2, (arg >> 32) as u32); // high
        self.write_reg(REG_ARG1, arg as u32); // low

        // Build command register value
        // Command index goes in bits 29-24, combine with provided flags
        let cmd_reg = (cmd_index << CMD_INDEX_SHIFT) | flags;

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

        // Enable interrupts
        self.write_reg(REG_IRPT_MASK, 0xFFFF_FFFF);

        // CMD0: GO_IDLE_STATE - Reset card
        self.send_cmd(CMD0, 0, CMD_RESPONSE_NONE)?;
        self.delay_ms(10);

        // CMD8: Check if SD v2.0+
        let cmd8_arg = 0x1AA; // 2.7-3.6V, check pattern 0xAA
        if self
            .send_cmd(
                CMD8,
                cmd8_arg,
                CMD_RESPONSE_48 | CMD_CRCCHK_EN | CMD_IXCHK_EN,
            )
            .is_ok()
        {
            let resp = self.get_response(0);
            if (resp & 0xFFF) == 0x1AA {
                // SD v2.0+ card
                self.card_type = CardType::SDv2;
                self.init_sd_v2()?;
            } else {
                // Not SD v2.0+
                self.card_type = CardType::SDv1;
                self.init_sd_v1()?;
            }
        } else {
            // CMD8 failed, try SD v1.x or MMC
            self.card_type = CardType::SDv1;
            if let Err(_e) = self.init_sd_v1() {
                // Try MMC
                self.card_type = CardType::MMC;
                self.init_mmc()?;
            }
        }

        // Get CID
        self.send_cmd(CMD2, 0, CMD_RESPONSE_136 | CMD_CRCCHK_EN)?;
        let cid_u128 = (self.get_response(0) as u128)
            | ((self.get_response(1) as u128) << 32)
            | ((self.get_response(2) as u128) << 64)
            | ((self.get_response(3) as u128) << 96);
        let cid: [u8; 16] = cid_u128.to_be_bytes();
        self.cid = Cid::parse(&cid);

        // Get RCA
        self.send_cmd(CMD3, 0, CMD_RESPONSE_48 | CMD_CRCCHK_EN | CMD_IXCHK_EN)?;
        self.rca = self.get_response(0) >> 16;

        // Get CSD
        self.send_cmd(
            CMD9,
            (self.rca << 16).into(),
            CMD_RESPONSE_136 | CMD_CRCCHK_EN,
        )?;
        let csd_128 = (self.get_response(0) as u128)
            | ((self.get_response(1) as u128) << 32)
            | ((self.get_response(2) as u128) << 64)
            | ((self.get_response(3) as u128) << 96);
        let csd: [u8; 16] = csd_128.to_be_bytes();
        self.csd = Csd::parse(&csd)?;

        // Select card
        self.send_cmd(
            CMD7,
            (self.rca << 16).into(),
            CMD_RESPONSE_48 | CMD_CRCCHK_EN | CMD_IXCHK_EN,
        )?;

        // Set block size to 512 bytes
        self.send_cmd(
            CMD16,
            BLOCK_SIZE as u64,
            CMD_RESPONSE_48 | CMD_CRCCHK_EN | CMD_IXCHK_EN,
        )?;

        // Increase clock speed to 25 MHz for normal operation
        self.set_clock(25_000_000)?;

        Ok(())
    }

    /// Initialize SD v2.0+ card
    fn init_sd_v2(&mut self) -> Result<(), EmmcError> {
        let mut retries = 1000;
        loop {
            // CMD55: Next command is application-specific
            self.send_cmd(CMD55, 0, CMD_RESPONSE_48 | CMD_CRCCHK_EN | CMD_IXCHK_EN)?;

            // ACMD41: Send operating conditions with HCS bit
            let acmd41_arg = 0x4030_0000; // HCS=1 (SDHC/SDXC), 3.3V
            self.send_cmd(ACMD41, acmd41_arg, CMD_RESPONSE_48)?; // No CRC check for ACMD41

            let resp = self.get_response(0);
            if resp & 0x8000_0000 != 0 {
                // Card is ready
                break;
            }

            retries -= 1;
            if retries == 0 {
                return Err(EmmcError::InitFailed);
            }

            self.delay_ms(10);
        }

        Ok(())
    }

    /// Initialize SD v1.x card
    fn init_sd_v1(&mut self) -> Result<(), EmmcError> {
        let mut retries = 1000;
        loop {
            // CMD55: Next command is application-specific
            self.send_cmd(CMD55, 0, CMD_RESPONSE_48 | CMD_CRCCHK_EN | CMD_IXCHK_EN)?;

            // ACMD41: Send operating conditions (no HCS bit for v1.x)
            let acmd41_arg = 0x0030_0000; // 3.3V only
            self.send_cmd(ACMD41, acmd41_arg, CMD_RESPONSE_48)?; // No CRC check for ACMD41

            let resp = self.get_response(0);
            if resp & 0x8000_0000 != 0 {
                // Card is ready
                break;
            }

            retries -= 1;
            if retries == 0 {
                return Err(EmmcError::InitFailed);
            }

            self.delay_ms(10);
        }

        Ok(())
    }

    /// Initialize MMC card
    fn init_mmc(&mut self) -> Result<(), EmmcError> {
        let mut retries = 1000;
        loop {
            // CMD1: Send operating conditions (MMC)
            self.send_cmd(CMD1, 0x80FF_8000, CMD_RESPONSE_48)?; // No CRC check for CMD1

            let resp = self.get_response(0);
            if resp & 0x8000_0000 != 0 {
                // Card is ready
                break;
            }

            retries -= 1;
            if retries == 0 {
                return Err(EmmcError::InitFailed);
            }

            self.delay_ms(10);
        }

        Ok(())
    }

    /// Read a single block
    fn read_block_internal(&self, lba: u32, buf: &mut [u8]) -> Result<(), EmmcError> {
        if buf.len() < BLOCK_SIZE {
            return Err(EmmcError::BufferTooSmall);
        }

        // Wait for DAT line to be ready
        let timeout = 100_000;
        for _ in 0..timeout {
            let status = self.read_reg(REG_STATUS);
            if status & STATUS_DAT_INHIBIT == 0 {
                break;
            }
            self.delay_us(10);
        }

        // Set block size and count
        self.write_reg(REG_BLKSIZECNT, (1 << 16) | BLOCK_SIZE as u32);

        // Clear interrupts
        self.write_reg(REG_INTERRUPT, 0xFFFF_FFFF);

        // Calculate address
        let address = match self.csd.version {
            CsdVersion::V1_0 => (lba as u64) * (BLOCK_SIZE as u64),
            CsdVersion::V2_0 | CsdVersion::V3_0 => lba as u64,
        };

        // Build command flags for read operation
        let flags = CMD_RESPONSE_48 | CMD_CRCCHK_EN | CMD_IXCHK_EN | CMD_ISDATA | TM_DAT_DIR_READ;

        // Send CMD17 with read flags
        self.send_cmd(CMD17, address, flags)?;

        // Wait for data ready
        self.wait_data_ready()?;

        // Read data
        for chunk in buf[..BLOCK_SIZE].chunks_mut(4) {
            let word = self.read_reg(REG_DATA);
            chunk.copy_from_slice(&word.to_le_bytes()[..chunk.len()]);
        }

        // Wait for data done
        self.wait_data_done()?;

        Ok(())
    }

    /// Write a single block
    fn write_block_internal(&self, lba: u32, buf: &[u8]) -> Result<(), EmmcError> {
        if buf.len() < BLOCK_SIZE {
            return Err(EmmcError::BufferTooSmall);
        }

        // Wait for DAT line to be ready
        let timeout = 100_000;
        for _ in 0..timeout {
            let status = self.read_reg(REG_STATUS);
            if status & STATUS_DAT_INHIBIT == 0 {
                break;
            }
            self.delay_us(10);
        }

        // Set block size and count
        self.write_reg(REG_BLKSIZECNT, (1 << 16) | BLOCK_SIZE as u32);

        // Clear interrupts
        self.write_reg(REG_INTERRUPT, 0xFFFF_FFFF);

        // Calculate address
        let address = match self.csd.version {
            CsdVersion::V1_0 => (lba as u64) * (BLOCK_SIZE as u64),
            CsdVersion::V2_0 | CsdVersion::V3_0 => lba as u64,
        };

        // Build command flags for write operation (no TM_DAT_DIR_READ = write direction)
        let flags = CMD_RESPONSE_48 | CMD_CRCCHK_EN | CMD_IXCHK_EN | CMD_ISDATA;

        // Send CMD24 with write flags
        self.send_cmd(CMD24, address, flags)?;

        // Wait for buffer write ready
        self.wait_write_ready()?;

        // Write data
        for chunk in buf[..BLOCK_SIZE].chunks(4) {
            let mut word = [0u8; 4];
            let len = chunk.len().min(4);
            word[..len].copy_from_slice(&chunk[..len]);
            self.write_reg(REG_DATA, u32::from_le_bytes(word));
        }

        // Wait for data done
        self.wait_data_done()?;

        Ok(())
    }

    // ============================================================================
    // Helper methods
    // ============================================================================

    fn reset(&mut self) -> Result<(), EmmcError> {
        // Set reset bit in CONTROL1
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

        self.delay_us(10);

        // Calculate divisor: SD_CLK = BASE_CLK / (2 × divisor)
        let mut divisor = BASE_CLOCK / (2 * freq);
        if BASE_CLOCK % (2 * freq) != 0 {
            divisor += 1;
        }
        divisor = divisor.max(1).min(1023);

        // Encode divisor properly for BCM2835
        let divisor_ms = ((divisor >> 2) & 0xFF) << 8; // Bits 15-8
        let divisor_ls = (divisor & 0x3) << 6; // Bits 7-6

        // Read current control register and modify clock bits
        ctrl1 = self.read_reg(REG_CONTROL1);

        // Clear old clock divisor bits (bits 15-8 and 7-6)
        ctrl1 &= !(0xFF << 8); // Clear bits 15-8
        ctrl1 &= !(0x3 << 6); // Clear bits 7-6

        // Set new divisor and enable internal clock with programmable mode
        ctrl1 |= divisor_ms | divisor_ls | CLK_GENSEL | CLK_INTLEN;
        self.write_reg(REG_CONTROL1, ctrl1);

        self.delay_us(10);

        // Wait for clock to stabilize
        for _ in 0..10_000 {
            ctrl1 = self.read_reg(REG_CONTROL1);
            if ctrl1 & CLK_STABLE != 0 {
                break;
            }
            self.delay_us(10);
        }

        if ctrl1 & CLK_STABLE == 0 {
            return Err(EmmcError::Timeout);
        }

        self.delay_us(10);

        // Enable SD clock output
        ctrl1 |= CLK_EN;
        self.write_reg(REG_CONTROL1, ctrl1);

        self.delay_us(10);

        Ok(())
    }

    fn delay_us(&self, us: u32) {
        // Simple busy wait - should be replaced with proper timer
        for _ in 0..us {
            core::hint::spin_loop();
        }
    }

    fn delay_ms(&self, ms: u32) {
        self.delay_us(ms * 1000);
    }

    fn wait_data_ready(&self) -> Result<(), EmmcError> {
        let timeout = 100_000;
        for _ in 0..timeout {
            let interrupt = self.read_reg(REG_INTERRUPT);

            if interrupt & INT_ERROR != 0 {
                if interrupt & INT_DATA_TIMEOUT != 0 {
                    self.write_reg(REG_INTERRUPT, INT_DATA_TIMEOUT);
                    return Err(EmmcError::Timeout);
                }
                if interrupt & INT_DATA_CRC != 0 {
                    self.write_reg(REG_INTERRUPT, INT_DATA_CRC);
                    return Err(EmmcError::ReadError);
                }
                self.write_reg(REG_INTERRUPT, INT_ERROR);
                return Err(EmmcError::ReadError);
            }

            if interrupt & INT_READ_READY != 0 {
                // Clear interrupt
                self.write_reg(REG_INTERRUPT, INT_READ_READY);
                return Ok(());
            }

            self.delay_us(10);
        }

        Err(EmmcError::Timeout)
    }

    fn wait_write_ready(&self) -> Result<(), EmmcError> {
        let timeout = 100_000;
        for _ in 0..timeout {
            let interrupt = self.read_reg(REG_INTERRUPT);

            if interrupt & INT_ERROR != 0 {
                self.write_reg(REG_INTERRUPT, INT_ERROR);
                return Err(EmmcError::WriteError);
            }

            if interrupt & INT_WRITE_READY != 0 {
                // Clear interrupt
                self.write_reg(REG_INTERRUPT, INT_WRITE_READY);
                return Ok(());
            }

            self.delay_us(10);
        }

        Err(EmmcError::Timeout)
    }

    fn wait_data_done(&self) -> Result<(), EmmcError> {
        let timeout = 100_000;
        for _ in 0..timeout {
            let interrupt = self.read_reg(REG_INTERRUPT);

            if interrupt & INT_ERROR != 0 {
                self.write_reg(REG_INTERRUPT, INT_ERROR);
                return Err(EmmcError::WriteError);
            }

            if interrupt & INT_DATA_DONE != 0 {
                // Clear interrupt
                self.write_reg(REG_INTERRUPT, INT_DATA_DONE);
                return Ok(());
            }

            self.delay_us(10);
        }

        Err(EmmcError::Timeout)
    }
}

impl BlockDevice for Emmc {
    fn info(&self) -> BlockDeviceInfo {
        // Get block count from CSD and mark as removable (SD cards are removable)
        BlockDeviceInfo::new(self.csd.block_count()).removable() // SD cards are removable
    }

    fn read_block(&self, block: u64, buffer: &mut [u8]) -> Result<(), BlockDeviceError> {
        // Validate buffer size
        if buffer.len() < BLOCK_SIZE {
            return Err(BlockDeviceError::InvalidBuffer);
        }

        // Validate block address
        let block_count = self.csd.block_count();
        if block >= block_count {
            return Err(BlockDeviceError::InvalidAddress);
        }

        // Check if device is ready
        if !self.is_ready() {
            return Err(BlockDeviceError::NotReady);
        }

        self.read_block_internal(block as u32, buffer)
            .map_err(|e| e.into())
    }

    fn write_block(&self, block: u64, buffer: &[u8]) -> Result<(), BlockDeviceError> {
        // Validate buffer size
        if buffer.len() < BLOCK_SIZE {
            return Err(BlockDeviceError::InvalidBuffer);
        }

        // Validate block address
        let block_count = self.csd.block_count();
        if block >= block_count {
            return Err(BlockDeviceError::InvalidAddress);
        }

        // Check if device is ready
        if !self.is_ready() {
            return Err(BlockDeviceError::NotReady);
        }

        self.write_block_internal(block as u32, buffer)
            .map_err(|e| e.into())
    }

    fn flush(&mut self) -> Result<(), BlockDeviceError> {
        // For SD cards, writes are typically immediate, but we could send CMD13 to check status
        Ok(())
    }

    fn is_ready(&self) -> bool {
        let status = self.read_reg(REG_STATUS);
        (status & STATUS_CARD_INSERTED) != 0 && (status & STATUS_CARD_STATE_STABLE) != 0
    }

    fn read_blocks(
        &self,
        start_block: u64,
        buffers: &mut [&mut [u8]],
    ) -> Result<(), BlockDeviceError> {
        // Validate all buffers
        for buffer in buffers.iter() {
            if buffer.len() < BLOCK_SIZE {
                return Err(BlockDeviceError::InvalidBuffer);
            }
        }

        // Validate block range
        let block_count = self.csd.block_count();
        if start_block + buffers.len() as u64 > block_count {
            return Err(BlockDeviceError::InvalidAddress);
        }

        // Check if device is ready
        if !self.is_ready() {
            return Err(BlockDeviceError::NotReady);
        }

        // Read each block
        for (i, buf_slice) in buffers.iter_mut().enumerate() {
            self.read_block_internal((start_block + i as u64) as u32, buf_slice)?;
        }

        Ok(())
    }

    fn write_blocks(&self, start_block: u64, buffers: &[&[u8]]) -> Result<(), BlockDeviceError> {
        // Validate all buffers
        for buffer in buffers.iter() {
            if buffer.len() < BLOCK_SIZE {
                return Err(BlockDeviceError::InvalidBuffer);
            }
        }

        // Validate block range
        let block_count = self.csd.block_count();
        if start_block + buffers.len() as u64 > block_count {
            return Err(BlockDeviceError::InvalidAddress);
        }

        // Check if device is ready
        if !self.is_ready() {
            return Err(BlockDeviceError::NotReady);
        }

        // Write each block
        for (i, buf_slice) in buffers.iter().enumerate() {
            self.write_block_internal((start_block + i as u64) as u32, buf_slice)?;
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

// Ensure Emmc is Send + Sync for thread safety
// This is safe because we're accessing memory-mapped registers which
// have synchronized access through the hardware
unsafe impl Send for Emmc {}
unsafe impl Sync for Emmc {}

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
            EmmcError::InitFailed => BlockDeviceError::Other,
            EmmcError::Timeout => BlockDeviceError::Timeout,
            EmmcError::BufferTooSmall => BlockDeviceError::InvalidBuffer,
            EmmcError::ReadError => BlockDeviceError::ReadError,
            EmmcError::WriteError => BlockDeviceError::WriteError,
            EmmcError::CommandError => BlockDeviceError::Other,
        }
    }
}

impl From<CsdParseError> for EmmcError {
    fn from(_err: CsdParseError) -> Self {
        EmmcError::UnsupportedCard
    }
}
