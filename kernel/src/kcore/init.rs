use crate::boot::BootInfo;
use crate::logger;
use crate::mm::mmu::{MmuOps, PlatformMmu};
use crate::mm::{heap_allocator, page_allocator::page_allocator};
use crate::subsystems::enable_graphical_framebuffer;
use crate::subsystems::log_sinks::SERIAL_SINK;
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicUsize, Ordering};
use drivers::hal::console;
use drivers::platform::{MemoryType, Platform};

/// Physical address of the kernel L1 page table (ARM only).
/// Written once by setup_memory_management(), read by ArmMmu::init()
/// and ArmMmu::map_region() / unmap_region().
#[cfg(target_arch = "arm")]
pub static KERNEL_L1_TABLE_PHYS: AtomicUsize = AtomicUsize::new(0);

/// Physical address of the kernel Page Directory (x86 only).
/// Written once by setup_memory_management(), loaded into CR3 by
/// X86Mmu::init(), and available to any code that needs the PD base
/// without going through CR3 directly.
#[cfg(target_arch = "x86")]
pub static KERNEL_PD_PHYS: AtomicUsize = AtomicUsize::new(0);

// ============================================================================
// Kernel Initialization
// ============================================================================

#[unsafe(no_mangle)]
pub extern "C" fn kernel_init(machine_type: u32, atags_addr: u32) {
    unsafe {
        // Early init (device discovery, arch-specific setup, etc)
        crate::boot::init(determine_boot_info(machine_type, atags_addr))
            .expect("Platform initialization failed");

        logger::init(log::LevelFilter::Info);

        let layout = setup_memory_management();

        crate::subsystems::init_devices();

        // #[cfg(target_arch = "arm")]
        // {
        //     let l1_phys = KERNEL_L1_TABLE_PHYS.load(Ordering::Relaxed);
        //     PlatformMmu::init(l1_phys);
        // }

        // #[cfg(target_arch = "x86")]
        // {
        //     let pd_phys = KERNEL_PD_PHYS.load(Ordering::Relaxed);
        //     PlatformMmu::init(pd_phys);
        // }

        log::info!("Kernel Early Initialization Complete\n");

        logger::attach_runtime(vec![&SERIAL_SINK]);

        // enable_graphical_framebuffer().expect("Failed to enable graphical framebuffer");

        log::info!("Runtime logger attached\n");

        log_memory_layout(
            layout.kernel_end,
            layout.heap_start,
            layout.heap_end,
            layout.page_alloc_start,
            layout.page_alloc_end,
            layout.page_table,
        );
        log_system_info();
        log_discovered_hardware();
        log_available_devices();
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

unsafe fn setup_memory_management() -> MemoryLayout {
    let mm = Platform::memory_map();

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
        let periph_end = mm.peripheral_base + mm.peripheral_size;
        assert!(
            free_mem_start < mm.peripheral_base || free_mem_start >= periph_end,
            "Kernel end address overlaps the peripheral MMIO region"
        );
    }

    // -------------------------------------------------------------------------
    // ARM: reserve L1 page table (16 KB, 16 KB-aligned) at the base of free
    // memory so its physical address is fixed before the MMU is enabled.
    // -------------------------------------------------------------------------
    #[cfg(target_arch = "arm")]
    let post_table_start = {
        const L1_TABLE_SIZE: usize = 16 * 1024;
        const L1_TABLE_ALIGN: usize = 16 * 1024;

        let l1_table_start = (free_mem_start + L1_TABLE_ALIGN - 1) & !(L1_TABLE_ALIGN - 1);
        let l1_table_end = l1_table_start + L1_TABLE_SIZE;

        if mm.peripheral_size > 0 {
            let periph_end = mm.peripheral_base + mm.peripheral_size;
            assert!(
                l1_table_end <= mm.peripheral_base || l1_table_start >= periph_end,
                "L1 page table allocation would overlap the peripheral MMIO region"
            );
        }

        core::ptr::write_bytes(l1_table_start as *mut u8, 0, L1_TABLE_SIZE);
        KERNEL_L1_TABLE_PHYS.store(l1_table_start, Ordering::Relaxed);

        (l1_table_end + 0xFFF) & !0xFFF
    };

    // -------------------------------------------------------------------------
    // x86: reserve Page Directory (4 KB, 4 KB-aligned) at the base of free
    // memory so its physical address is known before CR3 is loaded.
    // -------------------------------------------------------------------------
    #[cfg(target_arch = "x86")]
    let post_table_start = {
        const PD_SIZE: usize = 4 * 1024;
        const PD_ALIGN: usize = 4 * 1024;

        let pd_start = (free_mem_start + PD_ALIGN - 1) & !(PD_ALIGN - 1);
        let pd_end = pd_start + PD_SIZE;

        if mm.peripheral_size > 0 {
            let periph_end = mm.peripheral_base + mm.peripheral_size;
            assert!(
                pd_end <= mm.peripheral_base || pd_start >= periph_end,
                "x86 page directory allocation would overlap the peripheral MMIO region"
            );
        }

        unsafe { core::ptr::write_bytes(pd_start as *mut u8, 0, PD_SIZE) };
        KERNEL_PD_PHYS.store(pd_start, Ordering::Relaxed);

        (pd_end + 0xFFF) & !0xFFF
    };

    #[cfg(not(any(target_arch = "arm", target_arch = "x86")))]
    let post_table_start = free_mem_start;

    // -------------------------------------------------------------------------
    // Clamp usable RAM end to exclude the peripheral window if it sits
    // inside the RAM region (as it does on all BCM2835/6/7 platforms).
    // -------------------------------------------------------------------------
    let usable_ram_end = if mm.peripheral_size > 0
        && mm.peripheral_base >= post_table_start
        && mm.peripheral_base < ram_end
    {
        mm.peripheral_base
    } else {
        ram_end
    };

    // -------------------------------------------------------------------------
    // Heap: 10% of remaining RAM, capped at 16 MB
    // -------------------------------------------------------------------------
    let available_ram = usable_ram_end.saturating_sub(post_table_start);
    let heap_size = core::cmp::min(16 * 1024 * 1024, available_ram / 10);

    let heap_start = post_table_start;
    let heap_end = heap_start + heap_size;
    let page_alloc_start = (heap_end + 0xFFF) & !0xFFF;
    let page_alloc_end = usable_ram_end;

    // Final guard: page allocator range must not touch MMIO
    if mm.peripheral_size > 0 {
        let periph_end = mm.peripheral_base + mm.peripheral_size;
        assert!(
            page_alloc_end <= mm.peripheral_base || page_alloc_start >= periph_end,
            "Page allocator range overlaps the peripheral MMIO region"
        );
    }

    unsafe {
        heap_allocator::init_heap(heap_start, heap_end);
        page_allocator().init(page_alloc_start, page_alloc_end);
    }

    let page_table: Option<(usize, usize)> = {
        #[cfg(target_arch = "arm")]
        {
            let start = KERNEL_L1_TABLE_PHYS.load(Ordering::Relaxed);
            Some((start, start + 16 * 1024))
        }

        #[cfg(target_arch = "x86")]
        {
            let start = KERNEL_PD_PHYS.load(Ordering::Relaxed);
            Some((start, start + 4 * 1024))
        }

        #[cfg(not(any(target_arch = "arm", target_arch = "x86")))]
        {
            None
        }
    };

    MemoryLayout {
        kernel_end,
        heap_start,
        heap_end,
        page_alloc_start,
        page_alloc_end,
        page_table,
    }
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

struct MemoryLayout {
    kernel_end: usize,
    heap_start: usize,
    heap_end: usize,
    page_alloc_start: usize,
    page_alloc_end: usize,
    page_table: Option<(usize, usize)>,
}

fn log_memory_layout(
    kernel_end: usize,
    heap_start: usize,
    heap_end: usize,
    page_start: usize,
    page_end: usize,
    page_table: Option<(usize, usize)>,
) {
    log::info!("Memory Layout:");
    log::info!("  Kernel End:     0x{:08x}", kernel_end);

    if let Some((pt_start, pt_end)) = page_table {
        log::info!(
            "  Page Table:     0x{:08x} - 0x{:08x} ({} B)",
            pt_start,
            pt_end,
            pt_end - pt_start
        );
    }

    log::info!(
        "  Heap:           0x{:08x} - 0x{:08x} ({} KB)",
        heap_start,
        heap_end,
        (heap_end - heap_start) / 1024
    );

    log::info!(
        "  Page Allocator: 0x{:08x} - 0x{:08x} ({} MB)",
        page_start,
        page_end,
        (page_end - page_start) / (1024 * 1024)
    );
}

// log_system_info
fn log_system_info() {
    log::info!("System Information:");
    log::info!("  Platform:     {}", Platform::name());
    log::info!("  Architecture: {}", Platform::arch());
    log::info!(
        "  Total RAM:    {} MB",
        Platform::total_ram() / (1024 * 1024)
    );

    if let Some(cmdline) = Platform::cmdline() {
        log::info!("  Command Line: {}", cmdline);
    }
}

// log_discovered_hardware
fn log_discovered_hardware() {
    log::info!("Discovered Hardware:");
    log::info!("  Memory Regions:");
    for region in Platform::memory_regions() {
        let type_str = match region.mem_type {
            MemoryType::Available => "Available",
            MemoryType::Reserved => "Reserved",
            MemoryType::Mmio => "MMIO",
            MemoryType::Kernel => "Kernel",
            MemoryType::Framebuffer => "Framebuffer",
        };
        log::info!(
            "    {:12} : 0x{:08x} - 0x{:08x} ({} KB)",
            type_str,
            region.base,
            region.base + region.size,
            region.size / 1024,
        );
    }

    log::info!("  Devices:");
    for device in Platform::devices() {
        match device.irq {
            Some(irq) => log::info!(
                "    {} ({}) @ 0x{:08x} IRQ {}",
                device.name,
                device.compatible,
                device.base_addr,
                irq
            ),
            None => log::info!(
                "    {} ({}) @ 0x{:08x}",
                device.name,
                device.compatible,
                device.base_addr
            ),
        }
    }
}

// log_available_devices
fn log_available_devices() {
    use crate::device_manager;
    let names: Vec<alloc::string::String> = {
        let mgr = device_manager().lock();
        mgr.list().cloned().collect()
    };

    log::info!("Registered Devices:");
    for name in &names {
        log::info!("  - {}", name);
    }
}
