/* ================================================== */
/* Page Table & Boot                                  */
/* ================================================== */
    .syntax unified
    .arm
    .file 1 "kernel/src/arch/arm/boot/boot.S"

/* L1 Page Table for Kernel (16 KB) */
    .section .l1pagetable, "aw", %nobits
    .align 14
    .global l1_page_table
l1_page_table:
    .space 16384

/* -------------------------------------------------- */
/* Boot Entry                                         */
/* -------------------------------------------------- */
    .section .text
    .global _start
    .type _start, %function
_start:
    cpsid if

    /* -------------------------------------------------- */
    /* Copy .data from ROM to RAM                         */
    /* -------------------------------------------------- */
    ldr r0, =_data_load
    ldr r1, =_data_start
    ldr r2, =_data_end
copy_data:
    cmp r1, r2
    ldrlt r3, [r0], #4
    strlt r3, [r1], #4
    blt copy_data

    /* -------------------------------------------------- */
    /* Zero BSS                                          */
    /* -------------------------------------------------- */
    ldr r0, =_bss_start
    ldr r1, =_bss_end
    mov r2, #0
zero_bss:
    cmp r0, r1
    strlt r2, [r0], #4
    blt zero_bss

    /* -------------------------------------------------- */
    /* Setup stacks                                      */
    /* -------------------------------------------------- */
    cps #0x12
    ldr sp, =_irq_stack_top
    cps #0x13
    ldr sp, =_svc_stack_top
    cps #0x1F
    ldr sp, =_kernel_stack_top

    /* -------------------------------------------------- */
    /* Enable VFP                                        */
    /* -------------------------------------------------- */
    mrc p15, 0, r0, c1, c0, 2
    orr r0, r0, #0x00F00000
    mcr p15, 0, r0, c1, c0, 2
    mov r0, #0x40000000
    vmsr fpexc, r0

    /* -------------------------------------------------- */
    /* Kernel init + MMU                                 */
    /* -------------------------------------------------- */
    bl kernel_init
    bl init_kernel_page_table
    bl enable_mmu

    /* -------------------------------------------------- */
    /* Set VBAR to _vectors                               */
    /* -------------------------------------------------- */
    ldr r0, =_vectors
    mcr p15, 0, r0, c12, c0, 0
    
    /* ISB */
    mov     r0, #0
    mcr     p15, 0, r0, c7, c5, 4

    /* -------------------------------------------------- */
    /* Jump to Rust kernel                               */
    /* -------------------------------------------------- */
    bl kernel_main

halt:
    wfi
    b halt


/* ================================================== */
/* MMU Initialization                                 */
/* ================================================== */
    .global enable_mmu
    .type enable_mmu, %function
enable_mmu:
    mov     r0, #0
    mcr     p15, 0, r0, c8, c7, 0      @ Invalidate TLB

    ldr r0, =l1_page_table
    orr r0, r0, #(1<<6) | (1<<0)   @ IRGN=WBWA, RGN=WBWA
    mcr p15, 0, r0, c2, c0, 0     @ TTBR0

    mov     r0, #0
    mcr     p15, 0, r0, c2, c0, 2      @ TTBCR = 0

    ldr     r0, =0x55555555
    mcr     p15, 0, r0, c3, c0, 0      @ Domains client

    mrc     p15, 0, r0, c1, c0, 0
    bic     r0, r0, #(1 << 29)        @ Disable AFE
    mcr     p15, 0, r0, c1, c0, 0

    mov     r0, #0
    mcr     p15, 0, r0, c7, c10, 4     @ DSB

    mrc     p15, 0, r0, c1, c0, 0
    orr     r0, r0, #(1 << 0)         @ MMU enable
    orr     r0, r0, #(1 << 2)         @ D-cache
    orr     r0, r0, #(1 << 12)        @ I-cache
    mcr     p15, 0, r0, c1, c0, 0

    mov     r0, #0
    mcr     p15, 0, r0, c7, c5, 4      @ ISB

    bx      lr
    .size enable_mmu, . - enable_mmu
