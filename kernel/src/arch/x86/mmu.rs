use core::{
    ptr,
    sync::atomic::{AtomicUsize, Ordering},
};

use crate::mm::mmu::{MapFlags, MmuOps};

/// Number of entries in a page directory or page table.
const PD_ENTRIES: usize = 1024;
const PT_ENTRIES: usize = 1024;

/// Size of one page (4 KB).
pub const PAGE_SIZE: usize = 4096;

/// Size of one page directory (4 KB).
const PD_SIZE: usize = PD_ENTRIES * core::mem::size_of::<u32>();

/// Size of one page table (4 KB).
const PT_SIZE: usize = PT_ENTRIES * core::mem::size_of::<u32>();

/// Number of page tables in the static pool.
const PT_POOL_COUNT: usize = 64;

/// Present: the entry is valid.
const X86_PRESENT: u32 = 1 << 0;
/// Writable.
const X86_WRITABLE: u32 = 1 << 1;
/// User-accessible (ring 3 can touch it).
const X86_USER: u32 = 1 << 2;
/// Write-through caching (vs. write-back).
const X86_PWT: u32 = 1 << 3;
/// Cache disable.
const X86_PCD: u32 = 1 << 4;
/// Accessed (set by CPU on first read/write).
const X86_ACCESSED: u32 = 1 << 5;
/// Dirty (set by CPU on first write, PTE only).
const X86_DIRTY: u32 = 1 << 6;
/// Page Size: PDE maps a 4 MB page instead of a PT frame (PDE only).
const X86_PS: u32 = 1 << 7;

/// Mask to extract the 20-bit physical base address from a PDE/PTE.
const PHYS_ADDR_MASK: u32 = 0xFFFF_F000;

// Static early-boot PT pool

/// 64 pre-zeroed page-table frames.  Each is 4 KB and 4 KB-aligned.
#[repr(C, align(4096))]
struct PtPool([u8; PT_SIZE * PT_POOL_COUNT]);

static mut PT_POOL: PtPool = PtPool([0u8; PT_SIZE * PT_POOL_COUNT]);

/// Index of the next free slot in PT_POOL (bumps upward, never freed).
static PT_POOL_NEXT: AtomicUsize = AtomicUsize::new(0);

/// Panics if the pool is exhausted.
fn alloc_pt_frame() -> *mut u32 {
    let idx = PT_POOL_NEXT.fetch_add(1, Ordering::Relaxed);
    assert!(idx < PT_POOL_COUNT, "x86 MMU: early PT pool exhausted");
    let byte_ptr = unsafe { core::ptr::addr_of!(PT_POOL).cast::<u8>().add(idx * PT_SIZE) };
    byte_ptr as *mut u32
}

#[inline(always)]
fn pd_index(va: usize) -> usize {
    (va >> 22) & 0x3FF
}

#[inline(always)]
fn pt_index(va: usize) -> usize {
    (va >> 12) & 0x3FF
}

#[inline(always)]
fn page_offset(va: usize) -> usize {
    va & 0xFFF
}

/// Translate our portable `MapFlags` into the x86 PTE bits.
///
/// Notes on x86 semantics:
///  - READ has no dedicated hardware bit; a present, kernel-mode PTE is
///    always readable by the kernel.  We simply require PRESENT.
///  - EXEC has no hardware bit in 32-bit non-PAE paging.  We record user
///    intent but cannot enforce it in hardware here.
///  - DEVICE implies uncached (PCD=1) and write-through disabled (PWT=1).
///  - CACHED (our flag) means "normal cacheable"; absence of DEVICE with
///    CACHED set uses write-back, which is the CPU default.
fn map_flags_to_x86(flags: MapFlags) -> u32 {
    let mut bits: u32 = X86_PRESENT;

    if flags.contains(MapFlags::WRITE) {
        bits |= X86_WRITABLE;
    }
    if flags.contains(MapFlags::USER) {
        bits |= X86_USER;
    }
    if flags.contains(MapFlags::DEVICE) {
        // Uncached, write-through for MMIO regions.
        bits |= X86_PCD | X86_PWT;
    }
    // CACHED with no DEVICE -> write-back, which is the default (no bits set).

    bits
}

