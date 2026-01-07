#![allow(dead_code)]

use core::ptr::{read_volatile, write_volatile};

use super::PERIPHERAL_BASE;

/* Mailbox registers */
const MAILBOX_BASE: usize = PERIPHERAL_BASE + 0xB880;

const MAILBOX_READ: usize = MAILBOX_BASE + 0x00;
const MAILBOX_STATUS: usize = MAILBOX_BASE + 0x18;
const MAILBOX_WRITE: usize = MAILBOX_BASE + 0x20;

/* Status flags */
const MAILBOX_EMPTY: u32 = 1 << 30;
const MAILBOX_FULL: u32 = 1 << 31;

/* Masks */
const MAILBOX_READ_CHANNEL_MASK: u32 = 0xF;
const MAILBOX_READ_DATA_MASK: u32 = 0xFFFF_FFF0;
const MAILBOX_WRITE_CHANNEL_MASK: u32 = 0xF;
const MAILBOX_WRITE_DATA_MASK: u32 = 0xFFFF_FFF0;

/* Channels */
pub const CHANNEL_POWER: u8 = 0;
pub const CHANNEL_FB: u8 = 1;
pub const CHANNEL_VUART: u8 = 2;
pub const CHANNEL_VCHIQ: u8 = 3;
pub const CHANNEL_LEDS: u8 = 4;
pub const CHANNEL_BUTTONS: u8 = 5;
pub const CHANNEL_TOUCHSCREEN: u8 = 6;
pub const CHANNEL_UNUSED: u8 = 7;
pub const CHANNEL_PROPERTY: u8 = 8;

/* Property interface response codes */
pub const RESPONSE_SUCCESS: u32 = 0x8000_0000;
pub const RESPONSE_ERROR: u32 = 0x8000_0001;

/// Perform a mailbox call.
///
/// `buffer_phys` **must** be:
/// - 16-byte aligned
/// - A physical address visible to the GPU
///
/// Returns `true` on success (VC responded with SUCCESS).
///
/// # Safety
/// - Caller must ensure buffer is valid and correctly formatted
/// - Caller must ensure MMIO is accessible
/// - Not synchronized for multicore use
pub unsafe fn mailbox_call(channel: u8, buffer_phys: usize) -> bool {
    // Mailbox requires lower 4 bits to be channel
    let msg = (buffer_phys & MAILBOX_WRITE_DATA_MASK as usize)
        | (channel as usize & MAILBOX_WRITE_CHANNEL_MASK as usize);

    /* Wait until mailbox is not full */
    while unsafe { read_volatile(MAILBOX_STATUS as *const u32) } & MAILBOX_FULL != 0 {}

    /* Write request */
    unsafe {
        write_volatile(MAILBOX_WRITE as *mut u32, msg as u32);
    }

    loop {
        /* Wait for response */
        while unsafe { read_volatile(MAILBOX_STATUS as *const u32) } & MAILBOX_EMPTY != 0 {}

        let resp = unsafe { read_volatile(MAILBOX_READ as *const u32) };

        /* Check channel */
        if (resp & MAILBOX_READ_CHANNEL_MASK) == channel as u32 {
            let resp_addr = (resp & !MAILBOX_READ_CHANNEL_MASK) as usize;

            if resp_addr != buffer_phys {
                return false;
            }

            // First word of buffer is total size
            // Second word is response code
            let response_code = unsafe { read_volatile((buffer_phys + 4) as *const u32) };

            return response_code == RESPONSE_SUCCESS;
        }
    }
}
