//
// Multiboot2 linear framebuffer driver for pi-os x86.
//
// The bootloader (GRUB/multiboot2) sets up a linear framebuffer and
// describes it in the framebuffer tag (type 8) of the multiboot2 info
// structure.  This driver takes the tag fields directly and provides a
// safe(ish) `FrameBuffer` implementation on top of volatile byte writes.
//
// Supported pixel depths: 15, 16, 24, 32 bpp.
// Supported types:        1 = RGB direct-colour  (the common case)
//                         2 = EGA text mode       (not usable as a pixel FB)
//                         0 = indexed             (palette; not implemented)
//
// Layout of the multiboot2 framebuffer tag that you pass to `new`:
//
//   u64  framebuffer_addr    – physical base address
//   u32  framebuffer_pitch   – bytes per row (may include padding)
//   u32  framebuffer_width   – pixels per row
//   u32  framebuffer_height  – rows
//   u8   framebuffer_bpp     – bits per pixel
//   u8   framebuffer_type    – 0=indexed, 1=RGB, 2=EGA text
//   u8   _reserved
//   -- type-1 colour info --
//   u8   red_field_position, red_mask_size
//   u8   green_field_position, green_mask_size
//   u8   blue_field_position, blue_mask_size
//
// Only RGB (type 1) is wired up; calling `new` with another type
// returns `Err(FrameBufferError::NotSupported)`.

use spin::Once;

use crate::hal::fb::{FrameBuffer, FrameBufferError, FrameBufferInfo, PixelFormat};

// Raw multiboot2 tag fields

/// Colour-channel descriptor as encoded in the multiboot2 RGB info block.
#[derive(Clone, Copy, Debug)]
pub struct ChannelDesc {
    pub field_pos: u8, // LSB position of the channel within a pixel word
    pub mask_size: u8, // Number of bits used for this channel
}

/// All the information pi-os needs from the multiboot2 framebuffer tag.
/// Populate this from the tag fields before calling `Mb2Fb::new`.
#[derive(Clone, Copy, Debug)]
pub struct Mb2FbTag {
    /// Physical base address of the framebuffer.
    pub addr: u64,
    /// Bytes per scanline (pitch ≥ width * bytes_per_pixel).
    pub pitch: u32,
    pub width: u32,
    pub height: u32,
    /// Bits per pixel (typically 16, 24, or 32).
    pub bpp: u8,
    /// 1 = RGB direct-colour.  Other values are rejected by `Mb2Fb::new`.
    pub fb_type: u8,
    // RGB channel layout (only valid when fb_type == 1)
    pub red: ChannelDesc,
    pub green: ChannelDesc,
    pub blue: ChannelDesc,
}

// Internal pixel-packing helpers

/// Describes how to pack/unpack one colour channel.
#[derive(Clone, Copy, Debug)]
struct Channel {
    shift: u8, // bit offset inside the pixel word
    mask: u32, // pre-shifted mask (mask_size ones at position `shift`)
}

impl Channel {
    const fn from_desc(d: ChannelDesc) -> Self {
        let mask = ((1u32 << d.mask_size) - 1) << d.field_pos;
        Self {
            shift: d.field_pos,
            mask,
        }
    }

    /// Extract an 8-bit component from a packed pixel word.
    #[inline]
    fn extract(self, pixel: u32) -> u8 {
        let raw = (pixel & self.mask) >> self.shift;
        // Scale from mask_size bits → 8 bits
        let bits = self.mask.count_ones() as u8;
        if bits >= 8 {
            (raw >> (bits - 8)) as u8
        } else {
            (raw << (8 - bits)) as u8
        }
    }

    /// Pack an 8-bit component into a pixel word.
    #[inline]
    fn pack(self, val: u8) -> u32 {
        let bits = self.mask.count_ones() as u8;
        let scaled = if bits >= 8 {
            (val as u32) << (bits - 8)
        } else {
            (val as u32) >> (8 - bits)
        };
        (scaled << self.shift) & self.mask
    }
}

// Driver struct

/// Linear framebuffer driver backed by a multiboot2 tag.
///
/// All pixel writes use `write_volatile` so the compiler never elides them.
pub struct Mb2Fb {
    base: *mut u8,
    width: u32,
    height: u32,
    pitch: u32,             // bytes per row
    bpp: u8,                // bits per pixel
    bytes_per_pixel: usize, // bytes per pixel (ceil)
    r: Channel,
    g: Channel,
    b: Channel,
    pixel_format: PixelFormat,
}

