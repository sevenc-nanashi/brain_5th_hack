.syntax unified
.arm
.text
.balign 4
.global brain_setjmp
.type brain_setjmp,%function
brain_setjmp:
    stmia r0, {r4-r11, sp, lr}
    mov r0, #0
    bx lr
.global brain_longjmp
.type brain_longjmp,%function
brain_longjmp:
    ldmia r0, {r4-r11, sp, lr}
    mov r0, #1
    bx lr
