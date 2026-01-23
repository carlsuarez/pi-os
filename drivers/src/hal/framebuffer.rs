/// FrameBuffer trait for display devices
pub trait FrameBuffer: Send + Sync {
    /// Get the width of the framebuffer in pixels
    fn width(&self) -> usize;

    /// Get the height of the framebuffer in pixels
    fn height(&self) -> usize;

    /// Get the number of bytes per pixel
    fn bytes_per_pixel(&self) -> usize;

    /// Get the pitch (bytes per row, may include padding)
    fn pitch(&self) -> usize {
        self.width() * self.bytes_per_pixel()
    }

    /// Get the total size of the framebuffer in bytes
    fn size(&self) -> usize {
        self.pitch() * self.height()
    }

    /// Get a raw pointer to the framebuffer memory
    ///
    /// # Safety
    /// The caller must ensure proper synchronization when accessing this pointer
    fn buffer_ptr(&self) -> *mut u8;

    /// Get the pixel format (RGB, BGR, etc.)
    fn pixel_format(&self) -> PixelFormat {
        PixelFormat::Rgb
    }

    /// Clear the framebuffer to a solid color
    fn clear(&mut self, color: u32);

    /// Set a pixel at the given coordinates
    ///
    /// Returns `true` if successful, `false` if out of bounds
    fn set_pixel(&mut self, x: u32, y: u32, color: u32) -> bool;

    /// Get the color of a pixel at the given coordinates
    ///
    /// Returns `None` if out of bounds
    fn get_pixel(&self, x: u32, y: u32) -> Option<u32>;

    /// Draw a horizontal line
    fn draw_hline(&mut self, x1: u32, x2: u32, y: u32, color: u32);

    /// Draw a vertical line
    fn draw_vline(&mut self, x: u32, y1: u32, y2: u32, color: u32);

    /// Draw a filled rectangle
    fn draw_rect(&mut self, x: u32, y: u32, width: u32, height: u32, color: u32);

    /// Draw a line using Bresenham's algorithm (provided implementation)
    fn draw_line(&mut self, x1: u32, y1: u32, x2: u32, y2: u32, color: u32) {
        let mut x1 = x1 as i32;
        let mut y1 = y1 as i32;
        let x2 = x2 as i32;
        let y2 = y2 as i32;

        let dx = (x2 - x1).abs();
        let dy = -(y2 - y1).abs();
        let sx = if x1 < x2 { 1 } else { -1 };
        let sy = if y1 < y2 { 1 } else { -1 };
        let mut err = dx + dy;

        loop {
            self.set_pixel(x1 as u32, y1 as u32, color);

            if x1 == x2 && y1 == y2 {
                break;
            }

            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x1 += sx;
            }
            if e2 <= dx {
                err += dx;
                y1 += sy;
            }
        }
    }

    /// Draw a rectangle outline
    fn draw_rect_outline(&mut self, x: u32, y: u32, width: u32, height: u32, color: u32) {
        if width == 0 || height == 0 {
            return;
        }

        let x2 = x.saturating_add(width - 1).min(self.width() as u32 - 1);
        let y2 = y.saturating_add(height - 1).min(self.height() as u32 - 1);

        self.draw_hline(x, x2, y, color); // Top
        self.draw_hline(x, x2, y2, color); // Bottom
        self.draw_vline(x, y, y2, color); // Left
        self.draw_vline(x2, y, y2, color); // Right
    }

    /// Draw a circle using midpoint circle algorithm
    fn draw_circle(&mut self, cx: u32, cy: u32, radius: u32, color: u32) {
        let cx = cx as i32;
        let cy = cy as i32;
        let radius = radius as i32;

        let mut x = radius;
        let mut y = 0;
        let mut err = 0;

        while x >= y {
            // Draw 8 octants
            self.set_pixel((cx + x) as u32, (cy + y) as u32, color);
            self.set_pixel((cx + y) as u32, (cy + x) as u32, color);
            self.set_pixel((cx - y) as u32, (cy + x) as u32, color);
            self.set_pixel((cx - x) as u32, (cy + y) as u32, color);
            self.set_pixel((cx - x) as u32, (cy - y) as u32, color);
            self.set_pixel((cx - y) as u32, (cy - x) as u32, color);
            self.set_pixel((cx + y) as u32, (cy - x) as u32, color);
            self.set_pixel((cx + x) as u32, (cy - y) as u32, color);

            y += 1;
            if err <= 0 {
                err += 2 * y + 1;
            }
            if err > 0 {
                x -= 1;
                err -= 2 * x + 1;
            }
        }
    }

    /// Fill a circle
    fn fill_circle(&mut self, cx: u32, cy: u32, radius: u32, color: u32) {
        let cx = cx as i32;
        let cy = cy as i32;
        let radius = radius as i32;

        let mut x = radius;
        let mut y = 0;
        let mut err = 0;

        while x >= y {
            // Draw horizontal lines for each octant
            self.draw_hline((cx - x) as u32, (cx + x) as u32, (cy + y) as u32, color);
            self.draw_hline((cx - x) as u32, (cx + x) as u32, (cy - y) as u32, color);
            self.draw_hline((cx - y) as u32, (cx + y) as u32, (cy + x) as u32, color);
            self.draw_hline((cx - y) as u32, (cx + y) as u32, (cy - x) as u32, color);

            y += 1;
            if err <= 0 {
                err += 2 * y + 1;
            }
            if err > 0 {
                x -= 1;
                err -= 2 * x + 1;
            }
        }
    }

    /// Copy a region from one location to another (for scrolling, etc.)
    fn copy_region(
        &mut self,
        src_x: u32,
        src_y: u32,
        dst_x: u32,
        dst_y: u32,
        width: u32,
        height: u32,
    ) {
        // Naive implementation - can be optimized with memcpy
        for y in 0..height {
            for x in 0..width {
                if let Some(color) = self.get_pixel(src_x + x, src_y + y) {
                    self.set_pixel(dst_x + x, dst_y + y, color);
                }
            }
        }
    }
}