// SAFETY: Mb2Fb holds a raw pointer to MMIO/boot-mapped memory.
// Like VgaBuffer, we guarantee exclusive access through the Mutex
// that wraps it in the console/display subsystem.
unsafe impl Send for Mb2Fb {}
unsafe impl Sync for Mb2Fb {}

impl Mb2Fb {
    /// Construct the driver from a multiboot2 framebuffer tag.
    ///
    /// Returns `Err(FrameBufferError::NotSupported)` if the tag type is not 1
    /// (RGB direct-colour), or `Err(FrameBufferError::InvalidConfig)` if the
    /// bit depth is not 15, 16, 24, or 32.
    ///
    /// # Safety
    /// * `tag.addr` must be the true physical (or identity-mapped virtual)
    ///   base of the framebuffer as provided by GRUB/multiboot2.
    /// * Must be called at most once per framebuffer region.
    /// * The caller must ensure no other code aliases this memory region
    ///   without synchronisation.
    pub unsafe fn new(tag: Mb2FbTag) -> Result<Self, FrameBufferError> {
        if tag.fb_type != 1 {
            return Err(FrameBufferError::NotSupported);
        }

        let bytes_per_pixel = match tag.bpp {
            15 | 16 => 2,
            24 => 3,
            32 => 4,
            _ => return Err(FrameBufferError::InvalidConfig),
        };

        // Detect the pixel format from the channel positions so that callers
        // (e.g. a blitter) can choose the fastest path.
        let pixel_format = detect_pixel_format(&tag, bytes_per_pixel);

        Ok(Self {
            base: tag.addr as usize as *mut u8,
            width: tag.width,
            height: tag.height,
            pitch: tag.pitch,
            bpp: tag.bpp,
            bytes_per_pixel,
            r: Channel::from_desc(tag.red),
            g: Channel::from_desc(tag.green),
            b: Channel::from_desc(tag.blue),
            pixel_format,
        })
    }

    /// Return a `FrameBufferInfo` snapshot (useful for logging / subsystem
    /// registration).
    pub fn info(&self) -> FrameBufferInfo {
        FrameBufferInfo {
            width: self.width as usize,
            height: self.height as usize,
            pitch: self.pitch as usize,
            depth: self.bpp as usize,
            pixel_format: self.pixel_format,
            address: self.base as usize,
            size: (self.pitch * self.height) as usize,
        }
    }

    // Low-level volatile I/O

    /// Byte offset of pixel `(x, y)` in the flat buffer.
    #[inline(always)]
    fn offset(&self, x: u32, y: u32) -> usize {
        (y as usize) * (self.pitch as usize) + (x as usize) * self.bytes_per_pixel
    }

    /// Write a pixel word at `(x, y)` using volatile stores.
    ///
    /// `color` is an ARGB u32 (alpha ignored); the method packs it according
    /// to the channel layout detected at construction time.
    #[inline]
    fn write_pixel_raw(&mut self, x: u32, y: u32, packed: u32) {
        let off = self.offset(x, y);
        // Write only the bytes that belong to this pixel (handles 15/16/24/32).
        unsafe {
            match self.bytes_per_pixel {
                2 => {
                    let bytes = (packed as u16).to_le_bytes();
                    core::ptr::write_volatile(self.base.add(off) as *mut u16, packed as u16);
                    let _ = bytes; // suppress warning
                }
                3 => {
                    let bytes = packed.to_le_bytes();
                    core::ptr::write_volatile(self.base.add(off), bytes[0]);
                    core::ptr::write_volatile(self.base.add(off + 1), bytes[1]);
                    core::ptr::write_volatile(self.base.add(off + 2), bytes[2]);
                }
                4 => {
                    core::ptr::write_volatile(self.base.add(off) as *mut u32, packed);
                }
                _ => unreachable!(),
            }
        }
    }

    /// Read a packed pixel word from `(x, y)`.
    #[inline]
    fn read_pixel_raw(&self, x: u32, y: u32) -> u32 {
        let off = self.offset(x, y);
        unsafe {
            match self.bytes_per_pixel {
                2 => core::ptr::read_volatile(self.base.add(off) as *const u16) as u32,
                3 => {
                    let b0 = core::ptr::read_volatile(self.base.add(off)) as u32;
                    let b1 = core::ptr::read_volatile(self.base.add(off + 1)) as u32;
                    let b2 = core::ptr::read_volatile(self.base.add(off + 2)) as u32;
                    b0 | (b1 << 8) | (b2 << 16)
                }
                4 => core::ptr::read_volatile(self.base.add(off) as *const u32),
                _ => unreachable!(),
            }
        }
    }

