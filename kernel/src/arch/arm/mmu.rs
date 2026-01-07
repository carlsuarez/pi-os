use crate::{
    kcore::sync::SpinLock,
    mm::page_table::{L1Table, L2Table},
};
use core::ptr;
use drivers::hw::bcm2835::PERIPHERAL_BASE;

/// Error types for MMU operations
#[derive(Debug, Clone, Copy)]
pub enum MmuError {
    InvalidL1Entry,
    InvalidPageIndex,
}

/// Constants
pub const NUM_L1_ENTRIES: usize = 4096;
pub const SECTION_SIZE: usize = 0x100000;
pub const SECTION_MASK: usize = 0xFFF00000;
pub const PAGE_MASK: usize = 0xFFFFF000;
pub const PAGE_OFFSET_MASK: usize = 0xFFF;

pub const L2_TYPE_SMALL: u32 = 2;

// ARMv6 Access Permissions (AP[2:0])
// AP[2] is in bit 15 (APX), AP[1:0] in bits [11:10]
pub const AP_NO_ACCESS: u32 = 0b000; // No access
pub const AP_PRIV_RW: u32 = 0b001; // Privileged RW, User no access
pub const AP_PRIV_RW_USER_RO: u32 = 0b010; // Privileged RW, User RO
pub const AP_FULL: u32 = 0b011; // Full access (RW for all)
pub const AP_PRIV_RO: u32 = 0b101; // Privileged RO, User no access
pub const AP_ALL_RO: u32 = 0b111; // Read-only for all

pub const DOMAIN_KERNEL: u32 = 0;
pub const DOMAIN_USER: u32 = 1;
pub const DOMAIN_HW: u32 = 2;

// Memory type constants (TEX, C, B)
pub const MEM_STRONGLY_ORDERED: u32 = (0b000 << 12) | (0 << 3) | (0 << 2);
pub const MEM_DEVICE: u32 = (0b000 << 12) | (0 << 3) | (1 << 2);
pub const MEM_NORMAL_UNCACHED: u32 = (0b001 << 12) | (0 << 3) | (0 << 2);
pub const MEM_NORMAL_WRITEBACK: u32 = (0b001 << 12) | (1 << 3) | (1 << 2);

// Assembly-declared static for kernel L1 page table
unsafe extern "C" {
    static mut l1_page_table: [u32; 4096];
}

static KERNEL_L1_LOCK: SpinLock<()> = SpinLock::new(());

/// Compute section descriptor for Normal memory (cacheable write-back)
#[inline(always)]
fn section_entry_normal(phys_addr: usize, ap: u32, domain: u32) -> u32 {
    let base = (phys_addr & SECTION_MASK) as u32;
    let ap_bits = ((ap & 0x4) << 13) | ((ap & 0x3) << 10); // APX in bit 15, AP[1:0] in [11:10]

    base
        | MEM_NORMAL_WRITEBACK  // TEX=001, C=1, B=1
        | ap_bits               // Access permissions
        | (domain << 5)         // Domain
        | (0 << 4)              // XN=0 (executable)
        | 0b10 // Section descriptor
}

/// Compute section descriptor for Device memory (MMIO)
#[inline(always)]
fn section_entry_device(phys_addr: usize, ap: u32, domain: u32) -> u32 {
    let base = (phys_addr & SECTION_MASK) as u32;
    let ap_bits = ((ap & 0x4) << 13) | ((ap & 0x3) << 10);

    base
        | MEM_DEVICE
        | ap_bits
        | (domain << 5)
        | (1 << 4)              // XN=1 (not executable for device memory)
        | 0b10
}

/// Compute coarse page table descriptor (points to L2 table)
#[inline(always)]
pub fn coarse_entry(l2_phys: usize, domain: u32) -> u32 {
    let base = (l2_phys & 0xFFFFFC00) as u32; // L2 table must be 1KB aligned
    base | (domain << 5) | 0b01 // Coarse page table descriptor
}

/// Compute L2 small page descriptor (4KB pages)
#[inline(always)]
pub fn l2_page_entry(phys_addr: usize, ap: u32) -> u32 {
    let base = (phys_addr & PAGE_MASK) as u32;
    let ap_bits = ((ap & 0x4) << 7) | ((ap & 0x3) << 4);

    base
        | ap_bits
        | (1 << 3)              // C=1
        | (1 << 2)              // B=1 (write-back)
        | (0 << 6)              // TEX=0
        | 0b10 // Small page (4KB)
}

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
pub fn is_valid_l1_section_entry(entry: u32) -> bool {
    entry & 0x3 == 0x2
}

