.global context_save
context_save:
    // FIXME: Save the remaining context to the stack.
    stp x0,  x1,  [sp, #-16]!
    stp x2,  x3,  [sp, #-16]!
    stp x18, x19, [sp, #-16]!
    stp x20, x21, [sp, #16]!
    stp x22, x23, [sp, #16]!
    stp x24, x25, [sp, #16]!
    stp x26, x27, [sp, #16]!
    stp x28, x29, [sp, #16]!

    // x0: Info, x1: esr, x2: tr
    mov x0, x29
    mrs x1, ESR_EL1
    mov x2, xzr

    stp xzr, lr, [sp, #-16]!

    bl  handle_exception

.global context_restore
context_restore:
    // FIXME: Restore the context from the stack.
    ldp xzr, lr, [sp], #16

    ldp x28, x29, [sp], #16
    ldp x26, x27, [sp], #16
    ldp x24, x25, [sp], #16
    ldp x22, x23, [sp], #16
    ldp x20, x21, [sp], #16
    ldp x18, x19, [sp], #16
    ldp x2,  x3,  [sp], #16
    ldp x0,  x1,  [sp], #16
    ret

.macro HANDLER source, kind
    .align 7
    stp     lr, xzr, [SP, #-16]!
    stp     x28, x29, [SP, #-16]!

    mov     x29, \source
    movk    x29, \kind, LSL #16
    bl      context_save

    ldp     x28, x29, [SP], #16
    ldp     lr, xzr, [SP], #16
    eret
.endm

.align 11
.global vectors
vectors:
    // FIXME: Setup the 16 exception vectors.
    HANDLER 0, 0
    HANDLER 0, 1
    HANDLER 0, 2
    HANDLER 0, 3
    HANDLER 1, 0
    HANDLER 1, 1
    HANDLER 1, 2
    HANDLER 1, 3
    HANDLER 2, 0
    HANDLER 2, 1
    HANDLER 2, 2
    HANDLER 2, 3
    HANDLER 3, 0
    HANDLER 3, 1
    HANDLER 3, 2
    HANDLER 3, 3
