/* ================================================== */
/* Page Table & Boot                                  */
/* ================================================== */
    .syntax unified
    .arm
    .file 1 "kernel/src/arch/arm/boot/boot.S"

/* L1 Page Table for Kernel (16 KB) */
    .section .l1pagetable, "aw", %nobits
    .align 14                              @ 16 KB alignment
    .global l1_page_table
l1_page_table:
    .space 16384                           @ Reserve 16 KB

/* Boot Entry Point */
    .section .text
    .global _start
    .type _start, %function
_start:
    .cfi_startproc
    .loc 1 21 0
    cpsid if                                @ Disable IRQ/FIQ interrupts

    /* Copy vector table from 0x8000 to 0x0 */
    mov r0, #0x8000                        @ Source: GPU loaded
    mov r1, #0x0000                        @ Destination: vector table base
    ldmia r0!, {r2-r9}                      @ Load first 32 bytes
    stmia r1!, {r2-r9}                      @ Store to 0x0
    ldmia r0!, {r2-r9}                      @ Load next 32 bytes
    stmia r1!, {r2-r9}                      @ Store to 0x20

    /* Copy .data section from ROM to RAM if needed */
    ldr r0, =_data_load
    ldr r1, =_data_start
    ldr r2, =_data_end
    cmp r0, r1
    beq skip_data_copy
copy_data:
    cmp r1, r2
    ldrlt r3, [r0], #4
    strlt r3, [r1], #4
    blt copy_data
skip_data_copy:

    /* Zero out the .bss section */
    ldr r0, =_bss_start
    ldr r1, =_bss_end
    mov r2, #0
zero_bss:
    cmp r0, r1
    strlt r2, [r0], #4
    blt zero_bss

    /* Initialize stacks for IRQ, SVC, System modes */
    cps #0x12                    @ IRQ mode
    ldr sp, =_irq_stack_top
    cps #0x13                    @ Supervisor mode
    ldr sp, =_svc_stack_top
    cps #0x1F                    @ System mode (kernel)
    ldr sp, =_kernel_stack_top

    /* Enable VFP */
    mrc p15, 0, r0, c1, c0, 2
    orr r0, r0, #0x00F00000
    mcr p15, 0, r0, c1, c0, 2
    mov r0, #0x40000000
    vmsr fpexc, r0

    /* Initalize core */
    bl kernel_init
    
    /* Initialize kernel's L1 page table */
    bl init_kernel_page_table

    /* Initialize MMU with kernel page table */
    bl enable_mmu

    /* Jump to Rust kernel_main */
    bl kernel_main

halt:
    wfi
    b halt
    .cfi_endproc
    .size _start, . - _start

/* MMU Initialization */
    .global enable_mmu
    .type enable_mmu, %function
enable_mmu:
    .cfi_startproc
    /* Invalidate unified TLB */
    mov     r0, #0
    mcr     p15, 0, r0, c8, c7, 0

    /* Set TTBR0 = L1 page table base + RGN=WBWA */
    ldr     r0, =l1_page_table
    orr     r0, r0, #(1 << 3)      @ RGN = Write-back, Write-allocate
    mcr     p15, 0, r0, c2, c0, 0  @ TTBR0

    /* Use TTBR0 only */
    mov     r0, #0
    mcr     p15, 0, r0, c2, c0, 2  @ TTBCR = 0

    /* Domain Access Control
     * Domain 0–3 = Client (01)
     */
    ldr     r0, =0x55555555
    mcr     p15, 0, r0, c3, c0, 0

    /* Disable AFE (use legacy AP encoding) */
    mrc     p15, 0, r0, c1, c0, 0
    bic     r0, r0, #(1 << 29)     @ SCTLR.AFE = 0
    mcr     p15, 0, r0, c1, c0, 0

    /* Data Synchronization Barrier */
    mov     r0, #0
    mcr     p15, 0, r0, c7, c10, 4

    /* Enable MMU + caches */
    mrc     p15, 0, r0, c1, c0, 0
    orr     r0, r0, #(1 << 0)      @ MMU enable
    orr     r0, r0, #(1 << 2)      @ D-cache enable
    orr     r0, r0, #(1 << 12)     @ I-cache enable
    mcr     p15, 0, r0, c1, c0, 0

    /* Instruction Synchronization Barrier */
    mov     r0, #0
    mcr     p15, 0, r0, c7, c5, 4

    bx      lr
    .size enable_mmu, . - enable_mmu
    .cfi_endproc
    