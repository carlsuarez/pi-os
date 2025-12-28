/*
 * Exception Handlers
 */
    .section .text
    .syntax unified
    .arm

    .file 1 "kernel/src/arch/arm/exception/entry.S"

    .global undefined_handler
    .global prefetch_abort_handler
    .global data_abort_handler
    .global fiq_handler
    .global svc_handler
    .global irq_handler

    .extern svc_entry_rust
    .extern irq_entry_rust

/*
    Undefined instruction handler
*/
    .type undefined_handler, %function
undefined_handler:
    .loc 1 20 0
    .cfi_startproc
1:
    b 1b
    .cfi_endproc
    .size undefined_handler, . - undefined_handler

/*
    Supervisor call handler
*/
    .type svc_handler, %function
svc_handler:
    .loc 1 32 0
    .cfi_startproc
    b svc_handler
    .cfi_endproc
    .size svc_handler, . - svc_handler

/* 
    Prefetch abort handler
*/
    .type prefetch_abort_handler, %function
prefetch_abort_handler:
    .loc 1 44 0
    .cfi_startproc
2:
    b 2b
    .cfi_endproc
    .size prefetch_abort_handler, . - prefetch_abort_handler

/* 
    Data abort handler
*/
    .type data_abort_handler, %function
data_abort_handler:
    .loc 1 56 0
    .cfi_startproc
3:
    b 3b
    .cfi_endproc
    .size data_abort_handler, . - data_abort_handler

/*
    IRQ handler 
*/
    .type irq_handler, %function
irq_handler:
    .loc 1 68 0
    .cfi_startproc

    sub     lr, lr, #4              @ LR fixup for IRQ return
    .loc 1 70 0

    stmdb   sp!, {r0-r12, lr}       @ save GPRs
    .cfi_adjust_cfa_offset 56
    .cfi_offset lr, -4

    mrs     r0, spsr
    push    {r0}                    @ save SPSR
    .cfi_adjust_cfa_offset 4

    cpsid   i                       @ mask further IRQs
    .loc 1 78 0

    mov     r0, sp                  @ &TrapFrame
    bl      irq_entry_rust
    .loc 1 81 0

    pop     {r0}                    @ restore SPSR
    msr     spsr_cxsf, r0
    .cfi_adjust_cfa_offset -4

    ldmia   sp!, {r0-r12, lr}       @ restore registers
    .cfi_adjust_cfa_offset -56

    subs    pc, lr, #0              @ exception return

    .cfi_endproc
    .size irq_handler, . - irq_handler

/*
    FIQ Handler
 */
    .type fiq_handler, %function
fiq_handler:
    .loc 1 96 0
    .cfi_startproc
4:
    b 4b
    .cfi_endproc
    .size fiq_handler, . - fiq_handler
