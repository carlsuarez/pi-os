pub trait Io {
    fn read8(addr: usize) -> u8;
    fn write8(addr: usize, val: u8);
    fn read16(addr: usize) -> u16;
    fn write16(addr: usize, val: u16);
    fn read32(addr: usize) -> u32;
    fn write32(addr: usize, val: u32);
    fn io_wait();
}

// --- MMIO ---

pub struct Mmio;

impl Io for Mmio {
    fn read8(addr: usize) -> u8 {
        unsafe { core::ptr::read_volatile(addr as *const u8) }
    }
    fn write8(addr: usize, val: u8) {
        unsafe { core::ptr::write_volatile(addr as *mut u8, val) }
    }
    fn read16(addr: usize) -> u16 {
        unsafe { core::ptr::read_volatile(addr as *const u16) }
    }
    fn write16(addr: usize, val: u16) {
        unsafe { core::ptr::write_volatile(addr as *mut u16, val) }
    }
    fn read32(addr: usize) -> u32 {
        unsafe { core::ptr::read_volatile(addr as *const u32) }
    }
    fn write32(addr: usize, val: u32) {
        unsafe { core::ptr::write_volatile(addr as *mut u32, val) }
    }
    fn io_wait() {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        crate::io::Pio::write8(0x80, 0);
    }
}

// --- PIO (x86 family only) ---

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub struct Pio;

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
impl Io for Pio {
    fn read8(addr: usize) -> u8 {
        unsafe {
            let val: u8;
            core::arch::asm!("in al, dx", out("al") val, in("dx") addr as u16);
            val
        }
    }
    fn write8(addr: usize, val: u8) {
        unsafe {
            core::arch::asm!("out dx, al", in("dx") addr as u16, in("al") val);
        }
    }
    fn read16(addr: usize) -> u16 {
        unsafe {
            let val: u16;
            core::arch::asm!("in ax, dx", out("ax") val, in("dx") addr as u16);
            val
        }
    }
    fn write16(addr: usize, val: u16) {
        unsafe {
            core::arch::asm!("out dx, ax", in("dx") addr as u16, in("ax") val);
        }
    }
    fn read32(addr: usize) -> u32 {
        unsafe {
            let val: u32;
            core::arch::asm!("in eax, dx", out("eax") val, in("dx") addr as u16);
            val
        }
    }
    fn write32(addr: usize, val: u32) {
        unsafe {
            core::arch::asm!("out dx, eax", in("dx") addr as u16, in("eax") val);
        }
    }
    fn io_wait() {
        Self::write8(0x80, 0);
    }
}
