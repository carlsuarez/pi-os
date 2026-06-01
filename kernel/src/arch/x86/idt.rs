use core::ptr::addr_of;

// ============================================================================
// IDT Entry — 32-bit interrupt gate descriptor (8 bytes)
// ============================================================================

#[derive(Clone, Copy)]
#[repr(C, packed)]
pub struct IdtEntry {
    offset_low:  u16, // handler address [15:0]
    selector:    u16, // code segment selector (0x08)
    zero:        u8,
    type_attr:   u8,  // 0x8E = present, DPL=0, 32-bit interrupt gate
    offset_high: u16, // handler address [31:16]
}

impl IdtEntry {
    const fn absent() -> Self {
        Self { offset_low: 0, selector: 0, zero: 0, type_attr: 0, offset_high: 0 }
    }

    fn new(handler: usize) -> Self {
        Self {
            offset_low:  (handler & 0xFFFF) as u16,
            selector:    0x08,
            zero:        0,
            type_attr:   0x8E, // P=1, DPL=00, type=interrupt gate (32-bit)
            offset_high: ((handler >> 16) & 0xFFFF) as u16,
        }
    }
}

// ============================================================================
// IDT Table & Pointer
// ============================================================================

static mut IDT: [IdtEntry; 256] = [IdtEntry::absent(); 256];

#[repr(C, packed)]
struct IdtPtr {
    limit: u16,
    base:  u32,
}

unsafe fn load() {
    let ptr = IdtPtr {
        limit: (256 * core::mem::size_of::<IdtEntry>() - 1) as u16,
        base:  addr_of!(IDT) as u32,
    };
    unsafe { core::arch::asm!("lidt ({0})", in(reg) &ptr, options(att_syntax, readonly, nostack)) };
}

// ============================================================================
// Vector address table (defined in the global_asm! block below)
// ============================================================================

unsafe extern "C" {
    static VECTOR_TABLE: [u32; 48];
}

// ============================================================================
// Public init: populate IDT entries from VECTOR_TABLE, then load
// ============================================================================

pub unsafe fn init() {
    for i in 0..48usize {
        let handler = unsafe { VECTOR_TABLE[i] } as usize;
        unsafe { IDT[i] = IdtEntry::new(handler) };
    }
    unsafe { load() };
}

// ============================================================================
// Interrupt vector stubs + common trap entry
//
// Stack layout when `call trap_handler` fires (low → high address):
//   [gs][fs][es][ds] [edi][esi][ebp][esp_saved][ebx][edx][ecx][eax]
//   [trap_number][error_code] [eip][cs][eflags]
//
// This exactly matches the TrapFrame struct field order (gs is field 0).
//
// Exceptions that push an error code (CPU-pushed, so no dummy needed):
//   8 (DF), 10 (TS), 11 (NP), 12 (SS), 13 (GP), 14 (PF), 17 (AC), 21 (CP)
// All other vectors: push dummy 0 as error_code before the vector number.
// ============================================================================

core::arch::global_asm!(r#"
.macro vec_noerr num
.global vector\num
vector\num:
    push 0
    push \num
    jmp  trap_entry
.endm

.macro vec_err num
.global vector\num
vector\num:
    push \num
    jmp  trap_entry
.endm

vec_noerr 0
vec_noerr 1
vec_noerr 2
vec_noerr 3
vec_noerr 4
vec_noerr 5
vec_noerr 6
vec_noerr 7
vec_err   8
vec_noerr 9
vec_err   10
vec_err   11
vec_err   12
vec_err   13
vec_err   14
vec_noerr 15
vec_noerr 16
vec_err   17
vec_noerr 18
vec_noerr 19
vec_noerr 20
vec_err   21
vec_noerr 22
vec_noerr 23
vec_noerr 24
vec_noerr 25
vec_noerr 26
vec_noerr 27
vec_noerr 28
vec_noerr 29
vec_noerr 30
vec_noerr 31
vec_noerr 32
vec_noerr 33
vec_noerr 34
vec_noerr 35
vec_noerr 36
vec_noerr 37
vec_noerr 38
vec_noerr 39
vec_noerr 40
vec_noerr 41
vec_noerr 42
vec_noerr 43
vec_noerr 44
vec_noerr 45
vec_noerr 46
vec_noerr 47

// Common entry point for all vectors.
// On entry the stack holds: [trap_number][error_code][eip][cs][eflags]
.global trap_entry
trap_entry:
    pusha
    push ds
    push es
    push fs
    push gs

    // Reload kernel data segments
    mov  ax, 0x10
    mov  ds, ax
    mov  es, ax
    mov  fs, ax
    mov  gs, ax

    push esp            // arg: *mut TrapFrame
    call trap_handler
    add  esp, 4

    pop  gs
    pop  fs
    pop  es
    pop  ds
    popa
    add  esp, 8         // discard trap_number and error_code
    iret

// Vector address lookup table, indexed by vector number.
.global VECTOR_TABLE
VECTOR_TABLE:
    .long vector0,  vector1,  vector2,  vector3
    .long vector4,  vector5,  vector6,  vector7
    .long vector8,  vector9,  vector10, vector11
    .long vector12, vector13, vector14, vector15
    .long vector16, vector17, vector18, vector19
    .long vector20, vector21, vector22, vector23
    .long vector24, vector25, vector26, vector27
    .long vector28, vector29, vector30, vector31
    .long vector32, vector33, vector34, vector35
    .long vector36, vector37, vector38, vector39
    .long vector40, vector41, vector42, vector43
    .long vector44, vector45, vector46, vector47
"#);
