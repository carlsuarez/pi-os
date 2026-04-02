//
// VGA text-mode driver for pi-os x86.
//
// The VGA text buffer lives at physical address 0xB8000 and is accessible
// as a flat array of (character, attribute) byte pairs:
//
//   offset = (row * VGA_COLS + col) * 2
//   [offset + 0] = ASCII character code
//   [offset + 1] = attribute byte:
//       bits [7:4] = background colour (0–7, bit 7 = blink enable)
//       bits [3:0] = foreground colour (0–15)
//
// Resolution: 80 columns × 25 rows = 2 000 cells = 4 000 bytes.
//
// Cursor position is programmed via I/O ports 0x3D4 (index) / 0x3D5 (data)
// on the CRT controller.

use crate::hal::console::ConsoleOutput;
use core::fmt;

pub const VGA_COLS: usize = 80;
pub const VGA_ROWS: usize = 25;

/// Physical base address of the VGA text buffer.
const VGA_BUFFER_ADDR: usize = 0xB8000;

/// CRT controller index / data ports.
const CRTC_ADDR_PORT: u16 = 0x3D4;
const CRTC_DATA_PORT: u16 = 0x3D5;

/// CRT controller register indices for the 16-bit cursor position.
const CRTC_CURSOR_HIGH: u8 = 0x0E;
const CRTC_CURSOR_LOW: u8 = 0x0F;

// ── Colour definitions ───────────────────────────────────────────────────────

/// Standard 4-bit VGA colours.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
#[allow(dead_code)]
pub enum Color {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    Pink = 13,
    Yellow = 14,
    White = 15,
}

/// Pack a foreground + background colour into a single VGA attribute byte.
#[inline(always)]
const fn make_attr(fg: Color, bg: Color) -> u8 {
    ((bg as u8) << 4) | (fg as u8)
}

/// A volatile-write wrapper around the raw VGA buffer.
///
/// All writes go through `write_volatile` so the compiler never
/// optimises away "dead" stores to MMIO memory.
struct VgaBuffer {
    base: *mut u8,
}

// SAFETY: we only ever construct one VgaBuffer (inside VgaText) and access
// it through the &mut self methods of VgaText, which is itself guarded by a
// SpinLock in the subsystem layer.
unsafe impl Send for VgaBuffer {}

impl VgaBuffer {
    /// # Safety
    /// Caller must ensure `addr` is the true VGA buffer base and that no
    /// other code aliases this pointer without synchronisation.
    const unsafe fn new(addr: usize) -> Self {
        Self {
            base: addr as *mut u8,
        }
    }

    /// Write a character + attribute pair at `(col, row)`.
    #[inline]
    fn write_cell(&mut self, col: usize, row: usize, ch: u8, attr: u8) {
        let offset = (row * VGA_COLS + col) * 2;
        unsafe {
            core::ptr::write_volatile(self.base.add(offset), ch);
            core::ptr::write_volatile(self.base.add(offset + 1), attr);
        }
    }

    /// Read the character byte at `(col, row)`.
    #[inline]
    fn read_char(&self, col: usize, row: usize) -> u8 {
        let offset = (row * VGA_COLS + col) * 2;
        unsafe { core::ptr::read_volatile(self.base.add(offset)) }
    }

    /// Read the attribute byte at `(col, row)`.
    #[inline]
    fn read_attr(&self, col: usize, row: usize) -> u8 {
        let offset = (row * VGA_COLS + col) * 2 + 1;
        unsafe { core::ptr::read_volatile(self.base.add(offset)) }
    }
}

#[inline]
unsafe fn outb(port: u16, val: u8) {
    unsafe {
        core::arch::asm!(
            "out dx, al",
            in("dx") port,
            in("al") val,
            options(nomem, nostack, preserves_flags)
        );
    }
}

