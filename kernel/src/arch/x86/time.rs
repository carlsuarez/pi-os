fn rdtsc() -> u64 {
    let low: u32;
    let high: u32;
    unsafe {
        core::arch::asm!("rdtsc", out("eax") low, out("edx") high);
    }
    ((high as u64) << 32) | low as u64
}

pub fn delay_cycles(cycles: u64) {
    let start = rdtsc();
    while rdtsc() - start < cycles {
        core::hint::spin_loop();
    }
}
