//! BCM2835 Framebuffer Driver
//!
//! This driver provides access to the display framebuffer through
//! the mailbox interface. It can be used to:
//! - Initialize a framebuffer
//! - Configure display resolution and depth
//! - Get framebuffer memory address
//! - Draw to the screen
//!
//! # Example
//!
//! ```no_run
//! use drivers::platform::bcm2835::framebuffer::{Framebuffer, FramebufferConfig};
//!
//! unsafe {
//!     let config = FramebufferConfig {
//!         width: 1920,
//!         height: 1080,
//!         virtual_width: 1920,
//!         virtual_height: 1080,
//!         depth: 32,
//!     };
//!     
//!     let mut fb = Framebuffer::new(config).unwrap();
//!     
//!     // Clear screen to red
//!     fb.clear(0xFFFF0000);
//!     
//!     // Draw a pixel
//!     fb.set_pixel(100, 100, 0xFFFFFFFF);
//! }
//! ```

use super::mailbox::{Channel, Mailbox, tags};
use core::ptr::{read_volatile, write_volatile};
use core::slice;

/// Framebuffer configuration.
#[derive(Debug, Copy, Clone)]
pub struct FramebufferConfig {
    /// Physical width in pixels.
    pub width: u32,
    /// Physical height in pixels.
    pub height: u32,
    /// Virtual width in pixels (for scrolling).
    pub virtual_width: u32,
    /// Virtual height in pixels (for double buffering).
    pub virtual_height: u32,
    /// Bits per pixel (16, 24, or 32).
    pub depth: u32,
}

impl Default for FramebufferConfig {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            virtual_width: 1920,
            virtual_height: 1080,
            depth: 32,
        }
    }
}

/// Pixel format.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PixelOrder {
    /// BGR (Blue, Green, Red).
    Bgr = 0,
    /// RGB (Red, Green, Blue).
    Rgb = 1,
}

/// Framebuffer information returned by GPU.
#[derive(Debug, Copy, Clone)]
pub struct FramebufferInfo {
    /// Physical width in pixels.
    pub width: u32,
    /// Physical height in pixels.
    pub height: u32,
    /// Virtual width in pixels.
    pub virtual_width: u32,
    /// Virtual height in pixels.
    pub virtual_height: u32,
    /// Bytes per row.
    pub pitch: u32,
    /// Bits per pixel.
    pub depth: u32,
    /// Pixel order (RGB or BGR).
    pub pixel_order: PixelOrder,
    /// Framebuffer physical address.
    pub address: usize,
    /// Framebuffer size in bytes.
    pub size: usize,
}

/// BCM2835 framebuffer.
pub struct Framebuffer {
    info: FramebufferInfo,
    buffer: &'static mut [u32],
}

impl Framebuffer {
    /// Initialize a new framebuffer.
    ///
    /// # Safety
    ///
    /// - Mailbox must be accessible
    /// - Identity mapping required
    pub unsafe fn new(config: FramebufferConfig) -> Result<Self, FramebufferError> {
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
            return Err(FramebufferError::MailboxFailed);
        }

        // Read response
        let fb_addr = unsafe { read_volatile(&req.fb_addr) };
        let fb_size = unsafe { read_volatile(&req.fb_size) };
        let pitch = unsafe { read_volatile(&req.pitch) };
        let pixel_order = unsafe { read_volatile(&req.pixel_order) };

        if fb_addr == 0 || fb_size == 0 {
            return Err(FramebufferError::AllocationFailed);
        }

        // Convert framebuffer address (GPU uses bus addresses)
        // The GPU address needs to be converted to ARM physical address
        // by clearing the top bits
        let fb_addr = (fb_addr & 0x3FFF_FFFF) as usize;

        let info = FramebufferInfo {
            width: config.width,
            height: config.height,
            virtual_width: config.virtual_width,
            virtual_height: config.virtual_height,
            pitch,
            depth: config.depth,
            pixel_order: if pixel_order == 0 {
                PixelOrder::Bgr
            } else {
                PixelOrder::Rgb
            },
            address: fb_addr,
            size: fb_size as usize,
        };

        // Create slice to framebuffer memory
        let buffer =
            unsafe { slice::from_raw_parts_mut(fb_addr as *mut u32, fb_size as usize / 4) };

