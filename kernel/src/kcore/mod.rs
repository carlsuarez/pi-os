pub mod init;

cfg_if::cfg_if!(
    if #[cfg(target_arch = "x86")] {
        pub use crate::arch::x86::time::delay_cycles;
    }
    else {
        // Dummy implementation for non-x86 platforms
pub fn delay(cycles: u64) {
    for _ in 0..cycles {
        core::hint::spin_loop();
    }
}
    }
);
