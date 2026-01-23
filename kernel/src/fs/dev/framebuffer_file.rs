use super::super::file::{File, FileStat};
use crate::fs::fd::FdError;
use crate::fs::file::FileType;
use alloc::format;
use alloc::string::String;
use drivers::device_manager::devices;
use drivers::hal::framebuffer::FrameBuffer;

/// File wrapper around a framebuffer device
pub struct FrameBufferFile {
    index: usize,

    // Cached info
    width: usize,
    height: usize,
    pitch: usize,
    bpp: usize,
}

impl FrameBufferFile {
    pub fn new(index: usize) -> Result<Self, FdError> {
        let name = format!("fb{}", index);

        let fb = devices()
            .lock()
            .framebuffer(&name)
            .ok_or(FdError::Other("No such device".into()))?;

        let fb = fb.lock();

        Ok(Self {
            index,
            width: fb.width(),
            height: fb.height(),
            pitch: fb.pitch(),
            bpp: fb.bytes_per_pixel(),
        })
    }

    #[inline]
    fn device_name(&self) -> String {
        format!("fb{}", self.index)
    }

    #[inline]
    fn size(&self) -> usize {
        self.pitch * self.height
    }

    fn validate_offset(&self, offset: usize) -> Result<(), FdError> {
        if offset > self.size() {
            Err(FdError::InvalidSeek)
        } else {
            Ok(())
        }
    }
}

impl File for FrameBufferFile {
    fn read(&self, buf: &mut [u8], offset: usize) -> Result<usize, FdError> {
        self.validate_offset(offset)?;

        let fb = devices()
            .lock()
            .framebuffer(&self.device_name())
            .ok_or(FdError::Other("No such device".into()))?;

        let fb = fb.lock();

        let available = self.size().saturating_sub(offset);
        let to_read = buf.len().min(available);
        if to_read == 0 {
            return Ok(0);
        }

        unsafe {
            let src = fb.buffer_ptr().add(offset);
            core::ptr::copy_nonoverlapping(src, buf.as_mut_ptr(), to_read);
        }

        Ok(to_read)
    }

    fn write(&self, buf: &[u8], offset: usize) -> Result<usize, FdError> {
        self.validate_offset(offset)?;

        let fb = devices()
            .lock()
            .framebuffer(&self.device_name())
            .ok_or(FdError::Other("No such device".into()))?;

        let fb = fb.lock();

        let available = self.size().saturating_sub(offset);
        let to_write = buf.len().min(available);
        if to_write == 0 {
            return Ok(0);
        }

        unsafe {
            let dst = fb.buffer_ptr().add(offset);
            core::ptr::copy_nonoverlapping(buf.as_ptr(), dst, to_write);
        }

        Ok(to_write)
    }

    fn stat(&self) -> Result<FileStat, FdError> {
        Ok(FileStat {
            size: self.size(),
            file_type: FileType::CharDevice,
            name: self.device_name(),
        })
    }
}
