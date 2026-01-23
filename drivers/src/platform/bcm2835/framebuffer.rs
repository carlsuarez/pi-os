use super::mailbox::{Channel, Mailbox, tags};
use crate::hal::framebuffer::{
    FrameBuffer, FrameBufferConfig, FrameBufferError, FrameBufferInfo, PixelFormat,
};
use core::ptr::{read_volatile, write_volatile};
use core::slice;

/// BCM2835 framebuffer implementation
pub struct Bcm2835Framebuffer {
    info: FrameBufferInfo,
    buffer: &'static mut [u32],
    pixel_format: PixelFormat,
}

impl Bcm2835Framebuffer {
    /// Initialize a new framebuffer with the given configuration
    ///
    /// # Safety
    /// - Mailbox must be accessible
    /// - Identity mapping required for framebuffer memory
    pub unsafe fn new(config: FrameBufferConfig) -> Result<Self, FrameBufferError> {
        // Request buffer must be 16-byte aligned
        #[repr(C, align(16))]
        struct FbRequest {
            size: u32,
            code: u32,

            // Physical size
            tag_phys: u32,
            val_buf_size_phys: u32,
            val_len_phys: u32,
            width_phys: u32,
            height_phys: u32,

            // Virtual size
            tag_virt: u32,
            val_buf_size_virt: u32,
            val_len_virt: u32,
            width_virt: u32,
            height_virt: u32,

            // Depth
            tag_depth: u32,
            val_buf_size_depth: u32,
            val_len_depth: u32,
            depth: u32,

            // Pixel order
            tag_pixel_order: u32,
            val_buf_size_pixel_order: u32,
            val_len_pixel_order: u32,
            pixel_order: u32,

            // Allocate buffer
            tag_alloc: u32,
            val_buf_size_alloc: u32,
            val_len_alloc: u32,
            fb_addr: u32,
            fb_size: u32,

            // Pitch
            tag_pitch: u32,
            val_buf_size_pitch: u32,
            val_len_pitch: u32,
            pitch: u32,

            end: u32,
        }

        let mut req = FbRequest {
            size: core::mem::size_of::<FbRequest>() as u32,
            code: 0,

            tag_phys: tags::SET_PHYSICAL_SIZE,
            val_buf_size_phys: 8,
            val_len_phys: 0,
            width_phys: 0,
            height_phys: 0,

            tag_virt: tags::SET_VIRTUAL_SIZE,
            val_buf_size_virt: 8,
            val_len_virt: 0,
            width_virt: 0,
            height_virt: 0,

            tag_depth: tags::SET_DEPTH,
            val_buf_size_depth: 4,
            val_len_depth: 0,
            depth: 0,

            tag_pixel_order: tags::SET_PIXEL_ORDER,
            val_buf_size_pixel_order: 4,
            val_len_pixel_order: 0,
            pixel_order: 1, // RGB

            tag_alloc: tags::ALLOCATE_BUFFER,
            val_buf_size_alloc: 8,
            val_len_alloc: 0,
            fb_addr: 0,
            fb_size: 0,

            tag_pitch: tags::GET_PITCH,
            val_buf_size_pitch: 4,
            val_len_pitch: 0,
            pitch: 0,

            end: 0,
        };

        // Fill in configuration
        unsafe {
            write_volatile(&mut req.width_phys, config.width);
            write_volatile(&mut req.height_phys, config.height);
            write_volatile(&mut req.width_virt, config.virtual_width);
            write_volatile(&mut req.height_virt, config.virtual_height);
            write_volatile(&mut req.depth, config.depth);
        }

        // Make mailbox call
        let mut mailbox = unsafe { Mailbox::new() };
        let req_phys = &raw const req as usize;

        if !unsafe { mailbox.call(Channel::Property, req_phys) } {
            return Err(FrameBufferError::MailboxFailed);
        }

        // Read response
        let fb_addr = unsafe { read_volatile(&req.fb_addr) };
        let fb_size = unsafe { read_volatile(&req.fb_size) };
        let pitch = unsafe { read_volatile(&req.pitch) };
        let pixel_order = unsafe { read_volatile(&req.pixel_order) };

        if fb_addr == 0 || fb_size == 0 {
            return Err(FrameBufferError::AllocationFailed);
        }

        // Convert GPU address to ARM physical address
        let fb_addr = (fb_addr & 0x3FFF_FFFF) as usize;

        let pixel_format = if pixel_order == 0 {
            PixelFormat::Bgr
        } else {
            PixelFormat::Rgb
        };

        let info = FrameBufferInfo {
            width: config.width as usize,
            height: config.height as usize,
            pitch: pitch as usize,
            depth: config.depth as usize,
            pixel_format,
            address: fb_addr,
            size: fb_size as usize,
        };

        // Create slice to framebuffer memory
        let buffer =
            unsafe { slice::from_raw_parts_mut(fb_addr as *mut u32, fb_size as usize / 4) };

        Ok(Self {
            info,
            buffer,
            pixel_format,
        })
    }

