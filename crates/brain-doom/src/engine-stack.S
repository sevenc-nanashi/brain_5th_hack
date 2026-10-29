.arm
.section .text.on_engine_stack,"ax"
.balign 4
.global on_engine_stack
.type on_engine_stack,%function
on_engine_stack:
    push {r4, lr}
    mov r4, sp
    mov sp, r1
    blx r0
    mov sp, r4
    pop {r4, pc}
