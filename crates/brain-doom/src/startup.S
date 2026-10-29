.section .text.entry,"ax"
.arm
.balign 4
.global _start
.type _start,%function
_start:
    push {r4, lr}
    mov r4, r0
    ldr r1, =__data_load
    ldr r2, =__data_start
    ldr r3, =__data_end
2:
    cmp r2, r3
    ldrlo r12, [r1], #4
    strlo r12, [r2], #4
    blo 2b
    ldr r1, =__bss_start
    ldr r2, =__bss_end
    mov r3, #0
1:
    cmp r1, r2
    strlo r3, [r1], #4
    blo 1b
    mov r0, r4
    bl doom_main
    pop {r4, pc}
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
