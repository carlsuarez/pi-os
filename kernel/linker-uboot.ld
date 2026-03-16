/* U-Boot / QEMU virt: RAM base 0x40000000, kernel at 0x40008000 */
ENTRY(_start)
SECTIONS
{
    . = 0x40008000;
    .vectors : ALIGN(4) {
        KEEP(*(.vectors))
    }
    .text : ALIGN(4) {
        *(.text._start)
        *(.text*)
    }
    .rodata : ALIGN(4) {
        *(.rodata*)
    }
    .data : ALIGN(4) {
        _data_start = .;
        *(.data*)
        _data_end = .;
    }
    _data_load = LOADADDR(.data);
    .bss (NOLOAD) : ALIGN(4) {
        _bss_start = .;
        *(.bss*)
        *(COMMON)
        _bss_end = .;
    }
    .l1pagetable (NOLOAD) : ALIGN(16K) {
        _l1pagetable_start = .;
        KEEP(*(.l1pagetable))
        _l1pagetable_end = .;
    }
    . = ALIGN(4096);
    _kernel_heap_start = .;
    . = . + 0x80000;
    _kernel_heap_end = .;
    . = ALIGN(4096);
    _kernel_stack_bottom = .;
    . = . + 0x10000;
    _kernel_stack_top = .;
    . = ALIGN(4096);
    _irq_stack_bottom = .;
    . = . + 0x2000;
    _irq_stack_top = .;
    . = ALIGN(4096);
    _svc_stack_bottom = .;
    . = . + 0x2000;
    _svc_stack_top = .;
    _kernel_end = .;
    . = ALIGN(4096);
    _free_memory_start = .;
    __uboot_r2 = .;
    . = . + 4;
    /DISCARD/ : {
        *(.ARM.exidx*)
        *(.ARM.extab*)
        *(.ARM.attributes)
        *(.comment)
        *(.eh_frame)
        *(.note*)
    }
}
_kernel_size = _kernel_end - 0x40008000;
_heap_size   = _kernel_heap_end - _kernel_heap_start;
_stack_size  = _kernel_stack_top - _kernel_stack_bottom;