/// Write a value to CR3 (page-directory base register).
///
/// This implicitly flushes the TLB entirely.
#[inline]
unsafe fn write_cr3(pd_phys: u32) {
    unsafe {
        core::arch::asm!(
            "mov cr3, {0}",
            in(reg) pd_phys,
            options(nostack, preserves_flags)
        );
    }
}

/// Read the current CR3 value.
#[inline]
unsafe fn read_cr3() -> u32 {
    let v: u32;
    unsafe {
        core::arch::asm!(
            "mov {0}, cr3",
            out(reg) v,
            options(nostack, nomem, preserves_flags)
        );
    }
    v
}

/// Enable paging by setting CR0 bit 31 (PG).
///
/// CR0 bit 0 (PE, protected mode) must already be set before calling this.
#[inline]
unsafe fn enable_paging() {
    unsafe {
        core::arch::asm!(
            "mov eax, cr0",
            "or  eax, 0x80000000",
            "mov cr0, eax",
            out("eax") _,
            options(nostack, preserves_flags)
        );
    }
}

/// Invalidate a single TLB entry via `INVLPG`.
#[inline]
unsafe fn invlpg(va: usize) {
    unsafe {
        core::arch::asm!(
            "invlpg [{0}]",
            in(reg) va,
            options(nostack, preserves_flags)
        );
    }
}

pub struct X86Mmu;

impl MmuOps for X86Mmu {
    /// One-time MMU initialisation.
    ///
    /// `l1_phys` must be the physical address of a **4 KB-aligned, zeroed**
    /// buffer (4 096 bytes) that will become the Page Directory.
    ///
    /// This function:
    ///  1. Points CR3 at `l1_phys` (does NOT yet enable paging).
    ///  2. Identity-maps the first 4 MB of physical memory (0x0000_0000 –
    ///     0x003F_FFFF) using a 4 MB PS PDE so the CPU can continue
    ///     fetching instructions from low addresses immediately after CR0.PG
    ///     is set.
    ///  3. Enables paging (CR0 bit 31).
    ///
    /// The caller is responsible for adding any additional mappings (kernel
    /// image, stack, device regions, …) via `map_region` **after** this
    /// returns, then removing the identity map if a higher-half layout is
    /// used.
    unsafe fn init(l1_phys: usize) {
        let pd = l1_phys as *mut u32;

        // Enable PSE
        unsafe {
            core::arch::asm!(
                "mov eax, cr4",
                "or  eax, 0x10",
                "mov cr4, eax",
                out("eax") _,
                options(nostack, preserves_flags)
            );

            // Identity-map first 4 MB via 4 MB PS PDE — covers the kernel at 1MB
            // and the PT pool which lives in BSS just after the kernel image.
            // With a debug build at ~4.4MB we need more than one 4MB PDE.

            // Map 0x00000000 - 0x00400000 (first 4MB)
            let pde0: u32 = 0x0000_0000 | X86_PS | X86_WRITABLE | X86_PRESENT;
            ptr::write_volatile(pd.add(0), pde0);

            // Map 0x00400000 - 0x00800000 (second 4MB) — covers BSS/PT_POOL
            // for debug builds where the kernel + pool exceeds 4MB
            let pde1: u32 = 0x0040_0000 | X86_PS | X86_WRITABLE | X86_PRESENT;
            ptr::write_volatile(pd.add(1), pde1);

            // Map VGA buffer at 0xB8000 — falls in first 4MB, already covered
            // by pde0. No extra mapping needed.

            write_cr3(l1_phys as u32);
            enable_paging();
        }
    }

