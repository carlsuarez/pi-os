// kernel/src/kcore/init.rs

use crate::mm::mmu::{MmuOps, PlatformMmu};
use crate::mm::{heap_allocator, page_allocator::page_allocator};
use crate::subsystems::console_write;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicUsize, Ordering};
use drivers::platform::{BootInfo, MemoryType, Platform};

/// Physical address of the kernel L1 page table.
/// Written once by setup_memory_management(), read by ArmMmu::init()
/// and ArmMmu::map_region() / unmap_region().
#[cfg(target_arch = "arm")]
pub static KERNEL_L1_TABLE_PHYS: AtomicUsize = AtomicUsize::new(0);

// ============================================================================
// Kernel Initialization
// ============================================================================

#[unsafe(no_mangle)]
pub extern "C" fn kernel_init(machine_type: u32, atags_addr: u32) {
    unsafe {
        let boot_info = determine_boot_info(machine_type, atags_addr);
        crate::subsystems::init(boot_info);

        setup_memory_management();

        // MMU init is driven from Rust — no assembly call needed.
        // On ARM this reads KERNEL_L1_TABLE_PHYS, maps kernel regions,
        // and enables the MMU before we proceed to kernel_main.
        #[cfg(target_arch = "arm")]
        {
            let l1_phys = KERNEL_L1_TABLE_PHYS.load(Ordering::Relaxed);
            PlatformMmu::init(l1_phys);
        }

        console_write("===========================================\n");
        console_write("Kernel Early Initialization Complete\n");
        console_write("===========================================\n");

        log_system_info();
        log_discovered_hardware();
        log_available_devices();

        console_write("===========================================\n");
    }
}

// ============================================================================
// Boot Info
// ============================================================================

unsafe fn determine_boot_info(machine_type: u32, atags_addr: u32) -> BootInfo {
    #[cfg(target_arch = "x86")]
    {
        if machine_type == 0x36d76289 {
            return BootInfo::Multiboot2 {
                magic: machine_type,
                info_addr: atags_addr as usize,
            };
        }
        BootInfo::Raw
    }

    #[cfg(target_arch = "arm")]
    {
        if atags_addr != 0 {
            let magic = core::ptr::read_volatile(atags_addr as *const u32);
            if magic.to_be() == 0xd00dfeed {
                return BootInfo::DeviceTree {
                    dtb_addr: atags_addr as usize,
                };
            }
        }
        BootInfo::Raw
    }

    #[cfg(not(any(target_arch = "x86", target_arch = "arm")))]
    {
        let _ = (machine_type, atags_addr);
        BootInfo::Raw
    }
}

// ============================================================================
// Memory Management Setup
// ============================================================================

unsafe fn setup_memory_management() {
    let mm = Platform::memory_map();

    // _free_memory_start is the linker symbol marking the end of the kernel
    // image + stacks. Everything below this is already spoken for.
    let kernel_end = unsafe { get_kernel_end_address() };
    let free_mem_start = (kernel_end + 0xFFF) & !0xFFF;

    let ram_end = mm.ram_start + mm.ram_size;

    // Sanity: free memory must fall inside the reported RAM region
    assert!(
        free_mem_start >= mm.ram_start && free_mem_start < ram_end,
        "Kernel end address falls outside the reported RAM region"
    );

    // Sanity: free memory must not overlap the peripheral window
    if mm.peripheral_size > 0 {
        assert!(
            free_mem_start < mm.peripheral_base
                || free_mem_start >= mm.peripheral_base + mm.peripheral_size,
            "Kernel end address overlaps the peripheral MMIO region"
        );
    }

    // -------------------------------------------------------------------------
    // ARM: reserve L1 page table (16KB, 16KB-aligned) at the base of free
    // memory so its physical address is fixed before the MMU is enabled.
    // -------------------------------------------------------------------------
    #[cfg(target_arch = "arm")]
    let post_table_start = {
        const L1_TABLE_SIZE: usize = 16 * 1024;
        const L1_TABLE_ALIGN: usize = 16 * 1024;

        let l1_table_start = (free_mem_start + L1_TABLE_ALIGN - 1) & !(L1_TABLE_ALIGN - 1);
        let l1_table_end = l1_table_start + L1_TABLE_SIZE;

        if mm.peripheral_size > 0 {
            assert!(
                l1_table_end <= mm.peripheral_base,
                "L1 page table allocation would overlap the peripheral MMIO region"
            );
        }

        // Zero before handing to ArmMmu::init
        core::ptr::write_bytes(l1_table_start as *mut u8, 0, L1_TABLE_SIZE);
        KERNEL_L1_TABLE_PHYS.store(l1_table_start, Ordering::Relaxed);

        (l1_table_end + 0xFFF) & !0xFFF
    };

    #[cfg(not(target_arch = "arm"))]
    let post_table_start = free_mem_start;

    // -------------------------------------------------------------------------
    // Clamp usable RAM end to exclude the peripheral window if it sits
    // inside the RAM region (as it does on all BCM2835/6/7 platforms).
    // -------------------------------------------------------------------------
    let usable_ram_end = if mm.peripheral_size > 0 && mm.peripheral_base < ram_end {
        mm.peripheral_base
    } else {
        ram_end
    };

    // -------------------------------------------------------------------------
    // Heap: 10% of remaining RAM, capped at 16MB
    // -------------------------------------------------------------------------
    let available_ram = usable_ram_end.saturating_sub(post_table_start);
    let heap_size = core::cmp::min(16 * 1024 * 1024, available_ram / 10);

    let heap_start = post_table_start;
    let heap_end = heap_start + heap_size;
    let page_alloc_start = (heap_end + 0xFFF) & !0xFFF;
    let page_alloc_end = usable_ram_end;

    // Final guard: page allocator range must not touch MMIO
    if mm.peripheral_size > 0 {
        assert!(
            page_alloc_end <= mm.peripheral_base
                || page_alloc_start >= mm.peripheral_base + mm.peripheral_size,
            "Page allocator range overlaps the peripheral MMIO region"
        );
    }

    unsafe {
        heap_allocator::init_heap(heap_start, heap_end);
        page_allocator().init(page_alloc_start, page_alloc_end);
    }

    let l1 = {
        #[cfg(target_arch = "arm")]
        {
            let start = KERNEL_L1_TABLE_PHYS.load(Ordering::Relaxed);
            Some((start, start + 16 * 1024))
        }

        #[cfg(not(target_arch = "arm"))]
        {
            None
        }
    };

    log_memory_layout(
        kernel_end,
        heap_start,
        heap_end,
        page_alloc_start,
        page_alloc_end,
        l1,
    );
}