#[inline(always)]
pub fn is_valid_l1_coarse_entry(entry: u32) -> bool {
    entry & 0x3 == 0x1
}

/// Get an L1 table reference to the kernel's page table
///# Safety
/// Caller must ensure exclusive access if modifying entries.
pub unsafe fn get_kernel_l1_table() -> L1Table {
    L1Table::new(core::ptr::addr_of_mut!(l1_page_table) as usize)
}

/// Execute a closure with exclusive access to the kernel L1 page table
///
/// # Safety
/// Caller must ensure no deadlocks occur if called from interrupt context
/// or nested calls.
pub fn with_kernel_l1<F, R>(f: F) -> R
where
    F: FnOnce(&mut L1Table) -> R,
{
    let _guard = KERNEL_L1_LOCK.lock();
    unsafe { f(&mut get_kernel_l1_table()) }
}

/// Set a single L1 entry in the kernel page table
///
/// # Safety
/// Caller must ensure:
/// - The entry is valid for the virtual address
/// - TLB is invalidated if MMU is already enabled
/// - No concurrent access to the same entry
pub unsafe fn set_kernel_l1_entry(va: usize, entry: u32) {
    unsafe {
        let l1: *mut u32 = &raw mut l1_page_table[l1_index(va)];
        ptr::write_volatile(l1, entry);
    }
}

/// Initialize the kernel's L1 page table (called from assembly before MMU is enabled)
///
/// This sets up the initial identity mapping for:
/// - 256MB of RAM starting at 0x0 (kernel code, data, stacks, heap)
/// - 16MB of peripheral space starting at PERIPHERAL_BASE (MMIO)
#[unsafe(no_mangle)]
pub unsafe extern "C" fn init_kernel_page_table() {
    unsafe {
        let l1: *mut u32 = &raw mut l1_page_table[0];

        // Clear entire L1 table
        for i in 0..NUM_L1_ENTRIES {
            ptr::write_volatile(l1.add(i), 0);
        }

        // Map entire first 256MB as normal memory (covers kernel, stacks, etc.)
        let (base, size) = drivers::hw::bcm2835::firmware_memory::get_arm_memory()
            .expect("Failed to get ARM memory from firmware\n");
        for i in base..(base + size) / SECTION_SIZE {
            let addr = i * SECTION_SIZE;
            ptr::write_volatile(
                l1.add(l1_index(addr)),
                section_entry_normal(addr, AP_PRIV_RW, DOMAIN_KERNEL),
            );
        }

        // Identity map hardware sections as Device memory
        // Map entire peripheral region (0x20000000 - 0x20FFFFFF, 16MB)
        for i in 0..16 {
            let addr = PERIPHERAL_BASE + (i * SECTION_SIZE);
            ptr::write_volatile(
                l1.add(l1_index(addr)),
                section_entry_device(addr, AP_PRIV_RW, DOMAIN_HW),
            );
        }
    }
}

/// Map a single page in an L2 table
///
/// # Safety
/// The L2 table must be properly initialized and aligned.
pub fn map_page(l2_table: &mut L2Table, va: usize, phys_addr: usize, ap: u32) {
    unsafe {
        let l2_ptr = l2_table.base() as *mut u32;
        ptr::write_volatile(l2_ptr.add(l2_index(va)), l2_page_entry(phys_addr, ap));
    }
}

/// Install an L2 table into an L1 entry
pub fn install_l2_table(l1_table: &mut L1Table, va: usize, l2_table: &L2Table, domain: u32) {
    let entry = coarse_entry(l2_table.base(), domain);
    l1_table.set_entry(l1_index(va), entry);
    invalidate_tlb_entry(va);
}

/// Set a single L1 entry
pub fn set_l1_entry(l1_table: &mut L1Table, va: usize, entry: u32) {
    l1_table.set_entry(l1_index(va), entry);
}

/// Invalidate a single TLB entry by virtual address
#[inline(always)]
pub fn invalidate_tlb_entry(va: usize) {
    unsafe {
        core::arch::asm!(
            "mcr p15, 0, {}, c8, c7, 1",  // TLBIMVA - invalidate by MVA
            in(reg) va,
            options(nostack)
        );
    }
}

/// Invalidate entire TLB
#[inline(always)]
pub fn invalidate_tlb_all() {
    unsafe {
        core::arch::asm!(
            "mov r0, #0",
            "mcr p15, 0, r0, c8, c7, 0", // TLBIALL - invalidate all
            options(nostack)
        );
    }
}
