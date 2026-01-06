unsafe extern "C" {
    static mut _free_memory_start: u8;
}

#[inline(always)]
pub fn get_ram_size() -> usize {
    const RAM_SIZE_ADDR: usize = 0x100000; // Hypothetical address for RAM size
    unsafe { core::ptr::read_volatile(RAM_SIZE_ADDR as *const usize) }
}

#[inline(always)]
pub fn get_ram_start() -> usize {
    core::ptr::addr_of!(_free_memory_start) as usize
}

#[inline(always)]
pub fn get_ram_end() -> usize {
    let start = core::ptr::addr_of!(_free_memory_start) as usize;
    start + 0x1400000 // 20MiB placeholder get_ram_size()
}