    /// Get framebuffer information
    pub fn info(&self) -> &FrameBufferInfo {
        &self.info
    }

    /// Get raw framebuffer slice (read-only)
    pub fn buffer(&self) -> &[u32] {
        self.buffer
    }

    /// Get raw mutable framebuffer slice
    pub fn buffer_mut(&mut self) -> &mut [u32] {
        self.buffer
    }

    /// Calculate pixel offset from coordinates
    #[inline]
    fn pixel_offset(&self, x: u32, y: u32) -> Option<usize> {
        if x >= self.info.width as u32 || y >= self.info.height as u32 {
            return None;
        }

        let offset = (y * (self.info.pitch as u32 / 4) + x) as usize;
        if offset < self.buffer.len() {
            Some(offset)
        } else {
            None
        }
    }
}

impl FrameBuffer for Bcm2835Framebuffer {
    fn width(&self) -> usize {
        self.info.width
    }

    fn height(&self) -> usize {
        self.info.height
    }

    fn bytes_per_pixel(&self) -> usize {
        self.info.depth / 8
    }

    fn pitch(&self) -> usize {
        self.info.pitch
    }

    fn buffer_ptr(&self) -> *mut u8 {
        self.info.address as *mut u8
    }

    fn pixel_format(&self) -> PixelFormat {
        self.pixel_format
    }

    fn clear(&mut self, color: u32) {
        self.buffer.fill(color);
    }

    fn set_pixel(&mut self, x: u32, y: u32, color: u32) -> bool {
        if let Some(offset) = self.pixel_offset(x, y) {
            self.buffer[offset] = color;
            true
        } else {
            false
        }
    }

    fn get_pixel(&self, x: u32, y: u32) -> Option<u32> {
        self.pixel_offset(x, y).map(|offset| self.buffer[offset])
    }

    fn draw_hline(&mut self, x1: u32, x2: u32, y: u32, color: u32) {
        if y >= self.info.height as u32 {
            return;
        }

        let x1 = x1.min(self.info.width as u32 - 1);
        let x2 = x2.min(self.info.width as u32 - 1);
        let (x1, x2) = if x1 <= x2 { (x1, x2) } else { (x2, x1) };

        if let Some(start_offset) = self.pixel_offset(x1, y) {
            let len = (x2 - x1 + 1) as usize;
            self.buffer[start_offset..start_offset + len].fill(color);
        }
    }

    fn draw_vline(&mut self, x: u32, y1: u32, y2: u32, color: u32) {
        if x >= self.info.width as u32 {
            return;
        }

        let y1 = y1.min(self.info.height as u32 - 1);
        let y2 = y2.min(self.info.height as u32 - 1);
        let (y1, y2) = if y1 <= y2 { (y1, y2) } else { (y2, y1) };

        for y in y1..=y2 {
            self.set_pixel(x, y, color);
        }
    }

    fn draw_rect(&mut self, x: u32, y: u32, width: u32, height: u32, color: u32) {
        let x2 = x.saturating_add(width).min(self.info.width as u32);
        let y2 = y.saturating_add(height).min(self.info.height as u32);

        for py in y..y2 {
            self.draw_hline(x, x2 - 1, py, color);
        }
    }
}
