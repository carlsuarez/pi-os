/* ================================================== */
/* ARM Boot Entry                                     */
/* ================================================== */
    .syntax unified
    .arm
    .file 1 "kernel/src/arch/arm/boot/boot.S"

    .section .text._start, "ax"
    .global _start
    .type _start, %function
_start:
    cpsid if

    /* Preserve boot parameters across setup code.        */
    /* r1 = machine_type, r2 = atags/dtb addr on entry.  */
    mov r4, r1
    mov r5, r2

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
    /* Zero BSS                                           */
    /* -------------------------------------------------- */
    ldr r0, =_bss_start
    ldr r1, =_bss_end
    mov r2, #0
zero_bss:
    cmp r0, r1
    strlt r2, [r0], #4
    blt zero_bss

    /* -------------------------------------------------- */
    /* Setup stacks                                       */
    /* -------------------------------------------------- */
    cps #0x12
    ldr sp, =_irq_stack_top
    cps #0x13
    ldr sp, =_svc_stack_top
    cps #0x1F
    ldr sp, =_kernel_stack_top

    /* -------------------------------------------------- */
    /* Set VBAR to exception vector table                 */
    /* -------------------------------------------------- */
    ldr r0, =_vectors
    mcr p15, 0, r0, c12, c0, 0
    mov r0, #0
    mcr p15, 0, r0, c7, c5, 4          @ ISB
    
    /* -------------------------------------------------- */
    /* kernel_init: memory discovery, heap + page         */
    /* allocator setup, and MMU enable all happen here.  */
    /* -------------------------------------------------- */
    mov r0, r4
    mov r1, r5
    bl kernel_init


    /* -------------------------------------------------- */
    /* Jump to Rust kernel main                           */
    /* -------------------------------------------------- */
    bl kernel_main

halt:
    wfi
    b halt

    .size _start, . - _start