    /// Pack an ARGB u32 `color` into the hardware pixel word format.
    #[inline]
    fn pack_color(&self, color: u32) -> u32 {
        let r = ((color >> 16) & 0xFF) as u8;
        let g = ((color >> 8) & 0xFF) as u8;
        let b = (color & 0xFF) as u8;
        self.r.pack(r) | self.g.pack(g) | self.b.pack(b)
    }

    /// Unpack a hardware pixel word into an ARGB u32 (alpha = 0xFF).
    #[inline]
    fn unpack_color(&self, packed: u32) -> u32 {
        let r = self.r.extract(packed) as u32;
        let g = self.g.extract(packed) as u32;
        let b = self.b.extract(packed) as u32;
        0xFF00_0000 | (r << 16) | (g << 8) | b
    }
}

// FrameBuffer trait impl
impl FrameBuffer for Mb2Fb {
    fn width(&self) -> usize {
        self.width as usize
    }

    fn height(&self) -> usize {
        self.height as usize
    }

    fn bytes_per_pixel(&self) -> usize {
        self.bytes_per_pixel
    }

    fn pitch(&self) -> usize {
        self.pitch as usize
    }

    fn size(&self) -> usize {
        (self.pitch * self.height) as usize
    }

    fn buffer_ptr(&self) -> *mut u8 {
        self.base
    }

    fn pixel_format(&self) -> PixelFormat {
        self.pixel_format
    }

    // Bulk fill

    fn clear(&mut self, color: u32) {
        let packed = self.pack_color(color);

        // Fast path: 32 bpp — fill entire buffer as u32 words.
        if self.bytes_per_pixel == 4 && self.pitch == self.width * 4 {
            let words = (self.pitch * self.height) as usize / 4;
            unsafe {
                let ptr = self.base as *mut u32;
                for i in 0..words {
                    core::ptr::write_volatile(ptr.add(i), packed);
                }
            }
            return;
        }

        // General path.
        for y in 0..self.height {
            for x in 0..self.width {
                self.write_pixel_raw(x, y, packed);
            }
        }
    }

    // Single-pixel ops

    fn set_pixel(&mut self, x: u32, y: u32, color: u32) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }
        let packed = self.pack_color(color);
        self.write_pixel_raw(x, y, packed);
        true
    }

    fn get_pixel(&self, x: u32, y: u32) -> Option<u32> {
        if x >= self.width || y >= self.height {
            return None;
        }
        Some(self.unpack_color(self.read_pixel_raw(x, y)))
    }

    //  Optimised line primitives

    fn draw_hline(&mut self, x1: u32, x2: u32, y: u32, color: u32) {
        if y >= self.height {
            return;
        }
        let x_start = x1.min(self.width - 1);
        let x_end = x2.min(self.width - 1);
        let packed = self.pack_color(color);

        if self.bytes_per_pixel == 4 {
            // Write whole scanline segment as u32 words — one volatile per pixel.
            let base_off = self.offset(x_start, y);
            unsafe {
                let ptr = self.base.add(base_off) as *mut u32;
                for i in 0..=(x_end - x_start) as usize {
                    core::ptr::write_volatile(ptr.add(i), packed);
                }
            }
        } else {
            for x in x_start..=x_end {
                self.write_pixel_raw(x, y, packed);
            }
        }
    }

    fn draw_vline(&mut self, x: u32, y1: u32, y2: u32, color: u32) {
        if x >= self.width {
            return;
        }
        let y_start = y1.min(self.height - 1);
        let y_end = y2.min(self.height - 1);
        let packed = self.pack_color(color);
        for y in y_start..=y_end {
            self.write_pixel_raw(x, y, packed);
        }
    }

    fn draw_rect(&mut self, x: u32, y: u32, width: u32, height: u32, color: u32) {
        let packed = self.pack_color(color);
        let x_end = (x + width).min(self.width);
        let y_end = (y + height).min(self.height);

        for row in y..y_end {
            if self.bytes_per_pixel == 4 {
                let base_off = self.offset(x, row);
                unsafe {
                    let ptr = self.base.add(base_off) as *mut u32;
                    for col in 0..(x_end - x) as usize {
                        core::ptr::write_volatile(ptr.add(col), packed);
                    }
                }
            } else {
                for col in x..x_end {
                    self.write_pixel_raw(col, row, packed);
                }
            }
        }
    }

    /// Optimised region copy using row-level volatile moves.
    ///
    /// Handles overlapping src/dst correctly by choosing copy direction.
    fn copy_region(
        &mut self,
        src_x: u32,
        src_y: u32,
        dst_x: u32,
        dst_y: u32,
        width: u32,
        height: u32,
    ) {
        let row_bytes = width as usize * self.bytes_per_pixel;

        // Determine row iteration order to handle vertical overlap.
        let rows: &dyn Fn(u32) -> u32 = if dst_y <= src_y {
            &|i| i
        } else {
            &|i| height - 1 - i
        };

        for i in 0..height {
            let si = rows(i);
            let src_off = self.offset(src_x, src_y + si);
            let dst_off = self.offset(dst_x, dst_y + si);
            unsafe {
                // Use memmove semantics via byte-by-byte volatile copy.
                // For non-overlapping rows this is always safe; for
                // overlapping same-row cases the direction choice above
                // is sufficient.
                if dst_off <= src_off {
                    for b in 0..row_bytes {
                        let v = core::ptr::read_volatile(self.base.add(src_off + b));
                        core::ptr::write_volatile(self.base.add(dst_off + b), v);
                    }
                } else {
                    for b in (0..row_bytes).rev() {
                        let v = core::ptr::read_volatile(self.base.add(src_off + b));
                        core::ptr::write_volatile(self.base.add(dst_off + b), v);
                    }
                }
            }
        }
    }
}

