ENTRY(_start)

SECTIONS
{
    . = KERNEL_LOAD_ADDR;

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

    . = ALIGN(4096);
    _kernel_stack_bottom = .;
    . += 0x10000;
    _kernel_stack_top = .;

    . = ALIGN(4096);
    _irq_stack_bottom = .;
    . += 0x2000;
    _irq_stack_top = .;

    . = ALIGN(4096);
    _svc_stack_bottom = .;
    . += 0x2000;
    _svc_stack_top = .;

    . = ALIGN(4096);
    _free_memory_start = .;

    /DISCARD/ : {
        *(.ARM.exidx*)
        *(.ARM.extab*)
        *(.ARM.attributes)
        *(.comment)
        *(.eh_frame)
        *(.note*)
    }
}

_kernel_size = _free_memory_start - KERNEL_LOAD_ADDR;