        Ok(Self { info, buffer })
    }

    /// Get framebuffer information.
    pub fn info(&self) -> &FramebufferInfo {
        &self.info
    }

    /// Get the raw framebuffer slice.
    pub fn buffer(&self) -> &[u32] {
        self.buffer
    }

    /// Get the raw mutable framebuffer slice.
    pub fn buffer_mut(&mut self) -> &mut [u32] {
        self.buffer
    }

    /// Clear the framebuffer to a solid color.
    ///
    /// # Arguments
    ///
    /// - `color`: 32-bit ARGB color value
    pub fn clear(&mut self, color: u32) {
        for pixel in self.buffer.iter_mut() {
            *pixel = color;
        }
    }

    /// Set a pixel at the given coordinates.
    ///
    /// # Arguments
    ///
    /// - `x`: X coordinate
    /// - `y`: Y coordinate
    /// - `color`: 32-bit ARGB color value
    ///
    /// # Returns
    ///
    /// `true` if the pixel was set, `false` if out of bounds.
    pub fn set_pixel(&mut self, x: u32, y: u32, color: u32) -> bool {
        if x >= self.info.width || y >= self.info.height {
            return false;
        }

        let offset = (y * (self.info.pitch / 4) + x) as usize;
        if offset < self.buffer.len() {
            self.buffer[offset] = color;
            true
        } else {
            false
        }
    }

    /// Get the color of a pixel at the given coordinates.
    ///
    /// # Arguments
    ///
    /// - `x`: X coordinate
    /// - `y`: Y coordinate
    ///
    /// # Returns
    ///
    /// The pixel color, or `None` if out of bounds.
    pub fn get_pixel(&self, x: u32, y: u32) -> Option<u32> {
        if x >= self.info.width || y >= self.info.height {
            return None;
        }

        let offset = (y * (self.info.pitch / 4) + x) as usize;
        self.buffer.get(offset).copied()
    }

    /// Draw a horizontal line.
    pub fn draw_hline(&mut self, x1: u32, x2: u32, y: u32, color: u32) {
        let x1 = x1.min(self.info.width - 1);
        let x2 = x2.min(self.info.width - 1);

        if y >= self.info.height {
            return;
        }

        for x in x1..=x2 {
            self.set_pixel(x, y, color);
        }
    }

    /// Draw a vertical line.
    pub fn draw_vline(&mut self, x: u32, y1: u32, y2: u32, color: u32) {
        let y1 = y1.min(self.info.height - 1);
        let y2 = y2.min(self.info.height - 1);

        if x >= self.info.width {
            return;
        }

        for y in y1..=y2 {
            self.set_pixel(x, y, color);
        }
    }

    /// Draw a filled rectangle.
    pub fn draw_rect(&mut self, x: u32, y: u32, width: u32, height: u32, color: u32) {
        let x2 = (x + width).min(self.info.width);
        let y2 = (y + height).min(self.info.height);

        for py in y..y2 {
            for px in x..x2 {
                self.set_pixel(px, py, color);
            }
        }
    }
}

/// Framebuffer errors.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum FramebufferError {
    /// Mailbox call failed.
    MailboxFailed,
    /// GPU failed to allocate framebuffer.
    AllocationFailed,
    /// Invalid configuration.
    InvalidConfig,
}

// ============================================================================
// Color Utilities
// ============================================================================

/// Color utility functions.
pub mod color {
    /// Create an ARGB color from components.
    pub const fn argb(a: u8, r: u8, g: u8, b: u8) -> u32 {
        ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
    }

    /// Create an RGB color (alpha = 255).
    pub const fn rgb(r: u8, g: u8, b: u8) -> u32 {
        argb(255, r, g, b)
    }

    /// Common colors.
    pub const BLACK: u32 = rgb(0, 0, 0);
    pub const WHITE: u32 = rgb(255, 255, 255);
    pub const RED: u32 = rgb(255, 0, 0);
    pub const GREEN: u32 = rgb(0, 255, 0);
    pub const BLUE: u32 = rgb(0, 0, 255);
    pub const YELLOW: u32 = rgb(255, 255, 0);
    pub const CYAN: u32 = rgb(0, 255, 255);
    pub const MAGENTA: u32 = rgb(255, 0, 255);
}