/// Pixel format enumeration
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PixelFormat {
    /// RGB (Red, Green, Blue)
    Rgb,
    /// BGR (Blue, Green, Red)
    Bgr,
    /// RGBA (Red, Green, Blue, Alpha)
    Rgba,
    /// BGRA (Blue, Green, Red, Alpha)
    Bgra,
}

/// FrameBuffer information structure
#[derive(Debug, Copy, Clone)]
pub struct FrameBufferInfo {
    pub width: usize,
    pub height: usize,
    pub pitch: usize,
    pub depth: usize,
    pub pixel_format: PixelFormat,
    pub address: usize,
    pub size: usize,
}

/// FrameBuffer configuration for initialization
#[derive(Debug, Copy, Clone)]
pub struct FrameBufferConfig {
    /// Physical width in pixels
    pub width: u32,
    /// Physical height in pixels
    pub height: u32,
    /// Virtual width in pixels (for scrolling)
    pub virtual_width: u32,
    /// Virtual height in pixels (for double buffering)
    pub virtual_height: u32,
    /// Bits per pixel (16, 24, or 32)
    pub depth: u32,
}

impl Default for FrameBufferConfig {
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

/// FrameBuffer errors
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum FrameBufferError {
    /// Mailbox call failed
    MailboxFailed,
    /// GPU failed to allocate framebuffer
    AllocationFailed,
    /// Invalid configuration
    InvalidConfig,
    /// Not supported
    NotSupported,
}

impl core::fmt::Display for FrameBufferError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            FrameBufferError::MailboxFailed => write!(f, "Mailbox call failed"),
            FrameBufferError::AllocationFailed => write!(f, "GPU failed to allocate framebuffer"),
            FrameBufferError::InvalidConfig => write!(f, "Invalid configuration"),
            FrameBufferError::NotSupported => write!(f, "Operation not supported"),
        }
    }
}

/// Color utility functions
pub mod color {
    /// Create an ARGB color from components
    pub const fn argb(a: u8, r: u8, g: u8, b: u8) -> u32 {
        ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
    }

    /// Create an RGB color (alpha = 255)
    pub const fn rgb(r: u8, g: u8, b: u8) -> u32 {
        argb(255, r, g, b)
    }

    /// Extract color components (alpha, red, green, blue)
    pub const fn components(color: u32) -> (u8, u8, u8, u8) {
        (
            ((color >> 24) & 0xFF) as u8, // Alpha
            ((color >> 16) & 0xFF) as u8, // Red
            ((color >> 8) & 0xFF) as u8,  // Green
            (color & 0xFF) as u8,         // Blue
        )
    }

    /// Common colors
    pub const BLACK: u32 = rgb(0, 0, 0);
    pub const WHITE: u32 = rgb(255, 255, 255);
    pub const RED: u32 = rgb(255, 0, 0);
    pub const GREEN: u32 = rgb(0, 255, 0);
    pub const BLUE: u32 = rgb(0, 0, 255);
    pub const YELLOW: u32 = rgb(255, 255, 0);
    pub const CYAN: u32 = rgb(0, 255, 255);
    pub const MAGENTA: u32 = rgb(255, 0, 255);
    pub const GRAY: u32 = rgb(128, 128, 128);
    pub const DARK_GRAY: u32 = rgb(64, 64, 64);
    pub const LIGHT_GRAY: u32 = rgb(192, 192, 192);
    pub const ORANGE: u32 = rgb(255, 165, 0);
    pub const PURPLE: u32 = rgb(128, 0, 128);
    pub const BROWN: u32 = rgb(165, 42, 42);
}
