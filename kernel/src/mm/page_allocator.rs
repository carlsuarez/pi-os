use crate::mm::buddy_allocator::BuddyAllocator;
use crate::mm::page_table::{L1Table, L2Table, PageBlock};
use crate::{kcore::sync::SpinLock, mm::page_table::Page};
use core::{
    mem::MaybeUninit,
    sync::atomic::{AtomicBool, Ordering},
};

const PAGE_SIZE: usize = 4096;

/// Global storage for the buddy allocator, wrapped in a spinlock for
/// safe concurrent access.
pub(in crate::mm) static mut BUDDY_STORAGE: MaybeUninit<SpinLock<BuddyAllocator>> =
    MaybeUninit::uninit();
pub(in crate::mm) static BUDDY_INITIALIZED: AtomicBool = AtomicBool::new(false);

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
            let mut alloc = BuddyAllocator::new(PAGE_SIZE);
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
    pub fn alloc(&self) -> Option<Page> {
        self.with_allocator(|alloc| unsafe { alloc.alloc_block() }.map(Page::new))
    }

    /// Allocates a block of pages of size `2^ORDER`.
    pub fn alloc_block<const ORDER: usize>(&self) -> Option<PageBlock<ORDER>> {
        self.with_allocator(|alloc| unsafe { alloc.alloc_block_order(ORDER) }.map(PageBlock::new))
    }

    /// Allocates an L1 page table (8 KiB, order = 2).
    pub fn alloc_l1_table(&self) -> Option<L1Table> {
        self.with_allocator(|alloc| unsafe { alloc.alloc_block_order(2) }.map(L1Table::new))
    }

    /// Allocates an L2 page table (single page).
    pub fn alloc_l2_table(&self) -> Option<L2Table> {
        self.with_allocator(|alloc| unsafe { alloc.alloc_block() }.map(L2Table::new))
    }
}
