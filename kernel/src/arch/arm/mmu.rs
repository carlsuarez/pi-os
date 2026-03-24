use crate::mm::mmu::{MapFlags, MmuOps};
use core::ptr::write_volatile;
use drivers::platform::{CurrentPlatform, Platform};

// ============================================================================
// Constants
// ============================================================================

pub const NUM_L1_ENTRIES: usize = 4096;
pub const SECTION_SIZE: usize = 0x100000;
pub const SECTION_MASK: usize = 0xFFF00000;
pub const PAGE_MASK: usize = 0xFFFFF000;

// Access permission encodings (AP[2:0])
pub const AP_NO_ACCESS: u32 = 0b000;
pub const AP_PRIV_RW: u32 = 0b001;
pub const AP_PRIV_RW_USER_RO: u32 = 0b010;
pub const AP_FULL: u32 = 0b011;
pub const AP_PRIV_RO: u32 = 0b101;
pub const AP_ALL_RO: u32 = 0b111;

pub const DOMAIN_KERNEL: u32 = 0;
pub const DOMAIN_USER: u32 = 1;
pub const DOMAIN_HW: u32 = 2;

// Memory type encodings (TEX, C, B)
pub const MEM_STRONGLY_ORDERED: u32 = (0b000 << 12) | (0 << 3) | (0 << 2);
pub const MEM_DEVICE: u32 = (0b000 << 12) | (0 << 3) | (1 << 2);
pub const MEM_NORMAL_UNCACHED: u32 = (0b001 << 12) | (0 << 3) | (0 << 2);
pub const MEM_NORMAL_WRITEBACK: u32 = (0b001 << 12) | (1 << 3) | (1 << 2);

unsafe extern "C" {
    static _vectors: u8;
}

// ============================================================================
// Entry constructors
// ============================================================================

#[inline(always)]
fn ap_bits(ap: u32) -> u32 {
    ((ap & 0x4) << 13) | ((ap & 0x3) << 10)
}

#[inline(always)]
fn section_entry(phys_addr: usize, mem_type: u32, ap: u32, domain: u32, exec: bool) -> u32 {
    let xn = if exec { 0 } else { 1 << 4 };
    ((phys_addr & SECTION_MASK) as u32) | mem_type | ap_bits(ap) | (domain << 5) | xn | 0b10
}

#[inline(always)]
pub fn coarse_entry(l2_phys: usize, domain: u32) -> u32 {
    ((l2_phys & 0xFFFFFC00) as u32) | (domain << 5) | 0b01
}

#[inline(always)]
pub fn l2_page_entry(phys_addr: usize, ap: u32) -> u32 {
    let base = (phys_addr & PAGE_MASK) as u32;
    let ap_l2 = ((ap & 0x4) << 7) | ((ap & 0x3) << 4);
    base | ap_l2 | (1 << 3) | (1 << 2) | 0b10
}

// ============================================================================
// Index helpers
// ============================================================================

#[inline(always)]
pub fn l1_index(va: usize) -> usize {
    va >> 20
}

#[inline(always)]
pub fn l2_index(va: usize) -> usize {
    (va >> 12) & 0xFF
}

#[inline(always)]
pub fn coarse_base(l1_entry: u32) -> usize {
    (l1_entry & 0xFFFFFC00) as usize
}

#[inline(always)]
pub fn is_section_entry(entry: u32) -> bool {
    entry & 0x3 == 0x2
}

#[inline(always)]
pub fn is_coarse_entry(entry: u32) -> bool {
    entry & 0x3 == 0x1
}

// ============================================================================
// ArmMmu
// ============================================================================

pub struct ArmMmu;

impl MmuOps for ArmMmu {
    /// Populate the L1 page table at l1_phys and enable the MMU.
    /// l1_phys must point to a zeroed, 16KB-aligned physical region.
    unsafe fn init(l1_phys: usize) {
        let l1 = l1_phys as *mut u32;
        let mm = CurrentPlatform::memory_map();

        // Map RAM as Normal Write-Back
        let ram_start = mm.ram_start & SECTION_MASK;
        let ram_end = (mm.ram_start + mm.ram_size + SECTION_SIZE - 1) & SECTION_MASK;
        let mut addr = ram_start;
        while addr < ram_end {
            write_volatile(
                l1.add(l1_index(addr)),
                section_entry(addr, MEM_NORMAL_WRITEBACK, AP_PRIV_RW, DOMAIN_KERNEL, true),
            );
            addr += SECTION_SIZE;
        }

        // Ensure the vectors section is mapped executable
        let v = (core::ptr::addr_of!(_vectors) as usize) & SECTION_MASK;
        write_volatile(
            l1.add(l1_index(v)),
            section_entry(v, MEM_NORMAL_WRITEBACK, AP_PRIV_RW, DOMAIN_KERNEL, true),
        );

        // Map peripherals as Device (non-cacheable, execute-never)
        let periph_start = mm.peripheral_base & SECTION_MASK;
        let periph_end =
            (mm.peripheral_base + mm.peripheral_size + SECTION_SIZE - 1) & SECTION_MASK;
        addr = periph_start;
        while addr < periph_end {
            write_volatile(
                l1.add(l1_index(addr)),
                section_entry(addr, MEM_DEVICE, AP_PRIV_RW, DOMAIN_HW, false),
            );
            addr += SECTION_SIZE;
        }

        enable_mmu(l1_phys);
    }

