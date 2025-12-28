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

    /* L1 page table (16KB, 16KB aligned) */
    .l1pagetable (NOLOAD) : ALIGN(16K) {
        _l1pagetable_start = .;
        KEEP(*(.l1pagetable))
        _l1pagetable_end = .;
    }

    /* Coarse page tables backing store (1MB) */
    .coarsepagetables_space (NOLOAD) : ALIGN(1M) {
        _coarsepagetables_space_start = .;
        KEEP(*(.coarsepagetables_space))
        _coarsepagetables_space_end = .;
    }

    /* Pool for additional L1 tables (2MB) */
    .l1pagetables_space (NOLOAD) : ALIGN(16K) {
        _l1pagetables_space_start = .;
        KEEP(*(.l1pagetables_space))
        _l1pagetables_space_end = .;
    }

    /* Free physical pages */
    .freepages (NOLOAD) : ALIGN(4K) {
        _free_pages_start = .;
        KEEP(*(.freepages))
        _free_pages_end = .;
    }
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
