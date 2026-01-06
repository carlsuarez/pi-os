use crate::mm::page_allocator::BUDDY_STORAGE;
use core::ptr::NonNull;

#[cfg(debug_assertions)]
mod debug {
    use core::sync::atomic::{AtomicBool, Ordering};

    /// Tracks whether an allocation has been freed to detect double frees.
    pub struct AllocFlag {
        freed: AtomicBool,
    }

    impl AllocFlag {
        pub const fn new() -> Self {
            Self {
                freed: AtomicBool::new(false),
            }
        }

        /// Marks the allocation as freed. Panics if double free detected.
        pub fn mark_freed(&self) {
            if self.freed.swap(true, Ordering::SeqCst) {
                panic!("double free detected");
            }
        }
    }
}

#[cfg(not(debug_assertions))]
mod debug {
    /// Dummy flag for non-debug builds.
    pub struct AllocFlag;
    impl AllocFlag {
        pub const fn new() -> Self {
            Self
        }
        pub fn mark_freed(&self) {}
    }
}

/*
 * RAII allocation types
 */

/// Represents a single allocated page.
pub struct Page {
    addr: NonNull<u8>,
    flag: debug::AllocFlag,
}

impl Page {
    pub fn new(addr: usize) -> Self {
        Self {
            addr: NonNull::new(addr as *mut u8).unwrap(),
            flag: debug::AllocFlag::new(),
        }
    }

    /// Returns the physical address of the page.
    pub fn addr(&self) -> usize {
        self.addr.as_ptr() as usize
    }
}

impl Drop for Page {
    /// Frees the page when it goes out of scope.
    fn drop(&mut self) {
        self.flag.mark_freed();
        unsafe {
            let storage_ptr = core::ptr::addr_of!(BUDDY_STORAGE);
            let alloc = &*(*storage_ptr).as_ptr();
            let mut guard = alloc.lock();
            guard.free_block(self.addr(), 0);
        }
    }
}

/// Represents a block of pages of size `2^ORDER`.
pub struct PageBlock<const ORDER: usize> {
    addr: NonNull<u8>,
    flag: debug::AllocFlag,
}

impl<const ORDER: usize> PageBlock<ORDER> {
    pub fn new(addr: usize) -> Self {
        Self {
            addr: NonNull::new(addr as *mut u8).unwrap(),
            flag: debug::AllocFlag::new(),
        }
    }

    /// Returns the base physical address of the block.
    pub fn addr(&self) -> usize {
        self.addr.as_ptr() as usize
    }
}

impl<const ORDER: usize> Drop for PageBlock<ORDER> {
    fn drop(&mut self) {
        self.flag.mark_freed();
        unsafe {
            let storage_ptr = core::ptr::addr_of!(BUDDY_STORAGE);
            let alloc = &*(*storage_ptr).as_ptr();
            let mut guard = alloc.lock();
            guard.free_block(self.addr(), ORDER);
        }
    }
}

/// Represents an L1 page table (8 KiB, order = 2).
pub struct L1Table {
    addr: NonNull<u32>,
    flag: debug::AllocFlag,
}

impl L1Table {
    pub fn new(addr: usize) -> Self {
        Self {
            addr: NonNull::new(addr as *mut u32).unwrap(),
            flag: debug::AllocFlag::new(),
        }
    }

    /// Returns the base address of the L1 table.
    pub fn base(&self) -> usize {
        self.addr.as_ptr() as usize
    }

    /// Set an entry at the given index (0..4095)
    pub fn set_entry(&mut self, index: usize, value: u32) {
        assert!(index < 4096, "L1Table index out of bounds");
        unsafe { self.addr.as_ptr().add(index).write_volatile(value) }
    }

    /// Get an entry at the given index
    pub fn get_entry(&self, index: usize) -> u32 {
        assert!(index < 4096, "L1Table index out of bounds");
        unsafe { self.addr.as_ptr().add(index).read_volatile() }
    }
}

impl Drop for L1Table {
    fn drop(&mut self) {
        self.flag.mark_freed();
        unsafe {
            let storage_ptr = core::ptr::addr_of!(BUDDY_STORAGE);
            let alloc = &*(*storage_ptr).as_ptr();
            let mut guard = alloc.lock();
            guard.free_block(self.base(), 2);
        }
    }
}

/// Represents an L2 page table (single page).
pub struct L2Table {
    addr: NonNull<u32>,
    flag: debug::AllocFlag,
}

impl L2Table {
    pub fn new(addr: usize) -> Self {
        Self {
            addr: NonNull::new(addr as *mut u32).unwrap(),
            flag: debug::AllocFlag::new(),
        }
    }

    /// Set an entry at the given index (0..255)
    pub fn set_entry(&mut self, index: usize, value: u32) {
        assert!(index < 256, "L2Table index out of bounds");
        unsafe { self.addr.as_ptr().add(index).write_volatile(value) }
    }

    /// Get an entry at the given index
    pub fn get_entry(&self, index: usize) -> u32 {
        assert!(index < 256, "L2Table index out of bounds");
        unsafe { self.addr.as_ptr().add(index).read_volatile() }
    }

    /// Returns the base address of the L2 table.
    pub fn base(&self) -> usize {
        self.addr.as_ptr() as usize
    }
}

impl Drop for L2Table {
    fn drop(&mut self) {
        self.flag.mark_freed();
        unsafe {
            let storage_ptr = core::ptr::addr_of!(BUDDY_STORAGE);
            let alloc = &*(*storage_ptr).as_ptr();
            let mut guard = alloc.lock();
            guard.free_block(self.base(), 0);
        }
    }
}
