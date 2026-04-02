# Multiboot2 header
.section .multiboot, "a"
.align 8
multiboot_header_start:
    .long 0xe85250d6                # magic number
    .long 0                         # architecture (0 = i386)
    .long multiboot_header_end - multiboot_header_start
    .long -(0xe85250d6 + 0 + (multiboot_header_end - multiboot_header_start))
    # End tag (type=0, flags=0, size=8)
    .short 0
    .short 0
    .long 8
multiboot_header_end:

# Entry point
.section .text.boot
.global _start
.extern kernel_init
.extern kernel_main
.extern _bss_start
.extern _bss_end

_start:
    cli
    mov $stack_top, %esp
    mov $stack_top, %ebp

    # Save multiboot registers
    mov %eax, %esi          # esi = multiboot2 magic
    mov %ebx, %edx          # edx = multiboot info pointer

    # Zero BSS
    mov $_bss_start, %edi
    mov $_bss_end, %ecx
    sub %edi, %ecx
    xor %eax, %eax
    rep stosb

    # Call kernel_init(machine_type, atags_addr)
    push %edx               # arg2: info pointer
    push %esi               # arg1: magic
    call kernel_init
    call kernel_main
    add $8, %esp

halt:
    cli
    hlt
    jmp halt

.section .bss
.align 16
stack_bottom:
    .skip 16384
stack_top: