use crate::mm::page_allocator::PAGE_SIZE;
use crate::mm::page_table::PageBlock;

/// Size of kernel stack in pages (order for buddy allocator)
const KERNEL_STACK_ORDER: usize = 2; // 2^2 = 4 pages = 16KB

/// Size of user stack in pages (order for buddy allocator)
const USER_STACK_ORDER: usize = 2; // 2^2 = 4 pages = 16KB

/// Kernel-mode stack for a process
///
/// Used when the process is executing kernel code (syscalls, interrupts).
/// Automatically deallocated on drop via RAII.
pub struct KernelStack {
    block: PageBlock<KERNEL_STACK_ORDER>,
}

impl KernelStack {
    /// Allocate a new kernel stack
    pub fn new() -> Result<Self, StackError> {
        let block = crate::mm::page_allocator::PAGE_ALLOCATOR
            .alloc_block::<KERNEL_STACK_ORDER>()
            .ok_or(StackError::OutOfMemory)?;

        Ok(Self { block: block })
    }

    /// Get the top of the stack (highest address, stack grows downward)
    pub fn top(&self) -> usize {
        self.block.addr() + (PAGE_SIZE << KERNEL_STACK_ORDER)
    }

    /// Get the bottom of the stack (lowest address)
    pub fn bottom(&self) -> usize {
        self.block.addr()
    }

    /// Get initial stack pointer for new process
    /// Leave 16 bytes at top for alignment/safety
    pub fn initial_sp(&self) -> usize {
        self.top() - 16
    }

    /// Get the size of the stack in bytes
    pub fn size(&self) -> usize {
        PAGE_SIZE << KERNEL_STACK_ORDER
    }
}

/// User-mode stack for a process
///
/// Used when the process is executing in user mode.
/// Automatically deallocated on drop via RAII.
pub struct UserStack {
    block: PageBlock<USER_STACK_ORDER>,
}

impl UserStack {
    /// Allocate a new user stack
    pub fn new() -> Result<Self, StackError> {
        let block = crate::mm::page_allocator::PAGE_ALLOCATOR
            .alloc_block::<USER_STACK_ORDER>()
            .ok_or(StackError::OutOfMemory)?;

        Ok(Self { block: block })
    }

    /// Get the top of the stack (highest address, stack grows downward)
    pub fn top(&self) -> usize {
        self.block.addr() + (PAGE_SIZE << USER_STACK_ORDER)
    }

    /// Get the bottom of the stack (lowest address)
    pub fn bottom(&self) -> usize {
        self.block.addr()
    }

    /// Get initial stack pointer for new process
    /// Leave 16 bytes at top for alignment/safety
    pub fn initial_sp(&self) -> usize {
        self.top() - 16
    }

    /// Get the size of the stack in bytes
    pub fn size(&self) -> usize {
        PAGE_SIZE << USER_STACK_ORDER
    }
}

/// Stack allocation error
#[derive(Debug, Clone, Copy)]
pub enum StackError {
    /// Not enough memory to allocate stack
    OutOfMemory,
}