// Pixel-format detection

fn detect_pixel_format(tag: &Mb2FbTag, bpp_bytes: usize) -> PixelFormat {
    // Only 3-byte and 4-byte pixels have an unambiguous standard layout.
    match bpp_bytes {
        3 => {
            // Check for BGR (e.g. many real GRUB configurations are BGR24)
            if tag.blue.field_pos == 0 && tag.green.field_pos == 8 && tag.red.field_pos == 16 {
                PixelFormat::Bgr
            } else {
                PixelFormat::Rgb
            }
        }
        4 => {
            let rp = tag.red.field_pos;
            let gp = tag.green.field_pos;
            let bp = tag.blue.field_pos;
            match (rp, gp, bp) {
                (16, 8, 0) => PixelFormat::Bgra, // 0xAARRGGBB → BGRA in memory (LE)
                (0, 8, 16) => PixelFormat::Rgba,
                _ => PixelFormat::Rgb, // best-effort fallback
            }
        }
        _ => PixelFormat::Rgb,
    }
}

// Convenience: parse from a raw multiboot2 tag pointer

/// Parse an `Mb2FbTag` directly from the raw bytes of a multiboot2 tag.
///
/// `ptr` must point to the first byte of the multiboot2 framebuffer tag
/// **payload** (i.e. after the 8-byte `{type, size}` tag header).
///
/// # Safety
/// `ptr` must be valid for at least 31 bytes of initialised data.
pub unsafe fn parse_mb2_fb_tag(ptr: *const u8) -> Mb2FbTag {
    // Multiboot2 framebuffer tag layout (after the 8-byte common header):
    //  0: u64  addr
    //  8: u32  pitch
    // 12: u32  width
    // 16: u32  height
    // 20: u8   bpp
    // 21: u8   type
    // 22: u8   _reserved
    // 23: u8   red_field_position
    // 24: u8   red_mask_size
    // 25: u8   green_field_position
    // 26: u8   green_mask_size
    // 27: u8   blue_field_position
    // 28: u8   blue_mask_size
    unsafe {
        let addr = core::ptr::read_unaligned(ptr as *const u64);
        let pitch = core::ptr::read_unaligned(ptr.add(8) as *const u32);
        let width = core::ptr::read_unaligned(ptr.add(12) as *const u32);
        let height = core::ptr::read_unaligned(ptr.add(16) as *const u32);
        let bpp = *ptr.add(20);
        let fb_type = *ptr.add(21);
        let red = ChannelDesc {
            field_pos: *ptr.add(23),
            mask_size: *ptr.add(24),
        };
        let green = ChannelDesc {
            field_pos: *ptr.add(25),
            mask_size: *ptr.add(26),
        };
        let blue = ChannelDesc {
            field_pos: *ptr.add(27),
            mask_size: *ptr.add(28),
        };

        Mb2FbTag {
            addr,
            pitch,
            width,
            height,
            bpp,
            fb_type,
            red,
            green,
            blue,
        }
    }
}

/// Storage cell.  Filled by `kernel::boot::multiboot2`.
pub static MB2_FB_TAG: Once<Mb2FbTag> = Once::new();

/// Called by `kernel::boot::multiboot2::parse_framebuffer` to store the tag.
pub fn set_mb2_fb_tag(tag: Mb2FbTag) {
    MB2_FB_TAG.call_once(|| tag);
}
