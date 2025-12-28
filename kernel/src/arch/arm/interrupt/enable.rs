#[inline(always)]
pub fn disable() {
    unsafe { core::arch::asm!("cpsid i", options(nomem, nostack)) }
}

#[inline(always)]
pub fn enable() {
    unsafe { core::arch::asm!("cpsie i", options(nomem, nostack)) }
}
