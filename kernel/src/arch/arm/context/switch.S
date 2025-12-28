.global context_switch
context_switch:
    /* r0 = old context, r1 = new context */
    /* save callee-saved regs */
    stmia r0!, {r4-r11, sp, lr}
    ldmia r1!, {r4-r11, sp, lr}
    bx lr
