use crate::fs::fd::FdError;
use crate::fs::file::FileType;
use crate::fs::{File, file::FileStat};
use crate::fs::{FileSystem, FsError};
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use common::sync::{RwLock, SpinLock};
use drivers::hal::block_device::BlockDevice;

/// FAT32 filesystem implementation
#[derive(Clone)]
pub struct Fat32Fs {
    dev: Arc<dyn BlockDevice>,
    fat_info: FatInfo,
    // Protects metadata operations (create, delete, mkdir, rmdir)
    metadata_lock: Arc<RwLock<()>>,
    // Protects FAT table access
    fat_lock: Arc<SpinLock<()>>,
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
    pub total_clusters: u32,
}

impl FatInfo {
    pub fn parse(boot_sector: &[u8]) -> Result<Self, Fat32Error> {
        let bytes_per_sector = u16::from_le_bytes([boot_sector[11], boot_sector[12]]);
        let sectors_per_cluster = boot_sector[13];
        let reserved_sector_count = u16::from_le_bytes([boot_sector[14], boot_sector[15]]);
        let num_fats = boot_sector[16];
        let sectors_per_fat = u32::from_le_bytes([
            boot_sector[36],
            boot_sector[37],
            boot_sector[38],
            boot_sector[39],
        ]) as u64;

        let total_sectors = {
            let small = u16::from_le_bytes([boot_sector[19], boot_sector[20]]) as u32;
            if small != 0 {
                small
            } else {
                u32::from_le_bytes([
                    boot_sector[32],
                    boot_sector[33],
                    boot_sector[34],
                    boot_sector[35],
                ])
            }
        };

        let data_sectors = total_sectors as u64
            - reserved_sector_count as u64
            - (num_fats as u64 * sectors_per_fat);
        let total_clusters = (data_sectors / sectors_per_cluster as u64) as u32;

        Ok(Self {
            bytes_per_sector,
            sectors_per_cluster,
            reserved_sector_count,
            num_fats,
            num_dir_entries: u16::from_le_bytes([boot_sector[17], boot_sector[18]]),
            sectors_per_fat,
            root_cluster: u32::from_le_bytes([
                boot_sector[44],
                boot_sector[45],
                boot_sector[46],
                boot_sector[47],
            ]),
            fat_start_lba: 0,
            cluster_heap_start_lba: 0,
            partition_start_lba: 0,
            total_clusters,
        })
    }
}

/// FAT32 file handle
pub struct Fat32File {
    fs: Arc<Fat32Fs>,
    start_cluster: u32,
    size: Arc<SpinLock<u32>>, // Mutable size for extending
    name: String,
    // Protects concurrent I/O operations on this file
    io_lock: SpinLock<()>,
}

impl Fat32File {
    pub fn new(fs: Arc<Fat32Fs>, start_cluster: u32, size: u32, name: String) -> Self {
        // Validate cluster for non-empty files
        if start_cluster < 2 && size > 0 {
            panic!("Invalid cluster {} for non-empty file", start_cluster);
        }

        Self {
            fs,
            start_cluster,
            size: Arc::new(SpinLock::new(size)),
            name,
            io_lock: SpinLock::new(()),
        }
    }

    /// Get current file size
    fn get_size(&self) -> u32 {
        *self.size.lock()
    }

    /// Set file size (internal use only)
    fn set_size(&self, new_size: u32) {
        *self.size.lock() = new_size;
    }
}

impl File for Fat32File {
    fn read(&self, buf: &mut [u8], offset: usize) -> Result<usize, FdError> {
        // Lock to prevent reading during concurrent write
        let _guard = self.io_lock.lock();

        let file_size = self.get_size() as usize;

        // Check if offset is beyond file size
        if offset >= file_size {
            return Ok(0); // EOF
        }

        // Calculate bytes to read (don't read past EOF)
        let bytes_to_read = buf.len().min(file_size - offset);
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
        let mut file_offset = offset;

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

        Ok(bytes_read)
    }

