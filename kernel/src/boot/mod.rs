//! Boot-time platform discovery.
//!
//! This module owns everything that touches boot protocols:
//!   - Multiboot2 tag parsing  (`multiboot2`)
//!   - Hardware probing         (`probe`)
//!   - Device tree parsing      (`device_tree`, stub until DTB support lands)
//!
//! The public entry point is [`init`], called from `kmain` with the
//! boot-protocol arguments GRUB/U-Boot left in registers.
//!
//! After [`init`] returns, [`drivers::platform::Platform`] is fully
//! populated and safe to query from anywhere.

pub mod device_tree;
pub mod multiboot2;
pub mod probe;

use drivers::platform::{Architecture, PlatformBuilder};

/// Boot information passed in from the arch-specific entry point.
#[derive(Debug)]
pub enum BootInfo {
    Multiboot2 { magic: u32, info_addr: usize },
    DeviceTree { dtb_addr: usize },
    Raw,
}

/// Initialize the platform from boot information.
///
/// Tries discovery methods in order:
/// 1. Multiboot2  (x86 / GRUB)
/// 2. Device tree (ARM / U-Boot)
/// 3. ACPI        (future)
/// 4. Hardware probing (fallback)
///
/// # Safety
/// Must be called exactly once, very early in boot before memory
/// management is initialized.
pub unsafe fn init(boot_info: BootInfo) -> Result<(), &'static str> {
    PlatformBuilder::begin()?;

    let arch = detect_architecture();
    PlatformBuilder::set_arch(arch);

    let discovered = match boot_info {
        BootInfo::Multiboot2 { magic, info_addr } => unsafe {
            multiboot2::discover(magic, info_addr).is_ok()
        },
        BootInfo::DeviceTree { dtb_addr } => unsafe { device_tree::discover(dtb_addr).is_ok() },
        BootInfo::Raw => false,
    };

    if !discovered {
        unsafe {
            match arch {
                Architecture::X86 | Architecture::X86_64 => {
                    // ACPI not yet implemented — fall straight to probing
                    probe::x86()?;
                }
                Architecture::Arm | Architecture::AArch64 => {
                    probe::arm()?;
                }
            }
        }
    }

    PlatformBuilder::set_platform_name(determine_platform_name());
    Ok(())
}

// Helpers

fn detect_architecture() -> Architecture {
    #[cfg(all(target_arch = "x86", target_pointer_width = "32"))]
    return Architecture::X86;

    #[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
    return Architecture::X86_64;

    #[cfg(all(target_arch = "arm", target_pointer_width = "32"))]
    return Architecture::Arm;

    #[cfg(all(target_arch = "aarch64", target_pointer_width = "64"))]
    return Architecture::AArch64;
}

fn determine_platform_name() -> &'static str {
    use drivers::platform::Platform;

    let has_compat = |s: &str| Platform::devices().any(|d| d.compatible.contains(s));

    if has_compat("bcm2835") {
        "Broadcom BCM2835 (Raspberry Pi Zero/1)"
    } else if has_compat("bcm2836") {
        "Broadcom BCM2836 (Raspberry Pi 2)"
    } else if has_compat("bcm2837") {
        "Broadcom BCM2837 (Raspberry Pi 3)"
    } else if has_compat("bcm2711") {
        "Broadcom BCM2711 (Raspberry Pi 4)"
    } else {
        match detect_architecture() {
            Architecture::X86 | Architecture::X86_64 => "Generic PC (x86)",
            Architecture::Arm | Architecture::AArch64 => "ARM-based Platform",
        }
    }
}
