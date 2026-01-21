use core::cell::Cell;
use core::sync::atomic::{AtomicUsize, Ordering};

use crate::fs::fd::FdError;
use crate::fs::file::SeekWhence;
use crate::fs::{File, file::FileStat};
use crate::fs::{FileSystem, FsError};
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use drivers::hal::block_device::BlockDevice;

/// FAT32 filesystem implementation
#[derive(Clone)]
pub struct Fat32Fs {
    dev: Arc<dyn BlockDevice>,
    fat_info: FatInfo,
}

#[derive(Copy, Clone)]
pub struct FatInfo {
    pub bytes_per_sector: u16,
    pub sectors_per_cluster: u8,
    pub reserved_sector_count: u16,
    pub num_fats: u8,
    pub num_dir_entries: u16,
    pub sectors_per_fat: u64,
    pub root_cluster: u32,
    pub fat_start_lba: u64,
    pub cluster_heap_start_lba: u64,
    pub partition_start_lba: u64,
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
            sectors_per_fat: u64::from_le_bytes([
                boot_sector[36],
                boot_sector[37],
                boot_sector[38],
                boot_sector[39],
                0,
                0,
                0,
                0,
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
pub struct Fat32File {
    fs: Arc<Fat32Fs>,
    start_cluster: u32,
    stats: FileStat,
    position: AtomicUsize,
}

impl Fat32File {
    pub const fn new(fs: Arc<Fat32Fs>, start_cluster: u32, size: u32) -> Self {
        Self {
            fs,
            start_cluster,
            stats: FileStat {
                size: size as usize,
                is_dir: false,
            },
            position: AtomicUsize::new(0),
        }
    }
}

impl File for Fat32File {
    fn read(&self, buf: &mut [u8], _offset: usize) -> Result<usize, FdError> {
        // Read from current position
        let position = self.position.load(Ordering::Relaxed); // Load atomically
        let bytes_to_read = buf.len().min(self.stats.size - position);
        if bytes_to_read == 0 {
            return Ok(0);
        }

        let cluster_chain = self
            .fs
            .get_chain(self.start_cluster)
            .map_err(|_| FdError::IoError)?;

        let bytes_per_cluster = (self.fs.fat_info.bytes_per_sector as usize)
            * (self.fs.fat_info.sectors_per_cluster as usize);

        let mut bytes_read = 0;
        let mut file_offset = position;

        while bytes_read < bytes_to_read {
            let cluster_idx = file_offset / bytes_per_cluster;
            let offset_in_cluster = file_offset % bytes_per_cluster;

            if cluster_idx >= cluster_chain.len() {
                break;
            }

            let cluster = cluster_chain[cluster_idx];
            let sector_in_cluster = offset_in_cluster / self.fs.fat_info.bytes_per_sector as usize;
            let offset_in_sector = offset_in_cluster % self.fs.fat_info.bytes_per_sector as usize;

            let lba = self.fs.cluster_to_lba(cluster) + sector_in_cluster as u64;
            let mut sector = vec![0u8; self.fs.fat_info.bytes_per_sector as usize];

            self.fs
                .dev
                .read_block(lba, &mut sector)
                .map_err(|_| FdError::IoError)?;

            let bytes_available = (self.fs.fat_info.bytes_per_sector as usize) - offset_in_sector;
            let bytes_to_copy = bytes_available.min(bytes_to_read - bytes_read);

            buf[bytes_read..bytes_read + bytes_to_copy]
                .copy_from_slice(&sector[offset_in_sector..offset_in_sector + bytes_to_copy]);

            bytes_read += bytes_to_copy;
            file_offset += bytes_to_copy;
        }

        // Update position after reading
        self.position
            .store(position + bytes_read, Ordering::Relaxed); // Store atomically

        Ok(bytes_read)
    }

    fn write(&self, buf: &[u8], _offset: usize) -> Result<usize, FdError> {
        // Write to current position
        let position = self.position.load(Ordering::Relaxed);

        // Calculate how much we can write
        let bytes_to_write = buf.len();
        if bytes_to_write == 0 {
            return Ok(0);
        }

        // For now, we don't support extending files, so cap at current size
        let max_write = self.stats.size.saturating_sub(position);
        if max_write == 0 {
            return Err(FdError::IoError); // Or could return Ok(0) for EOF
        }
        let bytes_to_write = bytes_to_write.min(max_write);

        let cluster_chain = self
            .fs
            .get_chain(self.start_cluster)
            .map_err(|_| FdError::IoError)?;

        let bytes_per_cluster = (self.fs.fat_info.bytes_per_sector as usize)
            * (self.fs.fat_info.sectors_per_cluster as usize);

        let mut bytes_written = 0;
        let mut file_offset = position;

        while bytes_written < bytes_to_write {
            let cluster_idx = file_offset / bytes_per_cluster;
            let offset_in_cluster = file_offset % bytes_per_cluster;

            if cluster_idx >= cluster_chain.len() {
                break;
            }

            let cluster = cluster_chain[cluster_idx];
            let sector_in_cluster = offset_in_cluster / self.fs.fat_info.bytes_per_sector as usize;
            let offset_in_sector = offset_in_cluster % self.fs.fat_info.bytes_per_sector as usize;

            let lba = self.fs.cluster_to_lba(cluster) + sector_in_cluster as u64;

            // For partial sector writes, we need to read-modify-write
            let mut sector = vec![0u8; self.fs.fat_info.bytes_per_sector as usize];

            // Read the existing sector if we're doing a partial write
            let bytes_available = (self.fs.fat_info.bytes_per_sector as usize) - offset_in_sector;
            let bytes_to_copy = bytes_available.min(bytes_to_write - bytes_written);

            if offset_in_sector != 0 || bytes_to_copy < self.fs.fat_info.bytes_per_sector as usize {
                // Partial sector write - need to read first
                self.fs
                    .dev
                    .read_block(lba as u64, &mut sector)
                    .map_err(|_| FdError::IoError)?;
            }

            // Copy data from buffer into sector
            sector[offset_in_sector..offset_in_sector + bytes_to_copy]
                .copy_from_slice(&buf[bytes_written..bytes_written + bytes_to_copy]);

            // Write the modified sector back
            self.fs
                .dev
                .write_block(lba as u64, &sector)
                .map_err(|_| FdError::IoError)?;

            bytes_written += bytes_to_copy;
            file_offset += bytes_to_copy;
        }

        // Update position after writing
        self.position
            .store(position + bytes_written, Ordering::Relaxed);

        Ok(bytes_written)
    }

    fn seek(&self, whence: SeekWhence, offset: isize) -> Result<usize, FdError> {
        let current = self.position.load(Ordering::Relaxed); // Load atomically

        let new_position = match whence {
            SeekWhence::Start => offset.max(0) as usize,
            SeekWhence::Current => (current as isize + offset).max(0) as usize,
            SeekWhence::End => (self.stats.size as isize + offset).max(0) as usize,
        };

        // Clamp position to file size
        let new_position = new_position.min(self.stats.size);
        self.position.store(new_position, Ordering::Relaxed); // Store atomically

        Ok(new_position)
    }
    fn size(&self) -> Result<usize, FdError> {
        Ok(self.stats.size)
    }
}

impl Fat32Fs {
    pub fn mount(dev: Arc<dyn BlockDevice>) -> Result<Arc<Self>, Fat32Error> {
        let mut mbr = [0u8; 512];
        dev.read_block(0, &mut mbr)
            .map_err(|_| Fat32Error::ReadError)?;

        let partition_start_lba = u32::from_le_bytes([mbr[454], mbr[455], mbr[456], mbr[457]]);

        let mut boot = [0u8; 512];
        dev.read_block(partition_start_lba as u64, &mut boot)
            .map_err(|_| Fat32Error::ReadError)?;

        let mut fat = FatInfo::parse(&boot)?;
        fat.partition_start_lba = partition_start_lba as u64;
        fat.fat_start_lba = partition_start_lba as u64 + fat.reserved_sector_count as u64;
        let total_fat_sectors = (fat.num_fats as u64) * fat.sectors_per_fat;
        fat.cluster_heap_start_lba = fat.fat_start_lba + total_fat_sectors;

        let fs = Self { dev, fat_info: fat };

        Ok(Arc::new(fs))
    }

    pub fn open(self: &Arc<Self>, path: &str) -> Result<Fat32File, Fat32Error> {
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        if parts.is_empty() {
            return Err(Fat32Error::InvalidPath);
        }

        // Navigate to parent directory
        let parent_parts = &parts[..parts.len() - 1];
        let parent_cluster = if parent_parts.is_empty() {
            self.fat_info.root_cluster
        } else {
            let parent_path = parent_parts.join("/");
            self.navigate_to_dir(&parent_path)?
        };

        // Find the file in the parent directory
        let file_name = parts[parts.len() - 1];
        let entry = self.find_entry(parent_cluster, file_name)?;

        if entry.is_dir {
            return Err(Fat32Error::IsADirectory);
        }

        Ok(Fat32File::new(
            self.clone(),
            entry.first_cluster,
            entry.size,
        ))
    }

    pub fn ls(&self, path: &str) -> Result<Vec<String>, Fat32Error> {
        let cluster = self.navigate_to_dir(path)?;
        let entries = self.list_entries(cluster)?;
        Ok(entries.into_iter().map(|e| e.name).collect())
    }

    pub fn stat(&self, path: &str) -> Result<FileStat, Fat32Error> {
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

        // Root directory
        if parts.is_empty() {
            return Ok(FileStat {
                size: 0,
                is_dir: true,
            });
        }

        // Navigate to parent directory
        let parent_parts = &parts[..parts.len() - 1];
        let parent_cluster = if parent_parts.is_empty() {
            self.fat_info.root_cluster
        } else {
            let parent_path = parts[..parts.len() - 1].join("/");
            self.navigate_to_dir(&parent_path)?
        };

        // Find the entry
        let name = parts[parts.len() - 1];
        let entry = self.find_entry(parent_cluster, name)?;

        Ok(FileStat {
            size: entry.size as usize,
            is_dir: entry.is_dir,
        })
    }

    // ============================================================================
    // Helper Methods
    // ============================================================================

    fn navigate_to_dir(&self, path: &str) -> Result<u32, Fat32Error> {
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

        // Empty path means root directory
        if parts.is_empty() {
            return Ok(self.fat_info.root_cluster);
        }

        let mut current_cluster = self.fat_info.root_cluster;

        for part in parts.iter() {
            let entry = self.find_entry(current_cluster, part)?;

            if !entry.is_dir {
                return Err(Fat32Error::InvalidPath);
            }

            current_cluster = entry.first_cluster;
        }

        Ok(current_cluster)
    }

    fn list_entries(&self, start_cluster: u32) -> Result<Vec<DirEntry>, Fat32Error> {
        let mut entries = Vec::new();
        let mut sector = vec![0u8; self.fat_info.bytes_per_sector as usize];
        let chain = self.get_chain(start_cluster)?;

        for cluster in chain {
            let base = self.cluster_to_lba(cluster);
            for s in 0..self.fat_info.sectors_per_cluster as u32 {
                self.dev
                    .read_block(base + s as u64, &mut sector)
                    .map_err(|_| Fat32Error::ReadError)?;

                for i in 0..sector.len() / 32 {
                    let raw = &sector[i * 32..i * 32 + 32];

                    if raw[0] == 0x00 {
                        // End of directory
                        return Ok(entries);
                    }
                    if let Some(e) = parse_dir_entry(raw) {
                        entries.push(e);
                    }
                }
            }
        }
        Ok(entries)
    }

    fn cluster_to_lba(&self, cluster: u32) -> u64 {
        self.fat_info.cluster_heap_start_lba
            + (cluster - 2) as u64 * self.fat_info.sectors_per_cluster as u64
    }

    /// Read FAT entry for a given cluster
    fn read_fat_entry(&self, cluster: u32) -> Result<u32, Fat32Error> {
        let bytes_per_sector = self.fat_info.bytes_per_sector as u64;

        // FAT32 entry = 4 bytes per cluster
        let offset = cluster as u64 * 4;
        let sector = self.fat_info.fat_start_lba + (offset / bytes_per_sector);
        let idx = (offset % bytes_per_sector) as usize;

        let mut buf = vec![0u8; self.fat_info.bytes_per_sector as usize];
        self.dev
            .read_block(sector, &mut buf)
            .map_err(|_| Fat32Error::ReadError)?;

        let entry = if idx + 4 <= buf.len() {
            u32::from_le_bytes([buf[idx], buf[idx + 1], buf[idx + 2], buf[idx + 3]])
        } else {
            // Entry crosses sector boundary → read next sector
            let mut next = vec![0u8; self.fat_info.bytes_per_sector as usize];
            self.dev
                .read_block(sector + 1, &mut next)
                .map_err(|_| Fat32Error::ReadError)?;

            let mut tmp = [0u8; 4];
            let first = buf.len() - idx;
            tmp[..first].copy_from_slice(&buf[idx..]);
            tmp[first..].copy_from_slice(&next[..4 - first]);
            u32::from_le_bytes(tmp)
        };

        Ok(entry & 0x0FFF_FFFF)
    }

    /// Get the full cluster chain starting from a given cluster
    fn get_chain(&self, start: u32) -> Result<Vec<u32>, Fat32Error> {
        const FAT32_EOC: u32 = 0x0FFFFFF8;
        let mut chain = Vec::new();
        let mut cur = start;

        loop {
            if cur < 2 {
                return Err(Fat32Error::InvalidPath);
            }

            chain.push(cur);

            let next = self.read_fat_entry(cur)?;

            if next >= FAT32_EOC {
                break;
            }

            if next == 0 {
                return Err(Fat32Error::InvalidPath); // free cluster in chain
            }

            cur = next;
        }

        Ok(chain)
    }

    /// Find a directory entry by name in a given directory cluster
    fn find_entry(&self, start_cluster: u32, name: &str) -> Result<DirEntry, Fat32Error> {
        let mut sector = vec![0u8; self.fat_info.bytes_per_sector as usize];
        let chain = self.get_chain(start_cluster)?;

        for cluster in chain {
            let base = self.cluster_to_lba(cluster);
            for s in 0..self.fat_info.sectors_per_cluster as u32 {
                self.dev
                    .read_block(base + s as u64, &mut sector)
                    .map_err(|_| Fat32Error::ReadError)?;

                for i in 0..sector.len() / 32 {
                    let raw = &sector[i * 32..i * 32 + 32];

                    if raw[0] == 0x00 {
                        // End of directory
                        return Err(Fat32Error::NotFound);
                    }
                    if let Some(e) = parse_dir_entry(raw) {
                        if e.name.eq_ignore_ascii_case(name) {
                            return Ok(e);
                        }
                    }
                }
            }
        }
        Err(Fat32Error::NotFound)
    }
}

fn parse_dir_entry(raw: &[u8]) -> Option<DirEntry> {
    if raw[0] == 0xE5 {
        return None;
    }
    let attr = raw[11];
    if attr == 0x0F || attr & 0x08 != 0 {
        return None;
    }

    let name = parse_83(raw);
    let hi = u16::from_le_bytes([raw[20], raw[21]]) as u32;
    let lo = u16::from_le_bytes([raw[26], raw[27]]) as u32;
    let size = u32::from_le_bytes([raw[28], raw[29], raw[30], raw[31]]);

    if name == "." || name == ".." {
        return None;
    }

    let first_cluster = (hi << 16) | lo;

    if first_cluster < 2 && size != 0 {
        return None;
    }

    Some(DirEntry {
        name,
        first_cluster: (hi << 16) | lo,
        size,
        is_dir: attr & 0x10 != 0,
    })
}

fn parse_83(raw: &[u8]) -> String {
    let base = core::str::from_utf8(&raw[0..8]).unwrap_or("").trim_end();
    let ext = core::str::from_utf8(&raw[8..11]).unwrap_or("").trim_end();

    if ext.is_empty() {
        base.to_string()
    } else {
        alloc::format!("{}.{}", base, ext)
    }
}

impl FileSystem for Fat32Fs {
    fn open(&self, path: &str) -> Result<Arc<dyn File>, FsError> {
        let file = Fat32Fs::open(&Arc::new(self.clone()), path)?;
        Ok(Arc::new(file))
    }

    fn create(&self, _p: &str) -> Result<Arc<dyn File>, FsError> {
        todo!()
    }
    fn delete(&self, _p: &str) -> Result<(), FsError> {
        todo!()
    }
    fn ls(&self, p: &str) -> Result<Vec<String>, FsError> {
        Ok(Fat32Fs::ls(self, p)?)
    }
    fn mkdir(&self, _p: &str) -> Result<(), FsError> {
        todo!()
    }
    fn rmdir(&self, _p: &str) -> Result<(), FsError> {
        todo!()
    }
    fn stat(&self, p: &str) -> Result<FileStat, FsError> {
        Ok(Fat32Fs::stat(self, p)?)
    }
}

#[derive(Debug)]
pub enum Fat32Error {
    NotFound,
    IoError,
    ReadError,
    WriteError,
    InvalidPath,
    IsADirectory,
}

impl From<Fat32Error> for crate::fs::FsError {
    fn from(err: Fat32Error) -> Self {
        match err {
            Fat32Error::NotFound => crate::fs::FsError::NotFound,
            Fat32Error::IoError | Fat32Error::ReadError | Fat32Error::WriteError => {
                crate::fs::FsError::IoError
            }
            Fat32Error::InvalidPath => crate::fs::FsError::NotFound,
            Fat32Error::IsADirectory => crate::fs::FsError::IsADirectory,
        }
    }
}

#[repr(u8)]
enum Fat32Attribute {
    ReadOnly = 0x1,
    Hidden = 0x2,
    System = 0x4,
    VolumeId = 0x8,
    Directory = 0x10,
    Archive = 0x20,
    LongFilename = 0x0F,
}

struct DirEntry {
    name: String,
    first_cluster: u32,
    size: u32,
    is_dir: bool,
}
