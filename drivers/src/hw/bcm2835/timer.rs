use core::ptr::{read_volatile, write_volatile};

/// Constants
pub const TIMER_BASE: usize = 0x2000_3000;
pub const CS_MATCH3: u32 = 1 << 3;
pub const CS_MATCH2: u32 = 1 << 2;
pub const CS_MATCH1: u32 = 1 << 1;
pub const CS_MATCH0: u32 = 1 << 0;

#[repr(C)]
pub struct Timer {
    cs: u32,
    clo: u32,
    chi: u32,
    c0: u32,
    c1: u32,
    c2: u32,
    c3: u32,
}

#[inline(always)]
fn regs() -> *mut Timer {
    TIMER_BASE as *mut Timer
}

impl Timer {
    pub fn start(interval_us: u32) {
        unsafe {
            let r = regs();

            // Read current time
            let now = read_volatile(&(*r).clo);

            // Set compare register
            write_volatile(&mut (*r).c1, now.wrapping_add(interval_us));

            // Clear any pending match (write-1-to-clear)
            write_volatile(&mut (*r).cs, CS_MATCH1);
        }
    }

    pub fn clear_interrupt() {
        unsafe {
            // Write-1-to-clear match bit
            write_volatile(&mut (*regs()).cs, CS_MATCH1);
        }
    }
}
