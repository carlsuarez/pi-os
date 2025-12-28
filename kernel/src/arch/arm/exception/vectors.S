/* -------------------------------------------------- */
/* ARM Exception Vector Table                          */
/* -------------------------------------------------- */
    .section .vectors, "ax"         @ Allocate in executable segment
    .syntax unified
    .arm
    .file 1 "kernel/src/arch/arm/exception/vectors.S" @ Source filename for debug info
    .align 2                        @ Align to 4-byte boundary (16 bytes)
    .global _vectors

    .extern _start
    .extern undefined_handler
    .extern svc_handler
    .extern prefetch_abort_handler
    .extern data_abort_handler
    .extern irq_handler
    .extern fiq_handler

/* Vector table */
    .type _vectors, %function
_vectors:
    .cfi_startproc
    .loc 1 12 0

    ldr pc, reset_vec      @ 0x00 Reset
    ldr pc, undef_vec      @ 0x04 Undefined instruction
    ldr pc, svc_vec        @ 0x08 Supervisor call
    ldr pc, pabort_vec     @ 0x0C Prefetch abort
    ldr pc, dabort_vec     @ 0x10 Data abort
    nop                    @ 0x14 Reserved / padding
    ldr pc, irq_vec        @ 0x18 IRQ
    ldr pc, fiq_vec        @ 0x1C FIQ

    .cfi_endproc
    .size _vectors, . - _vectors

/* Vector addresses */
reset_vec:      .word _start
undef_vec:      .word undefined_handler
svc_vec:        .word svc_handler
pabort_vec:     .word prefetch_abort_handler
dabort_vec:     .word data_abort_handler
irq_vec:        .word irq_handler
fiq_vec:        .word fiq_handler
