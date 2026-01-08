use super::stack::KernelStack;
use crate::fs::fd::FileDescriptorTable;
use crate::mm::page_table::L1Table;
use alloc::string::String;

cfg_if::cfg_if! {
    if #[cfg(target_arch = "arm")] {
        use crate::arch::arm::context::Context;
    } else {
        compile_error!("Unsupported architecture for Process Control Block");
    }
}

/// Process states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    Ready,   // Ready to run
    Running, // Currently executing
    Blocked, // Waiting for I/O or event
    Zombie,  // Terminated but not yet reaped
}

/// Process identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Pid(pub usize);

/// Process Control Block
pub struct Process {
    /// Process ID
    pub pid: Pid,

    /// Parent process ID
    pub parent_pid: Option<Pid>,

    /// Current state
    pub state: ProcessState,

    /// CPU context (registers to restore)
    pub context: Context,

    /// Page table
    pub page_table: L1Table,

    /// Kernel stack
    pub kernel_stack: KernelStack,

    /// User stack pointer
    pub user_stack_ptr: usize,

    /// Process name
    pub name: String,

    /// Priority (for scheduling)
    pub priority: u8,

    /// Time quantum remaining
    pub time_slice: u32,

    /// File descriptor table
    pub fd_table: FileDescriptorTable,

    /// Exit code (if zombie)
    pub exit_code: Option<i32>,
}
