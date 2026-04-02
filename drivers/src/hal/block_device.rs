//! Block device abstractions for generic storage access.
//!
//! Provides core traits for block-based I/O (`BlockDevice`, `DynBlockDevice`),
//! along with supporting types for device metadata (`BlockDeviceInfo`),
//! errors (`BlockDeviceError`), and status reporting.
//!
//! Optional extension traits add features like erase/trim operations,
//! caching, partitions, and device identification (CID/CSD for SD/MMC).
//!
//! Designed for low-level systems (kernels, bootloaders, embedded) with
//! thread-safe (`Send + Sync`) implementations operating on fixed-size blocks.

// Device info

#[derive(Debug, Clone, Copy)]
pub struct BlockDeviceInfo {
    pub block_size: usize,
    pub block_count: u64,
    pub capacity: u64,
    pub read_only: bool,
    pub removable: bool,
}

impl BlockDeviceInfo {
    pub fn new(block_count: u64) -> Self {
        Self {
            block_size: 512,
            block_count,
            capacity: block_count * 512,
            read_only: false,
            removable: false,
        }
    }

    pub fn with_block_size(block_size: usize, block_count: u64) -> Self {
        Self {
            block_size,
            block_count,
            capacity: (block_count as usize * block_size) as u64,
            read_only: false,
            removable: false,
        }
    }

    pub fn read_only(mut self) -> Self {
        self.read_only = true;
        self
    }
    pub fn removable(mut self) -> Self {
        self.removable = true;
        self
    }
}

// Canonical error type

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockDeviceError {
    NotReady,
    InvalidAddress,
    ReadError,
    WriteError,
    WriteProtected,
    InvalidBuffer,
    Timeout,
    DataError,
    DeviceRemoved,
    IoError,
    UnsupportedDevice,
    Other,
}

// BlockDevice: generic concrete trait
//
// Drivers implement this once with their own Error type.
// The only requirement beyond the methods is:
//   type Error: Into<BlockDeviceError>
// which lets the blanket impl below convert errors automatically.

pub trait BlockDevice: Send + Sync {
    type Error: core::fmt::Debug + Into<BlockDeviceError>;

    fn info(&self) -> BlockDeviceInfo;

    fn read_blocks(&self, start_block: u64, buffers: &mut [&mut [u8]]) -> Result<(), Self::Error>;
    fn write_blocks(&self, start_block: u64, buffers: &[&[u8]]) -> Result<(), Self::Error>;

    /// Convenience: read a single block.
    fn read_block(&self, block: u64, buffer: &mut [u8]) -> Result<(), Self::Error> {
        self.read_blocks(block, &mut [buffer])
    }

    /// Convenience: write a single block.
    fn write_block(&self, block: u64, buffer: &[u8]) -> Result<(), Self::Error> {
        self.write_blocks(block, &[buffer])
    }

    /// Flush pending writes. Default: no-op (assumes immediate persistence).
    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Returns true if the device is initialised and ready for I/O.
    fn is_ready(&self) -> bool {
        true
    }
}

// BlockDeviceExt: optional advanced operationS
//
// Supertrait of BlockDevice, so Into<BlockDeviceError> is already implied.

pub trait BlockDeviceExt: BlockDevice {
    fn erase_blocks(&mut self, start_block: u64, count: u64) -> Result<(), Self::Error>;
    fn trim_blocks(&mut self, start_block: u64, count: u64) -> Result<(), Self::Error>;
    fn status(&self) -> DeviceStatus;
}

// DynBlockDevice: object-safe type-erased trait
//
// The device manager stores Box<dyn DynBlockDevice>.
// Never implement this by hand — the blanket impl below does it automatically
// for any T: BlockDevice.

