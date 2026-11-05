.syntax unified
.arm
.fpu neon-vfpv4
.section .text.inspector_fpu_probe,"ax",%progbits
.balign 4
.global inspector_fpu_probe
.type inspector_fpu_probe,%function
inspector_fpu_probe:
    push {{r4-r6, lr}}
    mov r6, r1
    mrs r3, cpsr
    str r3, [r6, #{cpsr}]
    vmrs r4, fpexc
    str r4, [r6, #{fpexc}]
    orr r3, r4, #0x40000000
    vmsr fpexc, r3
    isb
    vmrs r5, fpscr
    str r5, [r6, #{fpscr}]
    vmrs r3, mvfr0
    str r3, [r6, #{mvfr0}]
    and r3, r3, #15
    cmp r3, #2
    bne .Lfpu_return
    vpush {{d8-d15}}
    mov r3, #0
    vmsr fpscr, r3

    vldr s0, .Lsingle_a
    vldr s1, .Lsingle_b
    vldr s2, .Lsingle_c
    vldr s3, .Lsingle_d
    vadd.f32 s4, s0, s1
    vmul.f32 s4, s4, s2
    vdiv.f32 s4, s4, s3
    vstr s4, [r6, #{single}]

    vldmia r0!, {{d0-d15}}
    vldmia r0!, {{d16-d31}}
.Lfpu_loop:
    vadd.f64 d4, d0, d1
    vmul.f64 d4, d4, d2
    vdiv.f64 d4, d4, d3
    subs r2, r2, #1
    bne .Lfpu_loop
    vstr d4, [r6, #{double}]
    add r3, r6, #{registers}
    vstmia r3!, {{d0-d15}}
    vstmia r3!, {{d16-d31}}
    vmrs r3, fpscr
    str r3, [r6, #{fpscr_after}]
    vpop {{d8-d15}}
    vmsr fpscr, r5
.Lfpu_return:
    vmsr fpexc, r4
    isb
    pop {{r4-r6, pc}}
.balign 4
.Lsingle_a: .float 1.5
.Lsingle_b: .float 2.25
.Lsingle_c: .float 2.0
.Lsingle_d: .float 3.0
.size inspector_fpu_probe, .-inspector_fpu_probe