#[cfg(target_arch = "x86")]
unsafe fn get_kernel_end_address() -> usize {
    unsafe extern "C" {
        static _bss_end: u8;
    }
    let bss_end = core::ptr::addr_of!(_bss_end) as usize;
    if bss_end > 0x100000 && bss_end < 0x6400000 {
        bss_end
    } else {
        0x100000 + 2 * 1024 * 1024
    }
}

#[cfg(target_arch = "arm")]
unsafe fn get_kernel_end_address() -> usize {
    unsafe extern "C" {
        static _free_memory_start: u8;
    }
    core::ptr::addr_of!(_free_memory_start) as usize
}

#[cfg(not(any(target_arch = "x86", target_arch = "arm")))]
unsafe fn get_kernel_end_address() -> usize {
    0x200000
}

// ============================================================================
// Logging
// ============================================================================

fn log_memory_layout(
    kernel_end: usize,
    heap_start: usize,
    heap_end: usize,
    page_start: usize,
    page_end: usize,
    l1_table: Option<(usize, usize)>,
) {
    use alloc::format;

    console_write("\nMemory Layout:\n");

    let msg = format!("  Kernel End:     0x{:08x}\n", kernel_end);
    console_write(&msg);

    if let Some((l1_table_start, l1_table_end)) = l1_table {
        let msg = format!(
            "  L1 Page Table:  0x{:08x} - 0x{:08x} (16 KB)\n",
            l1_table_start, l1_table_end,
        );
        console_write(&msg);
    }

    let heap_kb = (heap_end - heap_start) / 1024;
    let msg = format!(
        "  Heap:           0x{:08x} - 0x{:08x} ({} KB)\n",
        heap_start, heap_end, heap_kb,
    );
    console_write(&msg);

    let page_mb = (page_end - page_start) / (1024 * 1024);
    let msg = format!(
        "  Page Allocator: 0x{:08x} - 0x{:08x} ({} MB)\n",
        page_start, page_end, page_mb,
    );
    console_write(&msg);
}

fn log_system_info() {
    use alloc::format;

    console_write("\nSystem Information:\n");

    let msg = format!("  Platform:      {}\n", Platform::name());
    console_write(&msg);
    let msg = format!("  Architecture:  {}\n", Platform::arch());
    console_write(&msg);
    let total_mb = Platform::total_ram() / (1024 * 1024);
    let msg = format!("  Total RAM:     {} MB\n", total_mb);
    console_write(&msg);

    if let Some(cmdline) = Platform::cmdline() {
        let msg = format!("  Command Line:  {}\n", cmdline);
        console_write(&msg);
    }
}

fn log_discovered_hardware() {
    use alloc::format;

    console_write("\nDiscovered Hardware:\n");

    console_write("  Memory Regions:\n");
    for region in Platform::memory_regions() {
        let type_str = match region.mem_type {
            MemoryType::Available => "Available",
            MemoryType::Reserved => "Reserved",
            MemoryType::Mmio => "MMIO",
            MemoryType::Kernel => "Kernel",
            MemoryType::Framebuffer => "Framebuffer",
        };
        let size_kb = region.size / 1024;
        let msg = format!(
            "    {:12} : 0x{:08x} - 0x{:08x} ({} KB)\n",
            type_str,
            region.base,
            region.base + region.size,
            size_kb,
        );
        console_write(&msg);
    }

    console_write("  Devices:\n");
    for device in Platform::devices() {
        let msg = format!(
            "    {} ({}) @ 0x{:08x}",
            device.name, device.compatible, device.base_addr,
        );
        console_write(&msg);
        if let Some(irq) = device.irq {
            let msg = format!(" IRQ {}", irq);
            console_write(&msg);
        }
        console_write("\n");
    }
}

fn log_available_devices() {
    use crate::device_manager;
    use drivers::device_manager::DeviceManager;

    let names: Vec<alloc::string::String> = {
        let mgr = device_manager().lock();
        mgr.list().cloned().collect()
    };

    console_write("\nRegistered Devices:\n");
    for name in names {
        console_write("  - ");
        console_write(&name);
        console_write("\n");
    }
}