pub trait DynBlockDevice: Send + Sync {
    fn info(&self) -> BlockDeviceInfo;
    fn read_blocks(
        &self,
        start_block: u64,
        buffers: &mut [&mut [u8]],
    ) -> Result<(), BlockDeviceError>;
    fn write_blocks(&self, start_block: u64, buffers: &[&[u8]]) -> Result<(), BlockDeviceError>;
    fn read_block(&self, block: u64, buffer: &mut [u8]) -> Result<(), BlockDeviceError>;
    fn write_block(&self, block: u64, buffer: &[u8]) -> Result<(), BlockDeviceError>;
    fn flush(&mut self) -> Result<(), BlockDeviceError>;
    fn is_ready(&self) -> bool;
}

/// Blanket impl: any BlockDevice (whose Error converts into BlockDeviceError)
/// automatically becomes a DynBlockDevice.
impl<T: BlockDevice> DynBlockDevice for T {
    fn info(&self) -> BlockDeviceInfo {
        BlockDevice::info(self)
    }
    fn read_blocks(
        &self,
        start_block: u64,
        buffers: &mut [&mut [u8]],
    ) -> Result<(), BlockDeviceError> {
        BlockDevice::read_blocks(self, start_block, buffers).map_err(Into::into)
    }
    fn write_blocks(&self, start_block: u64, buffers: &[&[u8]]) -> Result<(), BlockDeviceError> {
        BlockDevice::write_blocks(self, start_block, buffers).map_err(Into::into)
    }
    fn read_block(&self, block: u64, buffer: &mut [u8]) -> Result<(), BlockDeviceError> {
        BlockDevice::read_block(self, block, buffer).map_err(Into::into)
    }
    fn write_block(&self, block: u64, buffer: &[u8]) -> Result<(), BlockDeviceError> {
        BlockDevice::write_block(self, block, buffer).map_err(Into::into)
    }
    fn flush(&mut self) -> Result<(), BlockDeviceError> {
        BlockDevice::flush(self).map_err(Into::into)
    }
    fn is_ready(&self) -> bool {
        BlockDevice::is_ready(self)
    }
}

// DynBlockDeviceExT

pub trait DynBlockDeviceExt: DynBlockDevice {
    fn erase_blocks(&mut self, start_block: u64, count: u64) -> Result<(), BlockDeviceError>;
    fn trim_blocks(&mut self, start_block: u64, count: u64) -> Result<(), BlockDeviceError>;
    fn status(&self) -> DeviceStatus;
}

/// Blanket impl: any BlockDeviceExt automatically becomes a DynBlockDeviceExt.
/// DynBlockDevice is already covered by the blanket impl above.
impl<T: BlockDeviceExt> DynBlockDeviceExt for T {
    fn erase_blocks(&mut self, start_block: u64, count: u64) -> Result<(), BlockDeviceError> {
        BlockDeviceExt::erase_blocks(self, start_block, count).map_err(Into::into)
    }
    fn trim_blocks(&mut self, start_block: u64, count: u64) -> Result<(), BlockDeviceError> {
        BlockDeviceExt::trim_blocks(self, start_block, count).map_err(Into::into)
    }
    fn status(&self) -> DeviceStatus {
        BlockDeviceExt::status(self)
    }
}

// IdentifiableBlockDevice

pub trait IdentifiableBlockDevice: BlockDevice {
    fn cid(&self) -> Option<&Cid> {
        None
    }
    fn csd(&self) -> Option<&Csd> {
        None
    }
}

pub trait DynIdentifiableBlockDevice: DynBlockDevice {
    fn cid(&self) -> Option<&Cid>;
    fn csd(&self) -> Option<&Csd>;
}

impl<T: IdentifiableBlockDevice> DynIdentifiableBlockDevice for T {
    fn cid(&self) -> Option<&Cid> {
        IdentifiableBlockDevice::cid(self)
    }
    fn csd(&self) -> Option<&Csd> {
        IdentifiableBlockDevice::csd(self)
    }
}

// DeviceStatus

