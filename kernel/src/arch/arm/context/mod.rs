#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Context {
    // General purpose registers
    pub r0: u32,
    pub r1: u32,
    pub r2: u32,
    pub r3: u32,
    pub r4: u32,
    pub r5: u32,
    pub r6: u32,
    pub r7: u32,
    pub r8: u32,
    pub r9: u32,
    pub r10: u32,
    pub r11: u32,
    pub r12: u32,

    // Stack pointer (user mode)
    pub sp: u32,

    // Link register
    pub lr: u32,

    // Program counter
    pub pc: u32,

    // Program status register
    pub cpsr: u32,
}

impl Context {
    pub const fn new() -> Self {
        Self {
            r0: 0,
            r1: 0,
            r2: 0,
            r3: 0,
            r4: 0,
            r5: 0,
            r6: 0,
            r7: 0,
            r8: 0,
            r9: 0,
            r10: 0,
            r11: 0,
            r12: 0,
            sp: 0,
            lr: 0,
            pc: 0,
            cpsr: 0x10, // User mode
        }
    }
}