    /// Map a physically contiguous region.
    ///
    /// Both `virt` and `phys` are rounded down to 4 KB boundaries.
    /// `size` is rounded up so that every byte in [virt, virt+size) is
    /// covered.
    ///
    /// Page tables are allocated from the static pool on demand (no heap).
    unsafe fn map_region(virt: usize, phys: usize, size: usize, flags: MapFlags) {
        let pte_bits = map_flags_to_x86(flags);

        // Align virt/phys down, size up.
        let virt_start = virt & !0xFFF;
        let phys_start = phys & !0xFFF;
        let pages = (size + page_offset(virt) + PAGE_SIZE - 1) / PAGE_SIZE;

        // CR3 lower 12 bits are flags; mask them off to get the PD base.
        let pd = (unsafe { read_cr3() } & PHYS_ADDR_MASK) as *mut u32;

        for i in 0..pages {
            let va = virt_start + i * PAGE_SIZE;
            let pa = phys_start + i * PAGE_SIZE;

            let pdi = pd_index(va);
            let pti = pt_index(va);

            //  Locate or create the PT for this PDE 
            let pde = unsafe { ptr::read_volatile(pd.add(pdi)) };
            let pt: *mut u32 = if pde & X86_PRESENT != 0 {
                // PT already exists.  Strip flags bits to get physical base.
                (pde & PHYS_ADDR_MASK) as *mut u32
            } else {
                // Allocate a fresh PT frame from the static pool.
                let pt_frame = alloc_pt_frame();

                // Write the PDE pointing at the new PT frame.
                // User bit in PDE must be set if *any* mapping under it is
                // user-accessible; conservatively set it here and rely on the
                // PTE user bit for the real enforcement.
                let new_pde: u32 = (pt_frame as u32) | X86_WRITABLE | X86_USER | X86_PRESENT;
                unsafe {
                    ptr::write_volatile(pd.add(pdi), new_pde);
                }

                pt_frame
            };

            // Write the PTE
            let pte: u32 = (pa as u32 & PHYS_ADDR_MASK) | pte_bits;
            unsafe {
                ptr::write_volatile(pt.add(pti), pte);

                // Invalidate the TLB entry for this page.
                invlpg(va);
            }
        }
    }

    /// Unmap a virtual region by clearing PTEs (present bit → 0).
    ///
    /// Does **not** free PT frames even if they become empty; this is safe
    /// because the pool is never reclaimed.  If you need to reclaim PT frames
    /// at runtime, extend this with a reference-count per PDE.
    unsafe fn unmap_region(virt: usize, size: usize) {
        let virt_start = virt & !0xFFF;
        let pages = (size + page_offset(virt) + PAGE_SIZE - 1) / PAGE_SIZE;

        let pd = unsafe { (read_cr3() & PHYS_ADDR_MASK) as *mut u32 };

        for i in 0..pages {
            let va = virt_start + i * PAGE_SIZE;
            let pdi = pd_index(va);
            let pti = pt_index(va);

            let pde = unsafe { ptr::read_volatile(pd.add(pdi)) };
            if pde & X86_PRESENT == 0 {
                // No PT for this region — nothing to do.
                continue;
            }

            // Skip 4 MB PS mappings: clearing a single PTE makes no sense.
            if pde & X86_PS != 0 {
                // Clear the entire 4 MB PDE instead.
                unsafe {
                    ptr::write_volatile(pd.add(pdi), 0);
                    invlpg(va);
                }
                continue;
            }

            let pt = (pde & PHYS_ADDR_MASK) as *mut u32;
            unsafe {
                ptr::write_volatile(pt.add(pti), 0);
                invlpg(va);
            }
        }
    }

    /// Invalidate a single TLB entry for virtual address `va`.
    #[inline]
    unsafe fn invalidate_tlb_entry(va: usize) {
        unsafe {
            invlpg(va);
        }
    }

    /// Invalidate the entire TLB by reloading CR3.
    ///
    /// This is the canonical x86 full TLB flush: a write to CR3 discards
    /// all non-global TLB entries.
    #[inline]
    unsafe fn invalidate_tlb_all() {
        unsafe {
            let cr3 = read_cr3();
            write_cr3(cr3);
        }
    }
}
