ENTRY(_start)

SECTIONS
{
    . = 0x8000;

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

    /* Load address for data copy at boot */
    _data_load = LOADADDR(.data);

    .bss (NOLOAD) : ALIGN(4) {
        _bss_start = .;
        *(.bss*)
        *(COMMON)
        _bss_end = .;
    }

    /* Kernel's L1 page table (16KB, 16KB aligned) */
    .l1pagetable (NOLOAD) : ALIGN(16K) {
        _l1pagetable_start = .;
        KEEP(*(.l1pagetable))
        _l1pagetable_end = .;
    }

    /* Kernel heap */
    . = ALIGN(4096);
    _kernel_heap_start = .;
    . = . + 0x40000;          /* 256 KB */
    _kernel_heap_end = .;

    /* Kernel (System mode) stack */
    . = ALIGN(4096);
    _kernel_stack_bottom = .;
    . = . + 0x8000;           /* 32 KB */
    _kernel_stack_top = .;

    /* IRQ stack */
    . = ALIGN(4096);
    _irq_stack_bottom = .;
    . = . + 0x1000;           /* 4 KB */
    _irq_stack_top = .;

    /* SVC stack */
    . = ALIGN(4096);
    _svc_stack_bottom = .;
    . = . + 0x1000;           /* 4 KB */
    _svc_stack_top = .;

    _kernel_end = .;

    . = ALIGN(4096);
    _free_memory_start = .;
    /* The end will be determined at runtime based on RAM size */

    /DISCARD/ : {
        *(.ARM.exidx*)
        *(.ARM.extab*)
        *(.ARM.attributes)
        *(.comment)
        *(.eh_frame)
        *(.note*)
    }
}

_kernel_size = _kernel_end - 0x8000;
_heap_size   = _kernel_heap_end - _kernel_heap_start;
_stack_size  = _kernel_stack_top - _kernel_stack_bottom;
