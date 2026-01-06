use core::ptr::{self, NonNull};

/// Maximum supported order for buddy allocator (2^MAX_ORDER * min_block_size max block size)
pub const MAX_ORDER: usize = 10;

/// Free block in the buddy allocator's free list
#[repr(C)]
struct FreeBlock {
    next: *mut FreeBlock,
}

/// Header stored before each allocated block in the heap
#[repr(C)]
struct BlockHeader {
    /// Order of the allocated block (power-of-two)
    order: u8,
}

/// A general-purpose buddy allocator for heap memory.
///
/// The allocator splits memory into blocks of size `2^order * min_block_size`.
/// Each allocated block stores a `BlockHeader` before the user-visible memory
/// so that `free` can retrieve the order and merge buddies.
///
/// # Safety
/// All methods are `unsafe` because the allocator assumes exclusive access
/// to the memory range and proper alignment.
pub struct BuddyAllocator {
    /// Free lists for each order
    free_lists: [*mut FreeBlock; MAX_ORDER + 1],

    /// Base address of managed memory
    base_addr: usize,

    /// Total size of managed memory
    total_size: usize,

    /// Minimum allocatable block size
    min_block_size: usize,
}

impl BuddyAllocator {
    /// Creates a new uninitialized buddy allocator
    ///
    /// `min_block_size` must be a power of two.
    pub const fn new(min_block_size: usize) -> Self {
        BuddyAllocator {
            free_lists: [ptr::null_mut(); MAX_ORDER + 1],
            base_addr: 0,
            total_size: 0,
            min_block_size,
        }
    }

    /// Initializes the allocator over a contiguous memory range.
    ///
    /// # Safety
    /// - Caller must ensure this memory range is not used elsewhere.
    /// - Memory should be aligned to `min_block_size`.
    pub unsafe fn init(&mut self, start_addr: usize, end_addr: usize) {
        let start = (start_addr + self.min_block_size - 1) & !(self.min_block_size - 1);
        let end = end_addr & !(self.min_block_size - 1);

        self.base_addr = start;
        self.total_size = end - start;

        for i in 0..=MAX_ORDER {
            self.free_lists[i] = ptr::null_mut();
        }

        let mut current = start;
        while current + self.min_block_size <= end {
            let remaining = end - current;
            let mut order = MAX_ORDER;
            while order > 0 {
                let block_size = self.min_block_size << order;
                if remaining >= block_size && (current & (block_size - 1)) == 0 {
                    unsafe {
                        self.add_to_free_list(current, order);
                    }
                    current += block_size;
                    break;
                }
                order -= 1;
            }

            if order == 0 && remaining >= self.min_block_size {
                unsafe {
                    self.add_to_free_list(current, 0);
                }
                current += self.min_block_size;
            }
        }
    }

    /// Allocates a block of at least `size` bytes.
    ///
    /// Returns a pointer to usable memory (after the header) or `None` if out of memory.
    ///
    /// # Safety
    /// Caller must not access the same memory from multiple threads without synchronization.
    pub unsafe fn alloc(&mut self, size: usize) -> Option<NonNull<u8>> {
        if size == 0 {
            return None;
        }

        // Include space for header
        let total_size = size + core::mem::size_of::<BlockHeader>();
        let mut order = 0;
        let mut block_size = self.min_block_size;
        while block_size < total_size {
            order += 1;
            block_size <<= 1;
        }

        if let Some(addr) = unsafe { self.alloc_block_order(order) } {
            let header_ptr = addr as *mut BlockHeader;
            unsafe {
                (*header_ptr).order = order as u8;
                return Some(NonNull::new_unchecked(
                    (addr + core::mem::size_of::<BlockHeader>()) as *mut u8,
                ));
            }
        }

        None
    }

    /// Frees a block previously allocated with `alloc`.
    ///
    /// # Safety
    /// - `ptr` must have been returned by a prior `alloc` call.
    /// - Must not be double-freed.
    pub unsafe fn free(&mut self, ptr: *mut u8) {
        if ptr.is_null() {
            return;
        }

        let header_addr = (ptr as usize) - core::mem::size_of::<BlockHeader>();
        let header_ptr = header_addr as *mut BlockHeader;

        unsafe {
            let order = (*header_ptr).order as usize;
            self.free_block(header_addr, order);
        }
    }

