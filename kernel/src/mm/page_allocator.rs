use crate::mm::buddy_allocator::BuddyAllocator;
use crate::mm::page_table::Page;
use crate::mm::page_table::{L1Table, L2Table, PageBlock};
use common::sync::SpinLock;
use core::cell::OnceCell;

pub const PAGE_SIZE: usize = 4096;

/// Global page allocator using buddy allocation
static PAGE_ALLOCATOR: PageAllocator = PageAllocator::new();

/// High-level interface for allocating pages, page blocks, and page tables.
///
/// `PageAllocator` wraps a `BuddyAllocator` stored in `PAGE_ALLOCATOR`.
/// Provides RAII-style wrappers for allocated memory to ensure proper
/// deallocation when values go out of scope.
pub struct PageAllocator {
    inner: OnceCell<SpinLock<BuddyAllocator>>,
}

impl PageAllocator {
    /// Create a new uninitialized page allocator
    const fn new() -> Self {
        Self {
            inner: OnceCell::new(),
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
        // Initialize buddy allocator
        let mut buddy = BuddyAllocator::new(PAGE_SIZE);
        unsafe {
            buddy.init(start, end);
        }

        // Try to set the OnceCell
        if self.inner.set(SpinLock::new(buddy)).is_err() {
            panic!("PageAllocator already initialized");
        }
    }

    /// Execute a closure with exclusive access to the underlying BuddyAllocator
    ///
    /// # Panics
    /// Panics if the allocator is not yet initialized.
    fn with_page_allocator<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut BuddyAllocator) -> R,
    {
        let allocator = self.inner.get().expect("PageAllocator not initialized");
        let mut guard = allocator.lock();
        f(&mut *guard)
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
        if let Some(allocator) = self.inner.get() {
            let mut guard = allocator.lock();
            unsafe {
                guard.free_block(addr, order);
            }
        }
    }
}

// SAFETY: PageAllocator wraps a OnceCell<SpinLock<BuddyAllocator>>.
// - OnceCell provides thread-safe one-time initialization
// - SpinLock ensures exclusive access to the BuddyAllocator
// - BuddyAllocator itself is Send + Sync (manages its own invariants)
// Thread safety is guaranteed by the SpinLock wrapper.
unsafe impl Send for PageAllocator {}
unsafe impl Sync for PageAllocator {}

pub fn page_allocator() -> &'static PageAllocator {
    &PAGE_ALLOCATOR
}
