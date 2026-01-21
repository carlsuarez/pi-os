use crate::fs::file::{File, FileStat};
use crate::fs::{FileSystem, FsError};

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use common::sync::SpinLock;

/// A mount point in the VFS.
pub struct Mount {
    pub prefix: String,
    pub fs: Arc<dyn FileSystem>,
}

static VFS: VirtFS = VirtFS::new();

pub struct VirtFS {
    mounts: SpinLock<Vec<Mount>>,
}

impl VirtFS {
    pub const fn new() -> Self {
        Self {
            mounts: SpinLock::new(Vec::new()),
        }
    }

    /// Initialize with a root filesystem.
    pub fn init(&'static self, rootfs: Arc<dyn FileSystem>) {
        let mut mounts = self.mounts.lock();
        mounts.clear();
        mounts.push(Mount {
            prefix: "/".into(),
            fs: rootfs,
        });
    }

    /// Mount a filesystem at a path.
    pub fn mount_fs(&self, prefix: &str, fs: Arc<dyn FileSystem>) -> Result<(), FsError> {
        let mut mounts = self.mounts.lock();

        if mounts.iter().any(|m| m.prefix == prefix) {
            return Err(FsError::AlreadyExists);
        }

        mounts.push(Mount {
            prefix: prefix.into(),
            fs,
        });

        Ok(())
    }

    /// Unmount a filesystem.
    pub fn umount(&self, prefix: &str) -> Result<(), FsError> {
        let mut mounts = self.mounts.lock();

        let idx = mounts
            .iter()
            .position(|m| m.prefix == prefix)
            .ok_or(FsError::NotFound)?;

        mounts.remove(idx);
        Ok(())
    }

    /// Dispatch a path to the filesystem with the longest matching mount prefix.
    fn dispatch<T, F>(&self, path: &str, f: F) -> Result<T, FsError>
    where
        F: Fn(&Mount, &str) -> Result<T, FsError>,
    {
        let mounts = self.mounts.lock();

        let mut best: Option<(&Mount, &str)> = None;

        for mount in mounts.iter() {
            if let Some(rest) = path.strip_prefix(&mount.prefix) {
                let rest = rest.strip_prefix('/').unwrap_or(rest);

                match best {
                    None => best = Some((mount, rest)),
                    Some((prev, _)) if mount.prefix.len() > prev.prefix.len() => {
                        best = Some((mount, rest))
                    }
                    _ => {}
                }
            }
        }

        let (mount, rest) = best.ok_or(FsError::NotFound)?;
        f(mount, rest)
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

    fn ls(&self, path: &str) -> Result<Vec<String>, FsError> {
        self.dispatch(path, |mount, rest| mount.fs.ls(rest))
    }

    fn mkdir(&self, path: &str) -> Result<(), FsError> {
        self.dispatch(path, |mount, rest| mount.fs.mkdir(rest))
    }

    fn rmdir(&self, path: &str) -> Result<(), FsError> {
        self.dispatch(path, |mount, rest| mount.fs.rmdir(rest))
    }

    fn stat(&self, path: &str) -> Result<FileStat, FsError> {
        self.dispatch(path, |mount, rest| mount.fs.stat(rest))
    }
}

/// Public VFS entry point
pub fn vfs() -> &'static VirtFS {
    &VFS
}
