# Multiboot2 header - works on all x86 32-bit CPUs
.section .multiboot
.align 8

multiboot_header_start:
    .long 0xe85250d6                # magic number
    .long 0                         # architecture (0 = i386)
    .long multiboot_header_end - multiboot_header_start  # header length
    .long -(0xe85250d6 + 0 + (multiboot_header_end - multiboot_header_start))  # checksum

    # End tag (type=0, flags=0, size=8)
    .short 0    # type
    .short 0    # flags
    .long 8     # size
multiboot_header_end:

# Entry point
.section .text
.global _start
.extern kmain

_start:
    cli                     # Disable interrupts
    mov $stack_top, %esp    # Set up stack
    mov $stack_top, %ebp
    
    # Zero out the BSS section
    mov $bss_start, %edi
    mov $bss_end, %ecx
    sub %edi, %ecx
    xor %eax, %eax
    rep stosb
    
    # Call kernel main
    # EBX contains multiboot info pointer (passed by GRUB)
    # EAX contains multiboot magic (0x36d76289)
    push %ebx              # Arg 2: Multiboot info pointer
    push %eax              # Arg 1: Multiboot magic
    call kernel_init
    call kernel_main
    add $8, %esp           # Clean up stack (in case function returns)

halt:
    cli
    hlt
    jmp halt

# Stack in BSS section
.section .bss
.align 16
stack_bottom:
    .skip 16384            # 16KB stack
stack_top:

.global bss_start
.global bss_end
bss_start = stack_bottom
bss_end = stack_top