/// VGA 80×25 text-mode driver.
///
/// Construct with [`VgaText::new`] and register with the kernel's console
/// subsystem.  All state (cursor position, colour) lives in this struct;
/// the hardware buffer is at the fixed address `0xB8000`.
pub struct VgaText {
    buf: VgaBuffer,
    col: usize,
    row: usize,
    attr: u8,
}

impl VgaText {
    /// Construct the driver and clear the screen.
    ///
    /// Default colours: light-gray on black — the classic BIOS default.
    ///
    /// # Safety
    /// Must be called only when the VGA text buffer at `0xB8000` is
    /// accessible (i.e. after the bootloader has put the CPU in text mode,
    /// which GRUB does by default).  Call at most once.
    pub unsafe fn new() -> Self {
        let mut vga = Self {
            buf: unsafe { VgaBuffer::new(VGA_BUFFER_ADDR) },
            col: 0,
            row: 0,
            attr: make_attr(Color::LightGray, Color::Black),
        };
        vga.clear();
        vga
    }

    /// Change the current foreground/background colour for subsequent writes.
    pub fn set_color(&mut self, fg: Color, bg: Color) {
        self.attr = make_attr(fg, bg);
    }

    /// Scroll the entire display up by one row, clearing the bottom row.
    fn scroll_up(&mut self) {
        // Copy rows 1..VGA_ROWS-1 → 0..VGA_ROWS-2
        for row in 1..VGA_ROWS {
            for col in 0..VGA_COLS {
                let ch = self.buf.read_char(col, row);
                let attr = self.buf.read_attr(col, row);
                self.buf.write_cell(col, row - 1, ch, attr);
            }
        }
        // Clear the last row
        for col in 0..VGA_COLS {
            self.buf.write_cell(col, VGA_ROWS - 1, b' ', self.attr);
        }
    }

    /// Advance the cursor by one cell, scrolling if necessary.
    fn advance_cursor(&mut self) {
        self.col += 1;
        if self.col >= VGA_COLS {
            self.col = 0;
            self.newline();
        }
    }

    /// Move the cursor to the start of the next line, scrolling if at bottom.
    fn newline(&mut self) {
        self.col = 0;
        if self.row + 1 >= VGA_ROWS {
            self.scroll_up();
            // row stays at VGA_ROWS - 1
            self.row = VGA_ROWS - 1;
        } else {
            self.row += 1;
        }
    }

    /// Program the CRT controller hardware cursor to the current position.
    fn update_hw_cursor(&mut self) {
        let pos = (self.row * VGA_COLS + self.col) as u16;
        unsafe {
            outb(CRTC_ADDR_PORT, CRTC_CURSOR_HIGH);
            outb(CRTC_DATA_PORT, (pos >> 8) as u8);
            outb(CRTC_ADDR_PORT, CRTC_CURSOR_LOW);
            outb(CRTC_DATA_PORT, (pos & 0xFF) as u8);
        }
    }
}

impl ConsoleOutput for VgaText {
    fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => {
                self.newline();
            }
            b'\r' => {
                self.col = 0;
            }
            0x08 => {
                // Backspace: erase previous cell
                if self.col > 0 {
                    self.col -= 1;
                }
                self.buf.write_cell(self.col, self.row, b' ', self.attr);
            }
            byte => {
                self.buf.write_cell(self.col, self.row, byte, self.attr);
                self.advance_cursor();
            }
        }
        self.update_hw_cursor();
    }

    fn clear(&mut self) {
        for row in 0..VGA_ROWS {
            for col in 0..VGA_COLS {
                self.buf.write_cell(col, row, b' ', self.attr);
            }
        }
        self.col = 0;
        self.row = 0;
        self.update_hw_cursor();
    }

    fn set_cursor(&mut self, col: usize, row: usize) {
        self.col = col.min(VGA_COLS - 1);
        self.row = row.min(VGA_ROWS - 1);
        self.update_hw_cursor();
    }
}

impl fmt::Write for VgaText {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        ConsoleOutput::write_str(self, s);
        Ok(())
    }
}
