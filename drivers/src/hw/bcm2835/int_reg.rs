pub const INT_REG_BASE: usize = 0x2000_b000;

#[repr(C)]
pub struct IntReg {
    _padding: [u8; 0x200],
    pub irq_basic_pend: u32,
    pub irq_1_pend: u32,
    pub irq_2_pend: u32,
    pub fiq_ctrl: u32,
    pub enable_irqs_1: u32,
    pub enable_irqs_2: u32,
    pub enable_basic_irqs: u32,
    pub disable_irqs_1: u32,
    pub disable_irqs_2: u32,
    pub disable_basic_irqs: u32,
}
