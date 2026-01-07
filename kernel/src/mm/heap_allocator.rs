use super::buddy_allocator::BuddyAllocator;
use common::sync::SpinLock;
use core::alloc::{GlobalAlloc, Layout};

/// Global heap allocator using buddy allocation
pub struct HeapAllocator {
    inner: SpinLock<Option<BuddyAllocator>>,
}

impl HeapAllocator {
    /// Creates a new uninitialized heap allocator
    const fn new() -> Self {
        Self {
            inner: SpinLock::new(None),
        }
    }

    /// Initializes the heap with a memory region
    ///
    /// # Safety
    /// - The memory region [start, end) must be valid and unused
    /// - Should only be called once during kernel initialization
    ///
    /// # Panics
    /// Panics if already initialized
    unsafe fn init(&self, start: usize, end: usize) {
        let mut allocator = self.inner.lock();

        if allocator.is_some() {
            panic!("HeapAllocator already initialized");
        }

        const MIN_BLOCK_SIZE: usize = 32;
        let mut buddy = BuddyAllocator::new(MIN_BLOCK_SIZE);
        unsafe {
            buddy.init(start, end);
        }
        *allocator = Some(buddy);
    }
}

unsafe impl GlobalAlloc for HeapAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut guard = self.inner.lock();
        let allocator = guard.as_mut().expect("heap not initialized");

        match unsafe { allocator.alloc(layout) } {
            Some(ptr) => ptr.as_ptr(),
            None => alloc_error_handler(layout),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        let mut guard = self.inner.lock();
        if let Some(allocator) = guard.as_mut() {
            unsafe {
                allocator.free(ptr);
            }
        }
    }

    unsafe fn realloc(&self, ptr: *mut u8, old_layout: Layout, new_size: usize) -> *mut u8 {
        unsafe {
            let new_layout = Layout::from_size_align_unchecked(new_size, old_layout.align());

            let new_ptr = self.alloc(new_layout);
            if new_ptr.is_null() {
                return core::ptr::null_mut();
            }

            core::ptr::copy_nonoverlapping(
                ptr,
                new_ptr,
                core::cmp::min(old_layout.size(), new_size),
            );

            self.dealloc(ptr, old_layout);
            new_ptr
        }
    }
}

/// The global heap allocator instance
#[global_allocator]
static HEAP: HeapAllocator = HeapAllocator::new();

/// Handler for allocation failures
#[alloc_error_handler]
fn alloc_error_handler(layout: Layout) -> ! {
    panic!(
        "Allocation error: size={}, align={}",
        layout.size(),
        layout.align()
    );
}

/// Initialize the kernel heap
///
/// # Safety
/// Must be called exactly once during early kernel initialization
///
/// # Panics
/// Panics if already initialized
pub unsafe fn init_heap(start: usize, end: usize) {
    unsafe {
        HEAP.init(start, end);
    }
}
