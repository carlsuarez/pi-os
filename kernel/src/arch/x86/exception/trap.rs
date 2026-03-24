pub struct TrapFrame {
    pub gs: u32,
    pub fs: u32,
    pub es: u32,
    pub ds: u32,
    pub edi: u32,
    pub esi: u32,
    pub ebp: u32,
    pub esp: u32, // This is the user-mode stack pointer, not the kernel stack pointer
    pub ebx: u32,
    pub edx: u32,
    pub ecx: u32,
    pub eax: u32,
    pub trap_number: u32,
    pub error_code: u32,
    pub eip: u32,
    pub cs: u32,
    pub eflags: u32,
}
