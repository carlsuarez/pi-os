ENTRY(_start)
SECTIONS
{
    . = 1M;

    .multiboot ALIGN(4K) : {
        KEEP(*(.multiboot))
    }

    .text ALIGN(4K) : {
        *(.text.boot)
        *(.text*)
    }

    .rodata ALIGN(4K) : {
        *(.rodata*)
    }

    .data ALIGN(4K) : {
        *(.data*)
    }

    .bss ALIGN(4K) : {
        _bss_start = .;
        *(COMMON)
        *(.bss*)
        . = ALIGN(4K);
        _bss_end = .;
    }

    /DISCARD/ : {
        *(.comment)
        *(.eh_frame)
        *(.note*)
    }
}