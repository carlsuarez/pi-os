use crate::kcore::sync::SpinLock;
use crate::mm::buddy_allocator::BuddyAllocator;
use core::{
    mem::MaybeUninit,
    ptr::NonNull,
    sync::atomic::{AtomicBool, Ordering},
};

/// Global storage for the buddy allocator, wrapped in a spinlock for
/// safe concurrent access.
static mut BUDDY_STORAGE: MaybeUninit<SpinLock<BuddyAllocator>> = MaybeUninit::uninit();
static BUDDY_INITIALIZED: AtomicBool = AtomicBool::new(false);

/// High-level interface for allocating pages, page blocks, and page tables.
///
/// `PageAllocator` wraps a `BuddyAllocator` stored in `BUDDY_STORAGE`.
/// Provides RAII-style wrappers for allocated memory to ensure proper
/// deallocation when values go out of scope.
pub struct PageAllocator;

impl PageAllocator {
    /// Initializes the global buddy allocator.
    ///
    /// # Safety
    /// - Must be called exactly once during early boot.
    /// - Must be called before interrupts or secondary cores are enabled.
    ///
    /// # Arguments
    /// - `start`: The start physical address of memory to manage.
    /// - `end`: The end physical address of memory to manage.
    ///
    /// # Panics
    /// Panics if called more than once.
    pub unsafe fn init(start: usize, end: usize) {
        if BUDDY_INITIALIZED.swap(true, Ordering::AcqRel) {
            panic!("PageAllocator initialized twice");
        }

        unsafe {
            let mut alloc = BuddyAllocator::new();
            alloc.init(start, end);

            let storage_ptr = core::ptr::addr_of_mut!(BUDDY_STORAGE);
            (*storage_ptr).write(SpinLock::new(alloc));
        }
    }

    /// Returns a reference to the global page allocator instance.
    ///
    /// # Panics
    /// Panics if the allocator has not been initialized.
    pub fn get() -> Self {
        if !BUDDY_INITIALIZED.load(Ordering::Acquire) {
            panic!("PageAllocator not initialized");
        }
        Self
    }

    /// Accesses the buddy allocator with a lock guard.
    fn with_allocator<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut BuddyAllocator) -> R,
    {
        unsafe {
            // SAFETY: We've verified initialization via BUDDY_INITIALIZED,
            // and SpinLock ensures exclusive access to the allocator.
            let storage_ptr = core::ptr::addr_of!(BUDDY_STORAGE);
            let alloc = &*(*storage_ptr).as_ptr();
            let mut guard = alloc.lock();
            f(&mut *guard)
        }
    }

    /// Allocates a single page.
    pub fn alloc_page(&self) -> Option<Page> {
        self.with_allocator(|alloc| unsafe { alloc.alloc_page() }.map(Page::new))
    }

    /// Allocates a block of pages of size `2^ORDER`.
    pub fn alloc_block<const ORDER: usize>(&self) -> Option<PageBlock<ORDER>> {
        self.with_allocator(|alloc| unsafe { alloc.alloc_pages(ORDER) }.map(PageBlock::new))
    }

    /// Allocates an L1 page table (8 KiB, order = 2).
    pub fn alloc_l1_table(&self) -> Option<L1Table> {
        self.with_allocator(|alloc| unsafe { alloc.alloc_pages(2) }.map(L1Table::new))
    }

    /// Allocates an L2 page table (single page).
    pub fn alloc_l2_table(&self) -> Option<L2Table> {
        self.with_allocator(|alloc| unsafe { alloc.alloc_page() }.map(L2Table::new))
    }
}

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
    fn new(addr: usize) -> Self {
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
            guard.free_page(self.addr());
        }
    }
}

/// Represents a block of pages of size `2^ORDER`.
pub struct PageBlock<const ORDER: usize> {
    addr: NonNull<u8>,
    flag: debug::AllocFlag,
}

impl<const ORDER: usize> PageBlock<ORDER> {
    fn new(addr: usize) -> Self {
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
            guard.free_pages(self.addr(), ORDER);
        }
    }
}

/// Represents an L1 page table (8 KiB, order = 2).
pub struct L1Table {
    addr: NonNull<u8>,
    flag: debug::AllocFlag,
}

impl L1Table {
    fn new(addr: usize) -> Self {
        Self {
            addr: NonNull::new(addr as *mut u8).unwrap(),
            flag: debug::AllocFlag::new(),
        }
    }

    /// Returns the base address of the L1 table.
    pub fn base(&self) -> usize {
        self.addr.as_ptr() as usize
    }
}

impl Drop for L1Table {
    fn drop(&mut self) {
        self.flag.mark_freed();
        unsafe {
            let storage_ptr = core::ptr::addr_of!(BUDDY_STORAGE);
            let alloc = &*(*storage_ptr).as_ptr();
            let mut guard = alloc.lock();
            guard.free_pages(self.base(), 2);
        }
    }
}

/// Represents an L2 page table (single page).
pub struct L2Table {
    addr: NonNull<u8>,
    flag: debug::AllocFlag,
}

impl L2Table {
    fn new(addr: usize) -> Self {
        Self {
            addr: NonNull::new(addr as *mut u8).unwrap(),
            flag: debug::AllocFlag::new(),
        }
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
            guard.free_page(self.base());
        }
    }
}
