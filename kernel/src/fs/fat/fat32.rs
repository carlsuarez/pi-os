use crate::fs::FsError;
use crate::fs::fd::FdError;
use crate::fs::{BlockDevice, FileSystem};
use crate::fs::{File, file::FileStat};
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::AtomicUsize;

pub struct Fat32Fs {
    dev: Arc<dyn BlockDevice>,
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

pub struct Fat32File {
    fs: Arc<Fat32Fs>,
    first_cluster: u32,
    size: AtomicUsize,
    is_dir: bool,
}

impl Fat32File {
    pub const fn new(
        fs: Arc<Fat32Fs>,
        first_cluster: u32,
        size: AtomicUsize,
        is_dir: bool,
    ) -> Self {
        Self {
            fs,
            first_cluster,
            size,
            is_dir,
        }
    }
}

impl File for Fat32File {
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

impl FileSystem for Fat32Fs {
    fn open(&self, _path: &str) -> Result<Arc<dyn File>, FsError> {
        Ok(Arc::new(Fat32File::new(
            Arc::new(Self {
                dev: self.dev.clone(),
                fat_info: self.fat_info,
            }),
            1,
            AtomicUsize::new(1),
            false,
        )))
    }

    fn create(&self, _path: &str) -> Result<Arc<dyn File>, FsError> {
        Ok(Arc::new(Fat32File::new(
            Arc::new(Self {
                dev: self.dev.clone(),
                fat_info: self.fat_info,
            }),
            1,
            AtomicUsize::new(1),
            false,
        )))
    }

    fn mkdir(&self, _path: &str) -> Result<(), FsError> {
        Ok(())
    }

    fn rmdir(&self, _path: &str) -> Result<(), FsError> {
        Ok(())
    }

    fn delete(&self, _path: &str) -> Result<(), FsError> {
        Ok(())
    }

    fn ls(&self, _path: &str) -> Result<Vec<String>, FsError> {
        Ok(alloc::vec![String::from("Not yet implemented")])
    }

    fn mount(&self) -> Result<(), FsError> {
        Ok(())
    }

    fn stat(&self, _path: &str) -> Result<FileStat, FsError> {
        Ok(FileStat {
            size: 1,
            is_dir: false,
        })
    }
}
