use super::fd::FdError;

/// Block device abstraction
pub trait BlockDevice: Send + Sync {
    /// Read a single sector into `buf`
    fn read_sector(&self, lba: u32, buf: &mut [u8]) -> Result<(), FdError>;

    /// Write a single sector from `buf`
    fn write_sector(&self, lba: u32, buf: &[u8]) -> Result<(), FdError>;
}
