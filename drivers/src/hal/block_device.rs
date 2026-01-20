//! Block Device Hardware Abstraction Layer
//!
//! This module provides generic traits for block-oriented storage devices
//! like SD cards, eMMC, hard drives, SSDs, etc.
//!
//! # Architecture
//!
//! ```
//! File System Layer (FAT32, ext4, etc.)
//!           ↓
//! Block Device HAL ← You are here
//!           ↓
//! Platform Drivers (EMMC, SPI-SD, etc.)
//! ```

/// Block device information
#[derive(Debug, Clone, Copy)]
pub struct BlockDeviceInfo {
    /// Block size in bytes (typically 512)
    pub block_size: usize,
    /// Total number of blocks
    pub block_count: u64,
    /// Total capacity in bytes
    pub capacity: u64,
    /// Device is read-only
    pub read_only: bool,
    /// Device is removable (e.g., SD card)
    pub removable: bool,
}

impl BlockDeviceInfo {
    /// Create info for a standard 512-byte block device
    pub fn new(block_count: u64) -> Self {
        Self {
            block_size: 512,
            block_count,
            capacity: block_count * 512,
            read_only: false,
            removable: false,
        }
    }

    /// Create info with custom block size
    pub fn with_block_size(block_size: usize, block_count: u64) -> Self {
        Self {
            block_size,
            block_count,
            capacity: (block_count as usize * block_size) as u64,
            read_only: false,
            removable: false,
        }
    }

    /// Mark device as read-only
    pub fn read_only(mut self) -> Self {
        self.read_only = true;
        self
    }

    /// Mark device as removable
    pub fn removable(mut self) -> Self {
        self.removable = true;
        self
    }
}

/// Block device errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockDeviceError {
    /// Device not initialized or not present
    NotReady,
    /// Invalid block address (out of range)
    InvalidAddress,
    /// Hardware error during read
    ReadError,
    /// Hardware error during write
    WriteError,
    /// Device is write-protected
    WriteProtected,
    /// Buffer size doesn't match block size
    InvalidBuffer,
    /// Operation timed out
    Timeout,
    /// CRC or checksum error
    DataError,
    /// Device was removed
    DeviceRemoved,
    /// Generic I/O error
    IoError,
    /// Unsupported device
    UnsupportedDevice,
    /// Other
    Other,
}

/// Block device trait - fundamental storage abstraction
///
/// This trait provides block-level access to storage devices.
/// Implementations must handle block-aligned reads and writes.
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` to allow use across threads.
///
/// # Example
///
/// ```rust
/// use drivers::hal::block_device::{BlockDevice, BlockDeviceInfo};
///
/// fn read_first_block<B: BlockDevice>(device: &B) -> Result<[u8; 512], B::Error> {
///     let mut buf = [0u8; 512];
///     device.read_blocks(0, &mut [&mut buf])?;
///     Ok(buf)
/// }
/// ```
pub trait BlockDevice: Send + Sync {
    /// Get device information
    fn info(&self) -> BlockDeviceInfo;

    /// Read one or more contiguous blocks
    ///
    /// # Arguments
    /// - `start_block`: Starting block address (LBA)
    /// - `buffers`: Slice of buffers to read into (each must be block_size bytes)
    ///
    /// # Returns
    /// - `Ok(())`: All blocks read successfully
    /// - `Err(e)`: Error during read
    ///
    /// # Errors
    /// - `InvalidAddress`: Block address out of range
    /// - `InvalidBuffer`: Buffer size incorrect
    /// - `ReadError`: Hardware failure
    fn read_blocks(
        &self,
        start_block: u64,
        buffers: &mut [&mut [u8]],
    ) -> Result<(), BlockDeviceError>;

    /// Write one or more contiguous blocks
    ///
    /// # Arguments
    /// - `start_block`: Starting block address (LBA)
    /// - `buffers`: Slice of buffers to write from (each must be block_size bytes)
    ///
    /// # Returns
    /// - `Ok(())`: All blocks written successfully
    /// - `Err(e)`: Error during write
    ///
    /// # Errors
    /// - `InvalidAddress`: Block address out of range
    /// - `InvalidBuffer`: Buffer size incorrect
    /// - `WriteProtected`: Device is read-only
    /// - `WriteError`: Hardware failure
    fn write_blocks(&mut self, start_block: u64, buffers: &[&[u8]])
    -> Result<(), BlockDeviceError>;