    fn write(&self, buf: &[u8], offset: usize) -> Result<usize, FdError> {
        // Lock to prevent concurrent writes or reads during write
        let _guard = self.io_lock.lock();

        let bytes_to_write = buf.len();
        if bytes_to_write == 0 {
            return Ok(0);
        }

        let current_size = self.get_size() as usize;
        let new_size = offset + bytes_to_write;

        // Extend file if needed
        if new_size > current_size {
            self.fs
                .extend_file(self.start_cluster, new_size)
                .map_err(|_| FdError::IoError)?;
            self.set_size(new_size as u32);
        }

        let cluster_chain = self
            .fs
            .get_chain(self.start_cluster)
            .map_err(|_| FdError::IoError)?;

        let bytes_per_cluster = (self.fs.fat_info.bytes_per_sector as usize)
            * (self.fs.fat_info.sectors_per_cluster as usize);

        let mut bytes_written = 0;
        let mut file_offset = offset;

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

            let bytes_available = (self.fs.fat_info.bytes_per_sector as usize) - offset_in_sector;
            let bytes_to_copy = bytes_available.min(bytes_to_write - bytes_written);

            // Read existing sector if we're doing a partial write
            if offset_in_sector != 0 || bytes_to_copy < self.fs.fat_info.bytes_per_sector as usize {
                self.fs
                    .dev
                    .read_block(lba, &mut sector)
                    .map_err(|_| FdError::IoError)?;
            }

            // Copy data from buffer into sector
            sector[offset_in_sector..offset_in_sector + bytes_to_copy]
                .copy_from_slice(&buf[bytes_written..bytes_written + bytes_to_copy]);

            // Write the modified sector back
            self.fs
                .dev
                .write_block(lba, &sector)
                .map_err(|_| FdError::IoError)?;

            bytes_written += bytes_to_copy;
            file_offset += bytes_to_copy;
        }

