// Constants
pub const UART0_BASE: usize = 0x2020_1000;
pub const PL011_CLOCK: u32 = 48_000_000;
pub const UART_FR_BUSY: u32 = 1 << 3;
pub const UART_CR_UARTEN: u32 = 1 << 0;
pub const UART_CR_TXE: u32 = 1 << 8;
pub const UART_CR_RXE: u32 = 1 << 9;
pub const UART_LCRH_WLEN_8: u32 = 0b11 << 5;
pub const UART_LCRH_STP2: u32 = 1 << 3;
pub const UART_LCRH_FEN: u32 = 1 << 4;
pub const UART_IMSC_RXIM: u32 = 1 << 4;
pub const UART_IFLS_RXIFLSEL_7_8: u32 = 0b110 << 3;

/// Memory-mapped PL011 UART registers
#[repr(C)]
pub struct Pl011 {
    pub dr: u32,
    pub rsr_ecr: u32,
    _reserved0: [u32; 4],
    pub fr: u32,
    _reserved1: u32,
    pub ilpr: u32,
    pub ibrd: u32,
    pub fbrd: u32,
    pub lcrh: u32,
    pub cr: u32,
    pub ifls: u32,
    pub imsc: u32,
    pub ris: u32,
    pub mis: u32,
    pub icr: u32,
    pub dmacr: u32,
}
