use crate::mm::buddy_allocator::BuddyAllocator;
use crate::mm::page_table::Page;
use crate::mm::page_table::{L1Table, L2Table, PageBlock};
use common::sync::SpinLock;

pub const PAGE_SIZE: usize = 4096;

/// Global page allocator using buddy allocation
pub static PAGE_ALLOCATOR: PageAllocator = PageAllocator::new();

/// High-level interface for allocating pages, page blocks, and page tables.
///
/// `PageAllocator` wraps a `BuddyAllocator` stored in `PAGE_ALLOCATOR`.
/// Provides RAII-style wrappers for allocated memory to ensure proper
/// deallocation when values go out of scope.
pub struct PageAllocator {
    inner: SpinLock<Option<BuddyAllocator>>,
}

impl PageAllocator {
    /// Create a new uninitialized page allocator
    const fn new() -> Self {
        Self {
            inner: SpinLock::new(None),
        }
    }

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
    pub unsafe fn init(&self, start: usize, end: usize) {
        // Retrieve lock and ensure uninitialized
        let mut allocator = self.inner.lock();

        if allocator.is_some() {
            panic!("PageAllocator already initialized");
        }

        // Initialize buddy allocator
        let mut buddy = BuddyAllocator::new(PAGE_SIZE);
        unsafe {
            buddy.init(start, end);
        }
        *allocator = Some(buddy);
    }

    /// Execute a closure with exclusive access to the underlying BuddyAllocator
    ///
    /// # Panics
    /// Panics if the allocator is not yet initialized.
    fn with_page_allocator<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut BuddyAllocator) -> R,
    {
        let mut guard = self.inner.lock();
        let allocator = guard.as_mut().expect("PageAllocator not initialized");
        f(allocator)
    }

    /// Allocates a single page.
    pub fn alloc(&self) -> Option<Page> {
        self.with_page_allocator(|alloc| unsafe { alloc.alloc_block() }.map(Page::new))
    }

    /// Allocates a block of pages of size `2^ORDER`.
    pub fn alloc_block<const ORDER: usize>(&self) -> Option<PageBlock<ORDER>> {
        self.with_page_allocator(|alloc| {
            unsafe { alloc.alloc_block_order(ORDER) }.map(PageBlock::new)
        })
    }

    /// Allocates an L1 page table (8 KiB, order = 2).
    pub fn alloc_l1_table(&self) -> Option<L1Table> {
        self.with_page_allocator(|alloc| unsafe { alloc.alloc_block_order(2) }.map(L1Table::new))
    }

    /// Allocates an L2 page table (single page).
    pub fn alloc_l2_table(&self) -> Option<L2Table> {
        self.with_page_allocator(|alloc| unsafe { alloc.alloc_block() }.map(L2Table::new))
    }

    /// Free a block of memory
    ///
    /// # Safety
    /// - `addr` must be a valid address returned by a prior allocation
    /// - `order` must match the order used during allocation
    /// - Must not be double-freed
    pub unsafe fn free_block(&self, addr: usize, order: usize) {
        let mut guard = self.inner.lock();
        if let Some(allocator) = guard.as_mut() {
            unsafe {
                allocator.free_block(addr, order);
            }
        }
    }
}
