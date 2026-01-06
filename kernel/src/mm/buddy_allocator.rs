use core::ptr;

/// Maximum supported order (2^MAX_ORDER * PAGE_SIZE = max block size).
/// Here, MAX_ORDER = 10 allows blocks up to 4 MiB (2^10 * 4 KiB).
const MAX_ORDER: usize = 10;
const PAGE_SIZE: usize = 4096;
const L1_TABLE_SIZE: usize = 16384; // 16 KiB for L1 page table
const L2_TABLE_SIZE: usize = 1024; // 1 KiB for L2 page table

/// Represents a free block in the buddy allocator's free list.
#[repr(C)]
struct FreeBlock {
    next: *mut FreeBlock,
}

/// A simple buddy allocator for managing physical memory in powers-of-two blocks.
///
/// The allocator maintains free lists for each order (from 0 to MAX_ORDER),
/// where each order represents blocks of size `2^order * PAGE_SIZE`.
///
/// Supports allocating single pages, multiple-page blocks, and merging
/// freed blocks with their buddies to reduce fragmentation.
///
/// # Safety
/// All public methods are unsafe because they assume that addresses and
/// memory ranges are managed exclusively by the allocator, and that
/// caller code respects page alignment.
#[repr(C)]
pub struct BuddyAllocator {
    /// Array of free lists for each order.
    free_lists: [*mut FreeBlock; MAX_ORDER + 1],

    /// Base physical address of memory managed by this allocator.
    base_addr: usize,

    /// Total number of managed pages.
    total_pages: usize,
}

impl BuddyAllocator {
    /// Creates a new uninitialized `BuddyAllocator`.
    ///
    /// Use `init` to set up memory ranges before allocating pages.
    pub(in crate::mm) const fn new() -> Self {
        BuddyAllocator {
            free_lists: [ptr::null_mut(); MAX_ORDER + 1],
            base_addr: 0,
            total_pages: 0,
        }
    }

    /// Initializes the buddy allocator over a memory range.
    ///
    /// # Safety
    /// Caller must ensure `start_addr` and `end_addr` define valid physical memory
    /// and that no other code accesses this range.
    ///
    /// Memory is automatically split into the largest aligned blocks possible
    /// and added to free lists.
    pub(in crate::mm) unsafe fn init(&mut self, start_addr: usize, end_addr: usize) {
        let start = (start_addr + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
        let end = end_addr & !(PAGE_SIZE - 1);

        self.base_addr = start;
        self.total_pages = (end - start) / PAGE_SIZE;

        // Initialize free lists
        for i in 0..=MAX_ORDER {
            self.free_lists[i] = ptr::null_mut();
        }

        // Add blocks to free lists
        let mut current = start;
        while current + PAGE_SIZE <= end {
            let remaining = end - current;

            // Find largest fitting block
            let mut order = MAX_ORDER;
            while order > 0 {
                let block_size = PAGE_SIZE << order;
                if remaining >= block_size && (current & (block_size - 1)) == 0 {
                    unsafe {
                        self.add_to_free_list(current, order);
                    }
                    current += block_size;
                    break;
                }
                order -= 1;
            }

            // Add single page if no larger block fits
            if order == 0 && remaining >= PAGE_SIZE {
                unsafe {
                    self.add_to_free_list(current, 0);
                }
                current += PAGE_SIZE;
            }
        }
    }

    /// Allocates a block of `2^order` pages.
    ///
    /// Returns the base address of the allocated block or `None` if out of memory.
    pub(in crate::mm) unsafe fn alloc_pages(&mut self, order: usize) -> Option<usize> {
        if order > MAX_ORDER {
            return None;
        }

        // Check free list for requested order
        if !self.free_lists[order].is_null() {
            return Some(unsafe { self.remove_from_free_list(order) });
        }

        // Try to split a larger block
        for higher_order in (order + 1)..=MAX_ORDER {
            if !self.free_lists[higher_order].is_null() {
                let block = unsafe { self.remove_from_free_list(higher_order) };

                // Split down to requested order
                for split_order in ((order + 1)..=higher_order).rev() {
                    let buddy = block + (PAGE_SIZE << (split_order - 1));
                    unsafe {
                        self.add_to_free_list(buddy, split_order - 1);
                    }
                }

                return Some(block);
            }
        }

        None
    }

    /// Frees a block of `2^order` pages and attempts to merge with buddy blocks.
    pub(in crate::mm) unsafe fn free_pages(&mut self, addr: usize, order: usize) {
        if order > MAX_ORDER {
            return;
        }

        let mut current_addr = addr;
        let mut current_order = order;

        while current_order < MAX_ORDER {
            let block_size = PAGE_SIZE << current_order;
            let buddy_addr = current_addr ^ block_size;

            if buddy_addr < self.base_addr
                || buddy_addr >= self.base_addr + (self.total_pages * PAGE_SIZE)
            {
                break;
            }

            if unsafe { self.remove_specific_from_free_list(buddy_addr, current_order) } {
                current_addr = current_addr.min(buddy_addr);
                current_order += 1;
            } else {
                break;
            }
        }

        unsafe {
            self.add_to_free_list(current_addr, current_order);
        }
    }

    /// Allocates a single 4 KB page.
    pub(in crate::mm) unsafe fn alloc_page(&mut self) -> Option<usize> {
        unsafe { self.alloc_pages(0) }
    }

    /// Frees a single 4 KB page.
    pub(in crate::mm) unsafe fn free_page(&mut self, addr: usize) {
        unsafe {
            self.free_pages(addr, 0);
        }
    }

    /* ------------------------------------------------------------
     * Internal helpers
     * ------------------------------------------------------------
     */

    /// Adds a block to the free list of a given order.
    unsafe fn add_to_free_list(&mut self, addr: usize, order: usize) {
        let block = addr as *mut FreeBlock;
        unsafe {
            (*block).next = self.free_lists[order];
        }
        self.free_lists[order] = block;
    }

    /// Removes and returns a block from the free list of a given order.
    unsafe fn remove_from_free_list(&mut self, order: usize) -> usize {
        let block = self.free_lists[order];
        unsafe {
            self.free_lists[order] = (*block).next;
        }
        block as usize
    }

    /// Removes a specific block from the free list of a given order.
    ///
    /// Returns true if the block was found and removed.
    unsafe fn remove_specific_from_free_list(&mut self, addr: usize, order: usize) -> bool {
        let mut current = &mut self.free_lists[order];

        while !(*current).is_null() {
            unsafe {
                if *current as usize == addr {
                    *current = (**current).next;
                    return true;
                }
                current = &mut (**current).next;
            }
        }

        false
    }
}
