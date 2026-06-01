pub mod context;
pub mod exception;
pub mod gdt;
pub mod idt;
pub mod interrupt;
pub mod mmu;
pub mod time;

pub unsafe fn arch_init() {
    unsafe {
        gdt::init();
        idt::init();
    }
}