        Ok(bytes_written)
    }

    fn stat(&self) -> Result<FileStat, FdError> {
        Ok(FileStat {
            size: self.get_size() as usize,
            file_type: FileType::Regular,
            name: self.name.clone(),
        })
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

        let fs = Self {
            dev,
            fat_info: fat,
            metadata_lock: Arc::new(RwLock::new(())),
            fat_lock: Arc::new(SpinLock::new(())),
        };

        Ok(Arc::new(fs))
    }

    pub fn open(self: &Arc<Self>, path: &str) -> Result<Fat32File, Fat32Error> {
        // Shared lock for reading directory structure
        let _guard = self.metadata_lock.read();

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
            entry.name,
        ))
    }

    pub fn ls(&self, path: &str) -> Result<Vec<String>, Fat32Error> {
        // Shared lock for reading
        let _guard = self.metadata_lock.read();

        let cluster = self.navigate_to_dir(path)?;
        let entries = self.list_entries(cluster)?;
        Ok(entries.into_iter().map(|e| e.name).collect())
    }

    pub fn stat(&self, path: &str) -> Result<FileStat, Fat32Error> {
        // Shared lock for reading
        let _guard = self.metadata_lock.read();

        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

        // Root directory
        if parts.is_empty() {
            return Ok(FileStat {
                size: 0,
                file_type: FileType::Directory,
                name: String::new(),
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
            file_type: if entry.is_dir {
                FileType::Directory
            } else {
                FileType::Regular
            },
            name: entry.name,
        })
    }

    // ============================================================================
    // Cluster Management
    // ============================================================================

    /// Allocate a free cluster
    fn alloc_cluster(&self) -> Result<u32, Fat32Error> {
        let _guard = self.fat_lock.lock();

        // Search for a free cluster (entry == 0)
        for cluster in 2..self.fat_info.total_clusters {
            let entry = self.read_fat_entry_unlocked(cluster)?;
            if entry == 0 {
                // Mark as end of chain
                self.write_fat_entry_unlocked(cluster, 0x0FFFFFFF)?;
                return Ok(cluster);
            }
        }

        Err(Fat32Error::DiskFull)
    }

    /// Link a cluster to the end of a chain
    fn link_cluster(&self, last_cluster: u32, new_cluster: u32) -> Result<(), Fat32Error> {
        let _guard = self.fat_lock.lock();

        // Update last cluster to point to new cluster
        self.write_fat_entry_unlocked(last_cluster, new_cluster)?;
        // Mark new cluster as end of chain
        self.write_fat_entry_unlocked(new_cluster, 0x0FFFFFFF)?;

        Ok(())
    }

    /// Extend file to accommodate new size
    fn extend_file(&self, start_cluster: u32, new_size: usize) -> Result<(), Fat32Error> {
        let bytes_per_cluster = (self.fat_info.bytes_per_sector as usize)
            * (self.fat_info.sectors_per_cluster as usize);

        let clusters_needed = (new_size + bytes_per_cluster - 1) / bytes_per_cluster;

        let chain = self.get_chain(start_cluster)?;
        let current_clusters = chain.len();

        if clusters_needed <= current_clusters {
            return Ok(());
        }

        let clusters_to_add = clusters_needed - current_clusters;
        let mut last_cluster = *chain.last().unwrap();

        for _ in 0..clusters_to_add {
            let new_cluster = self.alloc_cluster()?;
            self.link_cluster(last_cluster, new_cluster)?;
            last_cluster = new_cluster;
        }

        Ok(())
    }

    // ============================================================================
    // FAT Table Operations
    // ============================================================================

    /// Read FAT entry for a given cluster (without lock - internal use)
    fn read_fat_entry_unlocked(&self, cluster: u32) -> Result<u32, Fat32Error> {
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

    /// Read FAT entry for a given cluster (with lock)
    fn read_fat_entry(&self, cluster: u32) -> Result<u32, Fat32Error> {
        let _guard = self.fat_lock.lock();
        self.read_fat_entry_unlocked(cluster)
    }

    /// Write FAT entry for a given cluster (without lock - internal use)
    fn write_fat_entry_unlocked(&self, cluster: u32, value: u32) -> Result<(), Fat32Error> {
        let bytes_per_sector = self.fat_info.bytes_per_sector as u64;

        // Mask to preserve reserved bits
        let value = value & 0x0FFF_FFFF;

        // FAT32 entry = 4 bytes per cluster
        let offset = cluster as u64 * 4;
        let sector = self.fat_info.fat_start_lba + (offset / bytes_per_sector);
        let idx = (offset % bytes_per_sector) as usize;

        let mut buf = vec![0u8; self.fat_info.bytes_per_sector as usize];
        self.dev
            .read_block(sector, &mut buf)
            .map_err(|_| Fat32Error::ReadError)?;

        if idx + 4 <= buf.len() {
            // Entry fits in one sector
            let bytes = value.to_le_bytes();
            buf[idx..idx + 4].copy_from_slice(&bytes);
            self.dev
                .write_block(sector, &buf)
                .map_err(|_| Fat32Error::WriteError)?;
        } else {
            // Entry crosses sector boundary
            let mut next = vec![0u8; self.fat_info.bytes_per_sector as usize];
            self.dev
                .read_block(sector + 1, &mut next)
                .map_err(|_| Fat32Error::ReadError)?;

            let bytes = value.to_le_bytes();
            let first = buf.len() - idx;
            buf[idx..].copy_from_slice(&bytes[..first]);
            next[..4 - first].copy_from_slice(&bytes[first..]);

            self.dev
                .write_block(sector, &buf)
                .map_err(|_| Fat32Error::WriteError)?;
            self.dev
                .write_block(sector + 1, &next)
                .map_err(|_| Fat32Error::WriteError)?;
        }

        // Write to all FAT copies
        for fat_idx in 1..self.fat_info.num_fats {
            let fat_sector = sector + (fat_idx as u64 * self.fat_info.sectors_per_fat);
            self.dev
                .write_block(fat_sector, &buf)
                .map_err(|_| Fat32Error::WriteError)?;
        }

        Ok(())
    }

    /// Get the full cluster chain starting from a given cluster
    fn get_chain(&self, start: u32) -> Result<Vec<u32>, Fat32Error> {
        const FAT32_EOC: u32 = 0x0FFFFFF8;
        let mut chain = Vec::new();
        let mut cur = start;

        loop {
            if cur < 2 {
                return Err(Fat32Error::InvalidCluster);
            }

            chain.push(cur);

            let next = self.read_fat_entry(cur)?;

            if next >= FAT32_EOC {
                break;
            }

            if next == 0 {
                return Err(Fat32Error::InvalidCluster);
            }

            cur = next;
        }

        Ok(chain)
    }

    // ============================================================================
    // Helper Methods
    // ============================================================================

    fn cluster_to_lba(&self, cluster: u32) -> u64 {
        self.fat_info.cluster_heap_start_lba
            + (cluster - 2) as u64 * self.fat_info.sectors_per_cluster as u64
    }

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
                return Err(Fat32Error::NotADirectory);
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

// ============================================================================
// Directory Entry Parsing
// ============================================================================

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
        first_cluster,
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

// ============================================================================
// FileSystem Trait Implementation
// ============================================================================

impl FileSystem for Fat32Fs {
    fn open(&self, path: &str) -> Result<Arc<dyn File>, FsError> {
        let file = Fat32Fs::open(&Arc::new(self.clone()), path)?;
        Ok(Arc::new(file))
    }

    fn create(&self, _p: &str) -> Result<Arc<dyn File>, FsError> {
        let _guard = self.metadata_lock.write();
        // TODO: Implement file creation
        Err(FsError::NotSupported)
    }

    fn delete(&self, _p: &str) -> Result<(), FsError> {
        let _guard = self.metadata_lock.write();
        // TODO: Implement file deletion
        Err(FsError::NotSupported)
    }

    fn ls(&self, p: &str) -> Result<Vec<String>, FsError> {
        Ok(Fat32Fs::ls(self, p)?)
    }

    fn mkdir(&self, _p: &str) -> Result<(), FsError> {
        let _guard = self.metadata_lock.write();
        // TODO: Implement directory creation
        Err(FsError::NotSupported)
    }

    fn rmdir(&self, _p: &str) -> Result<(), FsError> {
        let _guard = self.metadata_lock.write();
        // TODO: Implement directory removal
        Err(FsError::NotSupported)
    }

    fn stat(&self, p: &str) -> Result<FileStat, FsError> {
        Ok(Fat32Fs::stat(self, p)?)
    }
}

// ============================================================================
// Error Types
// ============================================================================

#[derive(Debug)]
pub enum Fat32Error {
    NotFound,
    IoError,
    ReadError,
    WriteError,
    InvalidPath,
    InvalidCluster,
    IsADirectory,
    NotADirectory,
    DiskFull,
}

impl From<Fat32Error> for crate::fs::FsError {
    fn from(err: Fat32Error) -> Self {
        match err {
            Fat32Error::NotFound => crate::fs::FsError::NotFound,
            Fat32Error::IoError | Fat32Error::ReadError | Fat32Error::WriteError => {
                crate::fs::FsError::IoError
            }
            Fat32Error::InvalidPath | Fat32Error::InvalidCluster => crate::fs::FsError::NotFound,
            Fat32Error::IsADirectory => crate::fs::FsError::IsADirectory,
            Fat32Error::NotADirectory => crate::fs::FsError::NotADirectory,
            Fat32Error::DiskFull => crate::fs::FsError::IoError,
        }
    }
}

// ============================================================================
// Internal Structures
// ============================================================================

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
