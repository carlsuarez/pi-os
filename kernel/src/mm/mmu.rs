bitflags::bitflags! {
    pub struct MapFlags: u32 {
        const READ   = 1 << 0;
        const WRITE  = 1 << 1;
        const EXEC   = 1 << 2;
        const USER   = 1 << 3;
        const DEVICE = 1 << 4;
        const CACHED = 1 << 5;
    }
}

pub trait MmuOps {
    /// One-time setup: populate page table from l1_phys, then enable the MMU.
    /// Must be called exactly once, before kernel_main, with a valid zeroed
    /// 16KB-aligned physical address in l1_phys.
    unsafe fn init(l1_phys: usize);

    /// Map a physically contiguous region into the kernel address space.
    /// size is rounded up to the nearest page/section boundary internally.
    unsafe fn map_region(virt: usize, phys: usize, size: usize, flags: MapFlags);

    /// Unmap a virtual region. Does not free any backing physical memory.
    unsafe fn unmap_region(virt: usize, size: usize);

    /// Invalidate a single TLB entry by virtual address.
    unsafe fn invalidate_tlb_entry(va: usize);

    /// Invalidate the entire TLB.
    unsafe fn invalidate_tlb_all();
}

// Stable re-export so callers never need a cfg themselves
#[cfg(target_arch = "arm")]
pub use crate::arch::arm::mmu::ArmMmu as PlatformMmu;

#[cfg(target_arch = "x86")]
pub use crate::arch::x86::mmu::X86Mmu as PlatformMmu;