    /// Read a single block
    ///
    /// Convenience method for reading one block.
    ///
    /// # Arguments
    /// - `block`: Block address (LBA)
    /// - `buffer`: Buffer to read into (must be block_size bytes)
    fn read_block(&self, block: u64, buffer: &mut [u8]) -> Result<(), BlockDeviceError> {
        self.read_blocks(block, &mut [buffer])
    }

    /// Write a single block
    ///
    /// Convenience method for writing one block.
    ///
    /// # Arguments
    /// - `block`: Block address (LBA)
    /// - `buffer`: Buffer to write from (must be block_size bytes)
    fn write_block(&mut self, block: u64, buffer: &[u8]) -> Result<(), BlockDeviceError> {
        self.write_blocks(block, &[buffer])
    }

    /// Flush any pending writes
    ///
    /// Ensures all writes are committed to persistent storage.
    /// Default implementation does nothing (assumes immediate persistence).
    fn flush(&mut self) -> Result<(), BlockDeviceError> {
        Ok(())
    }

    /// Check if device is ready
    ///
    /// Returns true if device is initialized and ready for I/O.
    fn is_ready(&self) -> bool {
        true
    }
}

/// Extended block device operations
///
/// Optional trait for devices that support advanced features.
pub trait BlockDeviceExt: BlockDevice {
    /// Erase blocks
    ///
    /// Some devices (flash) can erase blocks more efficiently than writing zeros.
    ///
    /// # Arguments
    /// - `start_block`: Starting block address
    /// - `count`: Number of blocks to erase
    fn erase_blocks(&mut self, start_block: u64, count: u64) -> Result<(), BlockDeviceError>;

    /// Trim/discard blocks
    ///
    /// Notify device that blocks are no longer in use (SSD TRIM).
    ///
    /// # Arguments
    /// - `start_block`: Starting block address
    /// - `count`: Number of blocks to trim
    fn trim_blocks(&mut self, start_block: u64, count: u64) -> Result<(), BlockDeviceError>;

    /// Get device status
    ///
    /// Returns detailed device status (health, temperature, etc.)
    fn status(&self) -> DeviceStatus;
}

/// Device health and status information
#[derive(Debug, Clone, Copy)]
pub struct DeviceStatus {
    /// Device is healthy
    pub healthy: bool,
    /// Number of read errors
    pub read_errors: u64,
    /// Number of write errors
    pub write_errors: u64,
    /// Device temperature in Celsius (if available)
    pub temperature: Option<i32>,
    /// Percentage of device lifetime used (0-100)
    pub wear_level: Option<u8>,
}

impl Default for DeviceStatus {
    fn default() -> Self {
        Self {
            healthy: true,
            read_errors: 0,
            write_errors: 0,
            temperature: None,
            wear_level: None,
        }
    }
}

// ============================================================================
// Helper Traits
// ============================================================================

/// Partition on a block device
///
/// Represents a logical partition (e.g., from MBR or GPT).
pub trait Partition: BlockDevice {
    /// Get the underlying device
    fn device(&self) -> &dyn BlockDevice;

    /// Get partition offset (in blocks)
    fn offset(&self) -> u64;

    /// Get partition size (in blocks)
    fn size(&self) -> u64;
}

/// Block cache trait
///
/// Optional caching layer for block devices.
pub trait BlockCache: BlockDevice {
    /// Invalidate cache for a range of blocks
    fn invalidate(&mut self, start_block: u64, count: u64);

    /// Get cache statistics
    fn cache_stats(&self) -> CacheStats;
}

/// Cache statistics
#[derive(Debug, Clone, Copy, Default)]
pub struct CacheStats {
    /// Number of cache hits
    pub hits: u64,
    /// Number of cache misses
    pub misses: u64,
    /// Number of dirty blocks
    pub dirty_blocks: usize,
    /// Cache size in blocks
    pub cache_size: usize,
}

impl CacheStats {
    /// Calculate hit rate (0.0 to 1.0)
    pub fn hit_rate(&self) -> f32 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f32 / total as f32
        }
    }
}

