//! Device tree (FDT/DTB) platform discovery.
//!
//! Currently a stub.

#[cfg(target_arch = "arm")]
use drivers::platform::{DeviceInfo, MemoryRegion, MemoryType, PlatformBuilder};

/// Walk a Flattened Device Tree and populate the platform tables.
///
/// # Safety
/// `dtb_addr` must be the physical/identity-mapped base of a valid FDT blob.
pub unsafe fn discover(_dtb_addr: usize) -> Result<(), &'static str> {
    #[cfg(target_arch = "arm")]
    {
        use fdt::Fdt;

        let fdt =
            unsafe { Fdt::from_ptr(_dtb_addr as *const u8).map_err(|_| "invalid device tree")? };

        for region in fdt.memory().regions() {
            PlatformBuilder::add_memory_region(MemoryRegion {
                base: region.starting_address as usize,
                size: region.size.unwrap_or(0),
                mem_type: MemoryType::Available,
            });
        }

        for node in fdt.all_nodes() {
            let Some(compatible) = node.compatible() else {
                continue;
            };
            let Some(compat_str) = compatible.first() else {
                continue;
            };
            let Some(mut reg) = node.reg() else { continue };
            let Some(region) = reg.next() else { continue };

            PlatformBuilder::add_device(DeviceInfo {
                name: node.name,
                compatible: compat_str,
                base_addr: region.starting_address as usize,
                size: region.size.unwrap_or(0),
                irq: node.interrupts().and_then(|mut i| i.next()),
            });
        }

        return Ok(());
    }

    #[cfg(not(target_arch = "arm"))]
    Err("device tree support not enabled")
}