    unsafe fn map_region(virt: usize, phys: usize, size: usize, flags: MapFlags) {
        // Determine AP and memory type from flags
        let ap = if flags.contains(MapFlags::USER) {
            if flags.contains(MapFlags::WRITE) {
                AP_FULL
            } else {
                AP_PRIV_RW_USER_RO
            }
        } else {
            if flags.contains(MapFlags::WRITE) {
                AP_PRIV_RW
            } else {
                AP_PRIV_RO
            }
        };

        let mem_type = if flags.contains(MapFlags::DEVICE) {
            MEM_DEVICE
        } else if flags.contains(MapFlags::CACHED) {
            MEM_NORMAL_WRITEBACK
        } else {
            MEM_NORMAL_UNCACHED
        };

        let exec = flags.contains(MapFlags::EXEC);
        let domain = if flags.contains(MapFlags::USER) {
            DOMAIN_USER
        } else {
            DOMAIN_KERNEL
        };

        // Use the kernel L1 table published by init.rs
        let l1 = crate::kcore::init::KERNEL_L1_TABLE_PHYS
            .load(core::sync::atomic::Ordering::Relaxed) as *mut u32;

        let aligned_size = (size + SECTION_SIZE - 1) & SECTION_MASK;
        let mut offset = 0;
        while offset < aligned_size {
            write_volatile(
                l1.add(l1_index(virt + offset)),
                section_entry(phys + offset, mem_type, ap, domain, exec),
            );
            offset += SECTION_SIZE;
        }

        Self::invalidate_tlb_all();
    }

    unsafe fn unmap_region(virt: usize, size: usize) {
        let l1 = crate::kcore::init::KERNEL_L1_TABLE_PHYS
            .load(core::sync::atomic::Ordering::Relaxed) as *mut u32;

        let aligned_size = (size + SECTION_SIZE - 1) & SECTION_MASK;
        let mut offset = 0;
        while offset < aligned_size {
            write_volatile(l1.add(l1_index(virt + offset)), 0);
            Self::invalidate_tlb_entry(virt + offset);
            offset += SECTION_SIZE;
        }
    }

    #[inline(always)]
    unsafe fn invalidate_tlb_entry(va: usize) {
        core::arch::asm!(
            "mcr p15, 0, {va}, c8, c7, 1",
            va = in(reg) va,
            options(nostack),
        );
    }

    #[inline(always)]
    unsafe fn invalidate_tlb_all() {
        core::arch::asm!(
            "mov {t}, #0",
            "mcr p15, 0, {t}, c8, c7, 0",
            t = out(reg) _,
            options(nostack),
        );
    }
}

// ============================================================================
// MMU enable (private, ARM-only)
// ============================================================================

/// Load TTBR0, configure TTBCR/DACR, then enable MMU + caches.
///
/// # Safety
/// - ttbr0 must be the physical address of a valid fully-populated
///   16KB-aligned L1 page table.
/// - The caller's code must be identity-mapped in that table.
/// - Called exactly once before the MMU is enabled.
unsafe fn enable_mmu(ttbr0: usize) {
    core::arch::asm!(
        // Invalidate TLB before loading new TTBR0
        "mov     {t}, #0",
        "mcr     p15, 0, {t}, c8, c7, 0",      // TLBIALL

        // TTBR0: base | IRGN=WBWA (bit 6) | RGN=WBWA (bit 0)
        "orr     {b}, {b}, #(1 << 6)",
        "orr     {b}, {b}, #(1 << 0)",
        "mcr     p15, 0, {b}, c2, c0, 0",      // TTBR0

        // TTBCR = 0: use TTBR0 for all translations, N=0
        "mov     {t}, #0",
        "mcr     p15, 0, {t}, c2, c0, 2",      // TTBCR

        // DACR: domain 0 = client, all others = no-access
        "mov     {t}, #0x1",
        "mcr     p15, 0, {t}, c3, c0, 0",      // DACR

        // Clear AFE (bit 29) in SCTLR so AP[2:0] encoding is used
        "mrc     p15, 0, {t}, c1, c0, 0",
        "bic     {t}, {t}, #(1 << 29)",
        "mcr     p15, 0, {t}, c1, c0, 0",

        // DSB: ensure all table writes are visible to the page table walker
        "mov     {t}, #0",
        "mcr     p15, 0, {t}, c7, c10, 4",     // DSB

        // Enable MMU (bit 0) + D-cache (bit 2) + I-cache (bit 12)
        "mrc     p15, 0, {t}, c1, c0, 0",
        "orr     {t}, {t}, #(1 << 0)",
        "orr     {t}, {t}, #(1 << 2)",
        "orr     {t}, {t}, #(1 << 12)",
        "mcr     p15, 0, {t}, c1, c0, 0",

        // ISB: flush pipeline after SCTLR write
        "mov     {t}, #0",
        "mcr     p15, 0, {t}, c7, c5, 4",      // ISB

        b = in(reg) ttbr0,
        t = out(reg) _,
        options(nostack),
    );
}