/// Card Identification (for SD/MMC/eMMC devices)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cid {
    /// Manufacturer ID
    pub manufacturer_id: u8,
    /// OEM/Application ID (2 ASCII characters)
    pub oem_id: [u8; 2],
    /// Product name (5 ASCII characters)
    pub product_name: [u8; 5],
    /// Product revision (major, minor)
    pub product_revision: (u8, u8),
    /// Product serial number
    pub serial_number: u32,
    /// Manufacturing date (year, month)
    pub manufacturing_date: (u16, u8),
}

impl Cid {
    /// Parse CID from raw 16-byte buffer (big-endian)
    pub fn parse(raw: &[u8; 16]) -> Self {
        Self {
            manufacturer_id: raw[0],
            oem_id: [raw[1], raw[2]],
            product_name: [raw[3], raw[4], raw[5], raw[6], raw[7]],
            product_revision: (raw[8] >> 4, raw[8] & 0x0F),
            serial_number: u32::from_be_bytes([raw[9], raw[10], raw[11], raw[12]]),
            manufacturing_date: Self::parse_mdt(raw[13], raw[14]),
        }
    }

    fn parse_mdt(byte13: u8, byte14: u8) -> (u16, u8) {
        // Big-endian interpretation: byte13 is MSB, byte14 is LSB
        let mdt = u16::from_be_bytes([byte13, byte14]);
        let year = 2000 + ((mdt >> 4) & 0xFF);
        let month = (mdt & 0x0F) as u8;
        (year, month)
    }

    /// Get product name as string (if valid UTF-8)
    pub fn product_name_str(&self) -> Option<&str> {
        core::str::from_utf8(&self.product_name).ok()
    }

    /// Get OEM ID as string (if valid UTF-8)
    pub fn oem_id_str(&self) -> Option<&str> {
        core::str::from_utf8(&self.oem_id).ok()
    }

    pub const fn default() -> Self {
        Self {
            manufacturer_id: 0,
            oem_id: [0; 2],
            product_name: [0; 5],
            product_revision: (0, 0),
            serial_number: 0,
            manufacturing_date: (0, 0),
        }
    }
}

/// Card Specific Data (for SD/MMC/eMMC devices)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Csd {
    /// CSD structure version
    pub version: CsdVersion,
    /// Card capacity in bytes
    pub capacity: u64,
    /// Maximum transfer rate in kbit/s
    pub max_transfer_rate: u32,
    /// Read block length in bytes
    pub read_block_len: u16,
    /// Write block length in bytes
    pub write_block_len: u16,
    /// Card command classes supported (bitmap)
    pub card_command_classes: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsdVersion {
    /// SD Standard Capacity (≤2GB)
    V1_0,
    /// SD High Capacity (SDHC) or Extended Capacity (SDXC)
    V2_0,
    /// SD Ultra Capacity (SDUC)
    V3_0,
}

impl Csd {
    /// Parse CSD from raw 16-byte buffer (big-endian)
    pub fn parse(raw: &[u8; 16]) -> Result<Self, CsdParseError> {
        let version = match raw[0] >> 6 {
            0 => CsdVersion::V1_0,
            1 => CsdVersion::V2_0,
            2 => CsdVersion::V3_0,
            _ => return Err(CsdParseError::UnknownVersion),
        };

        match version {
            CsdVersion::V1_0 => Self::parse_v1(raw, version),
            CsdVersion::V2_0 => Self::parse_v2(raw, version),
            CsdVersion::V3_0 => Self::parse_v3(raw, version),
        }
    }

    fn parse_v2(raw: &[u8; 16], version: CsdVersion) -> Result<Self, CsdParseError> {
        // C_SIZE: bits [69:48] -> 22 bits in bytes 7-9
        // Big-endian: byte 7 is MSB
        let c_size = u32::from_be_bytes([0, raw[7] & 0x3F, raw[8], raw[9]]);

        // Capacity = (C_SIZE + 1) * 512 KB
        let capacity = ((c_size + 1) as u64) * 512 * 1024;

        Ok(Self {
            version,
            capacity,
            max_transfer_rate: Self::parse_tran_speed(raw[3]),
            read_block_len: 512,
            write_block_len: 512,
            card_command_classes: Self::parse_ccc(raw),
        })
    }

