use crate::fs::fd::FdError;
use crate::fs::{File, file::FileStat};
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use drivers::hal::block_device::BlockDevice;

/// FAT32 filesystem implementation
pub struct Fat32Fs<D: BlockDevice> {
    dev: Arc<D>,
    fat_info: FatInfo,
}

#[derive(Copy, Clone)]
pub struct FatInfo {
    bytes_per_sector: u16,
    sectors_per_cluster: u8,
    reserved_sector_count: u16,
    num_fats: u8,
    num_dir_entries: u16,
    sectors_per_fat: u32,
    root_cluster: u32,
    fat_start_lba: u32,
    cluster_heap_start_lba: u32,
    partition_start_lba: u32,
}

impl FatInfo {
    pub fn parse(boot_sector: &[u8]) -> Result<Self, Fat32Error> {
        // Parse FAT32 boot sector fields
        Ok(Self {
            bytes_per_sector: u16::from_le_bytes([boot_sector[11], boot_sector[12]]),
            sectors_per_cluster: boot_sector[13],
            reserved_sector_count: u16::from_le_bytes([boot_sector[14], boot_sector[15]]),
            num_fats: boot_sector[16],
            num_dir_entries: u16::from_le_bytes([boot_sector[17], boot_sector[18]]),
            sectors_per_fat: u32::from_le_bytes([
                boot_sector[36],
                boot_sector[37],
                boot_sector[38],
                boot_sector[39],
            ]),
            root_cluster: u32::from_le_bytes([
                boot_sector[44],
                boot_sector[45],
                boot_sector[46],
                boot_sector[47],
            ]),
            fat_start_lba: 0,          // To be calculated
            cluster_heap_start_lba: 0, // To be calculated
            partition_start_lba: 0,    // To be set
        })
    }
}

/// FAT32 file handle
pub struct Fat32File<D: BlockDevice> {
    fs: Arc<Fat32Fs<D>>,
    start_cluster: u32,
    stats: FileStat,
    position: u32,
}

impl<D: BlockDevice> Fat32File<D> {
    pub const fn new(fs: Arc<Fat32Fs<D>>, start_cluster: u32, size: u32) -> Self {
        Self {
            fs,
            start_cluster,
            stats: FileStat {
                size: size as usize,
                is_dir: false,
            },
            position: 0,
        }
    }
}

impl<D: BlockDevice> File for Fat32File<D> {
    fn read(&self, _buf: &mut [u8], _offset: usize) -> Result<usize, FdError> {
        Ok(1)
    }

    fn write(&self, _buf: &[u8], _offset: usize) -> Result<usize, FdError> {
        Ok(1)
    }

    fn seek(&self, _whence: crate::fs::file::SeekWhence, _offset: isize) -> Result<usize, FdError> {
        Ok(1)
    }

    fn size(&self) -> Result<usize, FdError> {
        Ok(1)
    }
}

impl<D: BlockDevice> Fat32Fs<D> {
    /// Mount a FAT32 filesystem
    pub fn mount(dev: Arc<D>) -> Result<Self, Fat32Error> {
        // Read boot sector
        let mut boot_sector = [0u8; 512];
        dev.read_block(0, &mut boot_sector)
            .map_err(|_| Fat32Error::ReadError)?;

        // Parse FAT info
        let fat_info = FatInfo::parse(&boot_sector)?;

        Ok(Self { dev, fat_info })
    }

    /// Open a file
    pub fn open(&self, path: &str) -> Result<Fat32File<D>, Fat32Error> {
        // Implementation using self.dev
        todo!()
    }

    /// List directory
    pub fn ls(&self, path: &str) -> Result<Vec<String>, Fat32Error> {
        // Implementation using self.dev
        todo!()
    }
}

impl<D: BlockDevice> Fat32File<D> {
    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, Fat32Error> {
        // Read using self.fs.dev
        todo!()
    }

    pub fn write(&mut self, buf: &[u8]) -> Result<usize, Fat32Error> {
        // Write using self.fs.dev
        todo!()
    }
}

#[derive(Debug)]
pub enum Fat32Error {
    NotFound,
    IoError,
    ReadError,
    WriteError,
    InvalidPath,
}
