use super::mailbox::{CHANNEL_PROPERTY, mailbox_call};
use core::ptr::read_volatile;

/// Mailbox tag: Get ARM memory
const TAG_GET_ARM_MEMORY: u32 = 0x00010005;

/// Response buffer must be 16-byte aligned and visible to the GPU
#[repr(C, align(16))]
struct ArmMemoryRequest {
    size: u32,
    code: u32,

    tag: u32,
    val_buf_size: u32,
    val_len: u32,

    base: u32,
    length: u32,

    end: u32,
}

/// Query total ARM-accessible RAM from VideoCore
///
/// Returns `(base, size)` in bytes
///
/// # Safety
/// - Must be called when mailbox MMIO is available
/// - Identity mapping required (phys == virt)
pub unsafe fn get_arm_memory() -> Option<(usize, usize)> {
    static mut REQ: ArmMemoryRequest = ArmMemoryRequest {
        size: core::mem::size_of::<ArmMemoryRequest>() as u32,
        code: 0,

        tag: TAG_GET_ARM_MEMORY,
        val_buf_size: 8,
        val_len: 0,

        base: 0,
        length: 0,

        end: 0,
    };

    let req_phys = &raw mut REQ as *mut _ as usize;

    if unsafe { !mailbox_call(CHANNEL_PROPERTY, req_phys) } {
        return None;
    }

    unsafe {
        let base = read_volatile(core::ptr::addr_of!(REQ.base)) as usize;
        let size = read_volatile(core::ptr::addr_of!(REQ.length)) as usize;

        Some((base, size))
    }
}