    fn parse_v1(raw: &[u8; 16], version: CsdVersion) -> Result<Self, CsdParseError> {
        // Bits 83-80: READ_BL_LEN (low nibble of byte 10)
        let read_bl_len = (raw[10] & 0x0F) as u32;

        // Bits 73-62: C_SIZE (12 bits)
        // Big-endian interpretation across bytes 7-9
        // Extract bits from the 3-byte sequence
        let c_size_bytes = [raw[7], raw[8], raw[9]];
        let c_size_24bit =
            u32::from_be_bytes([0, c_size_bytes[0], c_size_bytes[1], c_size_bytes[2]]);
        // C_SIZE is bits 13-2 of this 24-bit value (counting from LSB)
        let c_size = (c_size_24bit >> 2) & 0xFFF;

        // Bits 49-47: C_SIZE_MULT (3 bits)
        // Located in bytes 5-6
        let mult_bytes = u16::from_be_bytes([raw[5], raw[6]]);
        let c_size_mult = ((mult_bytes >> 7) & 0x07) as u32;

        // SDSC V1 Capacity Formula
        let mult = 1 << (c_size_mult + 2);
        let block_nr = (c_size + 1) * mult;
        let block_len: u16 = 1 << read_bl_len;

        let capacity = block_nr as u64 * block_len as u64;

        Ok(Self {
            version,
            capacity,
            max_transfer_rate: Self::parse_tran_speed(raw[3]),
            read_block_len: block_len,
            write_block_len: block_len,
            card_command_classes: Self::parse_ccc(raw),
        })
    }

    fn parse_v3(raw: &[u8; 16], version: CsdVersion) -> Result<Self, CsdParseError> {
        // For V3 (SDUC) the C_SIZE field layout is compatible with V2 parsing.
        // Big-endian: byte 7 is MSB
        let c_size = u32::from_be_bytes([0, raw[7] & 0x3F, raw[8], raw[9]]);

        // Capacity = (C_SIZE + 1) * 512 KB (same formula as V2)
        let capacity = ((c_size + 1) as u64) * 512 * 1024;

        Ok(Self {
            version,
            capacity,
            max_transfer_rate: Self::parse_tran_speed(raw[3]),
            read_block_len: 512,
            write_block_len: 512,
            card_command_classes: Self::parse_ccc(raw),
        })
    }

    fn parse_tran_speed(byte: u8) -> u32 {
        let time_value = match byte & 0x0F {
            0x1 => 10,
            0x2 => 12,
            0x3 => 13,
            0x4 => 15,
            0x5 => 20,
            0x6 => 25,
            0x7 => 30,
            0x8 => 35,
            0x9 => 40,
            0xA => 45,
            0xB => 50,
            0xC => 55,
            0xD => 60,
            0xE => 70,
            0xF => 80,
            _ => 0,
        };

        let rate_unit = match byte >> 3 {
            0 => 100,     // 100 kbit/s
            1 => 1_000,   // 1 Mbit/s
            2 => 10_000,  // 10 Mbit/s
            3 => 100_000, // 100 Mbit/s
            _ => 0,
        };

        time_value * rate_unit
    }

    fn parse_ccc(raw: &[u8; 16]) -> u16 {
        // Big-endian interpretation: byte 4 contains MSB, byte 5 contains LSB
        u16::from_be_bytes([raw[4] & 0x0F, raw[5]])
    }

    /// Get capacity in megabytes
    pub fn capacity_mb(&self) -> u64 {
        self.capacity / (1024 * 1024)
    }

    /// Get capacity in gigabytes
    pub fn capacity_gb(&self) -> u64 {
        self.capacity / (1024 * 1024 * 1024)
    }

    /// Get number of 512-byte blocks
    pub fn block_count(&self) -> u64 {
        self.capacity / 512
    }

    pub const fn default() -> Self {
        Self {
            version: CsdVersion::V1_0,
            capacity: 0,
            max_transfer_rate: 0,
            read_block_len: 0,
            write_block_len: 0,
            card_command_classes: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsdParseError {
    UnknownVersion,
    InvalidData,
}

/// Extended block device trait for devices with identification
///
/// This trait is optional and only implemented by devices that have
/// CID/CSD registers (SD cards, MMC, eMMC).
pub trait IdentifiableBlockDevice: BlockDevice {
    /// Get Card Identification (if available)
    fn cid(&self) -> Option<&Cid> {
        None
    }

    /// Get Card Specific Data (if available)
    fn csd(&self) -> Option<&Csd> {
        None
    }
}
