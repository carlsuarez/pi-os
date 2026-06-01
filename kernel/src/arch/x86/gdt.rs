use core::ptr::addr_of;

#[repr(C, packed)]
struct GdtEntry {
    limit_low:   u16,
    base_low:    u16,
    base_mid:    u8,
    access:      u8,
    granularity: u8,
    base_high:   u8,
}

#[repr(C, packed)]
struct Gdtr {
    limit: u16,
    base:  u32,
}

impl GdtEntry {
    const fn null() -> Self {
        Self { limit_low: 0, base_low: 0, base_mid: 0, access: 0, granularity: 0, base_high: 0 }
    }

    const fn new(access: u8, granularity: u8) -> Self {
        Self {
            limit_low:   0xFFFF,
            base_low:    0,
            base_mid:    0,
            access,
            granularity,
            base_high:   0,
        }
    }
}

// kernel code: ring0, execute/read, 32-bit, 4 GB flat (access=0x9A, gran=0xCF)
// kernel data: ring0, read/write,   32-bit, 4 GB flat (access=0x92, gran=0xCF)
static mut GDT: [GdtEntry; 3] = [
    GdtEntry::null(),
    GdtEntry::new(0x9A, 0xCF),
    GdtEntry::new(0x92, 0xCF),
];

pub unsafe fn init() {
    let gdtr = Gdtr {
        limit: (core::mem::size_of::<[GdtEntry; 3]>() - 1) as u16,
        base:  addr_of!(GDT) as u32,
    };

    // Load GDT and reload all segment registers.
    // CS is reloaded via far return (lret): push CS selector then return address,
    // then lret pops EIP → label 1, CS → 0x08 atomically.
    unsafe { core::arch::asm!(
        "lgdt ({gdtr})",
        "pushl $0x08",
        "pushl $1f",
        "lret",
        "1:",
        "movw $0x10, %ax",
        "movw %ax, %ds",
        "movw %ax, %es",
        "movw %ax, %fs",
        "movw %ax, %gs",
        "movw %ax, %ss",
        gdtr = in(reg) &gdtr,
        out("ax") _,
        options(att_syntax),
    ); }
}