#[derive(Debug, Clone, Copy)]
pub struct DeviceStatus {
    pub healthy: bool,
    pub read_errors: u64,
    pub write_errors: u64,
    pub temperature: Option<i32>,
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

pub trait Partition: BlockDevice {
    fn device(&self) -> &dyn DynBlockDevice;
    fn offset(&self) -> u64;
    fn size(&self) -> u64;
}

pub trait BlockCache: BlockDevice {
    fn invalidate(&mut self, start_block: u64, count: u64);
    fn cache_stats(&self) -> CacheStats;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub dirty_blocks: usize,
    pub cache_size: usize,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f32 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f32 / total as f32
        }
    }
}

// CID / CSD (SD/MMC identification)

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cid {
    pub manufacturer_id: u8,
    pub oem_id: [u8; 2],
    pub product_name: [u8; 5],
    pub product_revision: (u8, u8),
    pub serial_number: u32,
    pub manufacturing_date: (u16, u8),
}

impl Cid {
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
        let mdt = u16::from_be_bytes([byte13, byte14]);
        let year = 2000 + ((mdt >> 4) & 0xFF);
        let month = (mdt & 0x0F) as u8;
        (year, month)
    }

    pub fn product_name_str(&self) -> Option<&str> {
        core::str::from_utf8(&self.product_name).ok()
    }
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Csd {
    pub version: CsdVersion,
    pub capacity: u64,
    pub max_transfer_rate: u32,
    pub read_block_len: u16,
    pub write_block_len: u16,
    pub card_command_classes: u16,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CardType {
    Unknown,
    SDv1,
    SDv2,
    MMC,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsdVersion {
    V1_0,
    V2_0,
    V3_0,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsdParseError {
    UnknownVersion,
    InvalidData,
}

impl Csd {
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
        let c_size = u32::from_be_bytes([0, raw[7] & 0x3F, raw[8], raw[9]]);
        Ok(Self {
            version,
            capacity: ((c_size + 1) as u64) * 512 * 1024,
            max_transfer_rate: Self::parse_tran_speed(raw[3]),
            read_block_len: 512,
            write_block_len: 512,
            card_command_classes: Self::parse_ccc(raw),
        })
    }

    fn parse_v1(raw: &[u8; 16], version: CsdVersion) -> Result<Self, CsdParseError> {
        let read_bl_len = (raw[10] & 0x0F) as u32;
        let c_size_24bit = u32::from_be_bytes([0, raw[7], raw[8], raw[9]]);
        let c_size = (c_size_24bit >> 2) & 0xFFF;
        let mult_bytes = u16::from_be_bytes([raw[5], raw[6]]);
        let c_size_mult = ((mult_bytes >> 7) & 0x07) as u32;
        let mult = 1 << (c_size_mult + 2);
        let block_len: u16 = 1 << read_bl_len;
        Ok(Self {
            version,
            capacity: (c_size + 1) as u64 * mult as u64 * block_len as u64,
            max_transfer_rate: Self::parse_tran_speed(raw[3]),
            read_block_len: block_len,
            write_block_len: block_len,
            card_command_classes: Self::parse_ccc(raw),
        })
    }

    fn parse_v3(raw: &[u8; 16], version: CsdVersion) -> Result<Self, CsdParseError> {
        let c_size = u32::from_be_bytes([0, raw[7] & 0x3F, raw[8], raw[9]]);
        Ok(Self {
            version,
            capacity: ((c_size + 1) as u64) * 512 * 1024,
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
            0 => 100,
            1 => 1_000,
            2 => 10_000,
            3 => 100_000,
            _ => 0,
        };
        time_value * rate_unit
    }

    fn parse_ccc(raw: &[u8; 16]) -> u16 {
        u16::from_be_bytes([raw[4] & 0x0F, raw[5]])
    }

    pub fn capacity_mb(&self) -> u64 {
        self.capacity / (1024 * 1024)
    }
    pub fn capacity_gb(&self) -> u64 {
        self.capacity / (1024 * 1024 * 1024)
    }
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