    /* ---------------- Block-level alloc/free ---------------- */

    /// Allocates a single block of the minimum size (order 0).
    ///
    /// This is the simplest allocation, equivalent to a single `min_block_size` block.
    /// Internally calls `alloc_block_order(0)`.
    ///
    /// # Safety
    /// - Caller must ensure exclusive access to the allocator.
    /// - Returned address must not be accessed concurrently without synchronization.
    ///
    /// # Returns
    /// - `Some(addr)` containing the base address of the allocated block.
    /// - `None` if no free blocks are available.
    pub(in crate::mm) unsafe fn alloc_block(&mut self) -> Option<usize> {
        unsafe { self.alloc_block_order(0) }
    }

    /// Allocates a block of the specified `order` (2^order * min_block_size bytes).
    ///
    /// If no free block of the requested order exists, attempts to split
    /// a larger block into smaller blocks until a block of the requested
    /// order is obtained.
    ///
    /// # Parameters
    /// - `order`: The power-of-two order of the block to allocate.
    ///
    /// # Safety
    /// - Caller must ensure exclusive access to the allocator.
    ///
    /// # Returns
    /// - `Some(addr)` containing the base address of the allocated block.
    /// - `None` if no suitable block can be allocated.
    pub(in crate::mm) unsafe fn alloc_block_order(&mut self, order: usize) -> Option<usize> {
        if order > MAX_ORDER {
            return None;
        }

        if !self.free_lists[order].is_null() {
            return Some(unsafe { self.remove_from_free_list(order) });
        }

        for higher_order in (order + 1)..=MAX_ORDER {
            if !self.free_lists[higher_order].is_null() {
                let block = unsafe { self.remove_from_free_list(higher_order) };
                for split_order in ((order + 1)..=higher_order).rev() {
                    let buddy = block + (self.min_block_size << (split_order - 1));
                    unsafe {
                        self.add_to_free_list(buddy, split_order - 1);
                    }
                }
                return Some(block);
            }
        }

        None
    }

    /// Frees a block of memory at `addr` of the specified `order`.
    ///
    /// Attempts to merge the block with its buddy if the buddy is free,
    /// recursively increasing the order until the largest possible block
    /// is returned to the free list.
    ///
    /// # Parameters
    /// - `addr`: Base address of the block to free.
    /// - `order`: Order of the block being freed.
    ///
    /// # Safety
    /// - Caller must ensure that `addr` and `order` correspond to a previously
    ///   allocated block and that it is not double-freed.
    pub(in crate::mm) unsafe fn free_block(&mut self, addr: usize, order: usize) {
        if order > MAX_ORDER {
            return;
        }

        let mut current_addr = addr;
        let mut current_order = order;

        while current_order < MAX_ORDER {
            let block_size = self.min_block_size << current_order;
            let buddy_addr = current_addr ^ block_size;

            if buddy_addr < self.base_addr || buddy_addr >= self.base_addr + self.total_size {
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

    /* ---------------- Internal helpers ---------------- */

    /// Adds a block to the free list of the given order
    unsafe fn add_to_free_list(&mut self, addr: usize, order: usize) {
        let block = addr as *mut FreeBlock;
        unsafe {
            (*block).next = self.free_lists[order];
        }
        self.free_lists[order] = block;
    }

    /// Removes and returns a block from the free list of the given order
    unsafe fn remove_from_free_list(&mut self, order: usize) -> usize {
        let block = self.free_lists[order];
        unsafe {
            self.free_lists[order] = (*block).next;
        }
        block as usize
    }

    /// Removes a specific block from the free list of the given order.
    ///
    /// Returns true if the block was found and removed.
    unsafe fn remove_specific_from_free_list(&mut self, addr: usize, order: usize) -> bool {
        let mut current = &mut self.free_lists[order];

        while !(*current).is_null() {
            if *current as usize == addr {
                unsafe {
                    *current = (**current).next;
                }
                return true;
            }
            unsafe {
                current = &mut (**current).next;
            }
        }

        false
    }
}
