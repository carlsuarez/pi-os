use crate::fs::file::File;

use super::FileSystem;
use super::FsError;
use super::dev::DevFs;
use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec;

pub struct Mount {
    pub prefix: &'static str,
    pub fs: &'static dyn FileSystem,
}

static mut MOUNTS: Option<&[Mount]> = None;

static VFS: VirtFS = VirtFS::new();

struct VirtFS;

impl VirtFS {
    pub const fn new() -> Self {
        Self
    }

    pub fn init(devfs: &'static DevFs) {
        unsafe {
            let mounts = vec![Mount {
                prefix: "/dev",
                fs: devfs,
            }];
            MOUNTS = Some(Box::leak(mounts.into_boxed_slice()));
        }
    }

    fn dispatch<T, F>(&self, path: &str, f: F) -> Result<T, FsError>
    where
        F: Fn(&Mount, &str) -> Result<T, FsError>,
    {
        let mounts = unsafe { MOUNTS.expect("vfs not initialized\n") };

        for mount in mounts {
            if let Some(rest) = path.strip_prefix(mount.prefix) {
                let rest = rest.strip_prefix('/').unwrap_or(rest);
                return f(mount, rest);
            }
        }

        Err(FsError::NotFound)
    }
}

impl FileSystem for VirtFS {
    fn open(&self, path: &str) -> Result<Arc<dyn File>, FsError> {
        self.dispatch(path, |mount, rest| mount.fs.open(rest))
    }

    fn create(&self, path: &str) -> Result<Arc<dyn File>, FsError> {
        self.dispatch(path, |mount, rest| mount.fs.create(rest))
    }

    fn delete(&self, path: &str) -> Result<(), FsError> {
        self.dispatch(path, |mount, rest| mount.fs.delete(rest))
    }

    fn ls(&self, path: &str) -> Result<vec::Vec<alloc::string::String>, FsError> {
        self.dispatch(path, |mount, rest| mount.fs.ls(rest))
    }

    fn mkdir(&self, path: &str) -> Result<(), FsError> {
        self.dispatch(path, |mount, rest| mount.fs.mkdir(rest))
    }

    fn mount(&self) -> Result<(), FsError> {
        Ok(())
    }

    fn rmdir(&self, path: &str) -> Result<(), FsError> {
        self.dispatch(path, |mount, rest| mount.fs.rmdir(rest))
    }

    fn stat(&self, path: &str) -> Result<super::file::FileStat, FsError> {
        self.dispatch(path, |mount, rest| mount.fs.stat(rest))
    }
}
