use drivers::platform::{CurrentPlatform as Platform, Platform as PlatformTrait};

use crate::mm::{heap_allocator, page_allocator::PAGE_ALLOCATOR};

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
        Platform::early_init();
        Platform::init_console(115200).unwrap();
        Platform::init_interrupts();
        Platform::init_timer();

        // Query Hardware Configuration
        let (ram_base, ram_size) =
            Platform::query_ram_size().expect("Failed to query RAM size from firmware");

        let free_mem_start = core::ptr::addr_of!(_free_memory_start) as usize;
        let free_mem_end = ram_base + ram_size;

        PAGE_ALLOCATOR.init(free_mem_start, free_mem_end);

        let heap_start = core::ptr::addr_of!(_kernel_heap_start) as usize;
        let heap_end = core::ptr::addr_of!(_kernel_heap_end) as usize;
        heap_allocator::init_heap(heap_start, heap_end);

        Platform::console_write("Early init done\n");
    }
}
