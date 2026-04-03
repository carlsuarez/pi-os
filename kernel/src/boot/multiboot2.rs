//! Multiboot2 boot-info parsing.
//!
//! Called once from [`super::init`] when GRUB hands us a Multiboot2
//! info structure.  Populates [`drivers::platform::PlatformBuilder`]
//! with memory regions and device entries, and stashes the framebuffer
//! tag in [`MB2_FB_TAG`] for [`drivers::peripheral::x86::mb2fb::Mb2Fb`]
//! to consume during device init.

use drivers::peripheral::x86::mb2fb::{ChannelDesc, Mb2FbTag, parse_mb2_fb_tag, set_mb2_fb_tag};
use drivers::platform::{DeviceInfo, MemoryRegion, MemoryType, PlatformBuilder};

const MB2_MAGIC: u32 = 0x36d7_6289;

/// Walk the multiboot2 info structure and populate the platform tables.
///
/// # Safety
/// `info_addr` must be the physical/identity-mapped address that GRUB
/// passed in `ebx`.  Must be called at most once.
pub unsafe fn discover(magic: u32, info_addr: usize) -> Result<(), &'static str> {
    if magic != MB2_MAGIC {
        return Err("invalid Multiboot2 magic");
    }

    unsafe {
        let total_size = *(info_addr as *const u32) as usize;
        let end_addr = info_addr + total_size;
        let mut tag_addr = info_addr + 8; // skip {total_size, reserved}

        while tag_addr < end_addr {
            // Tags are 8-byte aligned
            tag_addr = (tag_addr + 7) & !7;

            let tag_type = *(tag_addr as *const u32);
            let tag_size = *((tag_addr + 4) as *const u32) as usize;

            if tag_type == 0 {
                break; // terminator tag
            }

            match tag_type {
                1 => parse_cmdline(tag_addr),
                6 => parse_memory_map(tag_addr)?,
                8 => parse_framebuffer(tag_addr),
                _ => {}
            }

            tag_addr += tag_size;
        }
    }

    // Always register the standard PC devices (serial, PIT, PIC, VGA)
    // regardless of what the MB2 tags contained.
    super::probe::register_standard_pc_devices();
    Ok(())
}

// Tag parsers

unsafe fn parse_cmdline(tag_addr: usize) {
    unsafe {
        let ptr = (tag_addr + 8) as *const u8;
        let mut len = 0usize;
        while *ptr.add(len) != 0 {
            len += 1;
        }
        if let Ok(s) = core::str::from_utf8(core::slice::from_raw_parts(ptr, len)) {
            PlatformBuilder::set_cmdline(s);
        }
    }
}

unsafe fn parse_memory_map(tag_addr: usize) -> Result<(), &'static str> {
    unsafe {
        let entry_size = *((tag_addr + 8) as *const u32) as usize;
        let entry_version = *((tag_addr + 12) as *const u32);
        if entry_version != 0 {
            return Err("unsupported multiboot2 memory map version");
        }

        let tag_size = *((tag_addr + 4) as *const u32) as usize;
        let entries_end = tag_addr + tag_size;
        let mut entry = tag_addr + 16;

        while entry < entries_end {
            let base = *(entry as *const u64) as usize;
            let length = *((entry + 8) as *const u64) as usize;
            let entry_type = *((entry + 16) as *const u32);

            let mem_type = match entry_type {
                1 => MemoryType::Available,
                3 => MemoryType::Mmio,
                _ => MemoryType::Reserved,
            };

            PlatformBuilder::add_memory_region(MemoryRegion {
                base,
                size: length,
                mem_type,
            });
            entry += entry_size;
        }
    }
    Ok(())
}

unsafe fn parse_framebuffer(tag_addr: usize) {
    unsafe {
        let tag = parse_mb2_fb_tag((tag_addr + 8) as *const u8);
        set_mb2_fb_tag(tag);

        PlatformBuilder::add_device(DeviceInfo {
            name: "framebuffer",
            compatible: "multiboot2-fb",
            base_addr: tag.addr as usize,
            size: (tag.pitch * tag.height) as usize,
            irq: None,
        });
    }
}
