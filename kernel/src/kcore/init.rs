use crate::mm::{heap_allocator, page_allocator::PAGE_ALLOCATOR};
use alloc::vec::Vec;
use drivers::console::console_write;
use drivers::device_manager::devices;
use drivers::platform::{CurrentPlatform as Platform, Platform as PlatformTrait};

// ============================================================================
// Linker Symbols
// ============================================================================

unsafe extern "C" {
    static mut _free_memory_start: u8;
    static mut _kernel_heap_start: u8;
    static mut _kernel_heap_end: u8;
}

// ============================================================================
// Kernel Initialization
// ============================================================================

#[unsafe(no_mangle)]
pub extern "C" fn kernel_init() {
    unsafe {
        // ====================================================================
        // Stage 1: Early Platform Initialization
        // ====================================================================
        // Configure GPIO, clocks, and basic hardware before anything else
        Platform::early_init();

        // ====================================================================
        // Stage 2: Memory Management Setup
        // ====================================================================
        // Query hardware configuration
        let (ram_base, ram_size) =
            Platform::query_ram_size().expect("Failed to query RAM size from firmware");

        // Initialize page allocator for physical memory management
        let free_mem_start = core::ptr::addr_of!(_free_memory_start) as usize;
        let free_mem_end = ram_base + ram_size;
        PAGE_ALLOCATOR.init(free_mem_start, free_mem_end);

        // Initialize kernel heap for dynamic allocations
        let heap_start = core::ptr::addr_of!(_kernel_heap_start) as usize;
        let heap_end = core::ptr::addr_of!(_kernel_heap_end) as usize;
        heap_allocator::init_heap(heap_start, heap_end);

        // ====================================================================
        // Stage 3: Device Initialization
        // ====================================================================
        // Now that heap is available, initialize all platform devices
        // This includes: console UART, interrupts, timer, EMMC, framebuffer
        {
            let mut device_mgr = devices().lock();
            Platform::init_devices(&mut device_mgr).expect("Failed to initialize platform devices");
        }

        // ====================================================================
        // Stage 4: Verify Initialization
        // ====================================================================
        // Console is now available through device manager
        console_write("===========================================\n");
        console_write("Kernel Early Initialization Complete\n");
        console_write("===========================================\n");

        // Log system information
        log_system_info(ram_base, ram_size, heap_start, heap_end);

        // List available devices
        let names: Vec<alloc::string::String> = {
            let mgr = devices().lock();
            mgr.list().cloned().collect()
        };

        console_write("\nAvailable devices:\n");
        for name in names {
            console_write("  - ");
            console_write(&name);
            console_write("\n");
        }

        console_write("===========================================\n");
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Log system information during boot
fn log_system_info(ram_base: usize, ram_size: usize, heap_start: usize, heap_end: usize) {
    use alloc::format;

    console_write("\nSystem Information:\n");

    // RAM info
    let ram_mb = ram_size / (1024 * 1024);
    let msg = format!(
        "  RAM: {} MB (0x{:08x} - 0x{:08x})\n",
        ram_mb,
        ram_base,
        ram_base + ram_size
    );
    console_write(&msg);

    // Heap info
    let heap_size = heap_end - heap_start;
    let heap_kb = heap_size / 1024;
    let msg = format!(
        "  Kernel Heap: {} KB (0x{:08x} - 0x{:08x})\n",
        heap_kb, heap_start, heap_end
    );
    console_write(&msg);

    // Platform info
    let msg = format!("  Platform: {}\n", Platform::name());
    console_write(&msg);
}
