# __alltraps / __restore：进出内核 trap 的汇编入口。
#
# 进入用户态之前，内核会把 sscratch 设成用户栈顶；用户态一旦发生 trap，
# 硬件自动把 sscratch 与 sp 交换，于是 sp 立刻指向内核栈（前提是内核已为
# 任务准备好了一块内核栈并在 sscratch 里存好它的栈顶）。

.altmacro
# 把 x<n> 保存到 sp 的 n*8 偏移处。
.macro STORE_GP n
    sd x\n, \n*8(sp)
.endm
# 从 sp 的 n*8 偏移处装载 x<n>。
.macro LOAD_GP n
    ld x\n, \n*8(sp)
.endm

    .section .text
    .globl __alltraps
    .globl __restore
    .align 2

__alltraps:
    # 交换 sp 与 sscratch：sp -> 内核栈，sscratch -> 用户栈。
    csrrw sp, sscratch, sp
    # 在内核栈顶为 TrapContext 腾出 34 个寄存器槽（34*8 字节）。
    addi sp, sp, -34*8
    # 保存通用寄存器（跳过 x0；x2 稍后单独保存；x4 是 tp，用户程序不用）。
    sd x1, 1*8(sp)      # ra
    sd x3, 3*8(sp)      # gp
    .set n, 5           # 从 x5 开始
    .rept 27
        STORE_GP %n
        .set n, n + 1
    .endr
    # 此时 t0/t1/t2 可以放心使用（它们已经在栈上被保存过了）。
    csrr t0, sstatus
    csrr t1, sepc
    sd t0, 32*8(sp)     # sstatus
    sd t1, 33*8(sp)     # sepc
    # 把用户栈顶（此刻还在 sscratch 里）读出来存进 x2 槽位。
    csrr t2, sscratch
    sd t2, 2*8(sp)
    # 传参：a0 = TrapContext 指针，交给 trap_handler(cx: &mut TrapContext)。
    mv a0, sp
    call trap_handler

__restore:
    # 现在 sp 指向内核栈上那块 TrapContext（分配后），sscratch 是用户栈顶。
    # 先恢复 sstatus/sepc/sscratch。
    ld t0, 32*8(sp)
    ld t1, 33*8(sp)
    ld t2, 2*8(sp)
    csrw sstatus, t0
    csrw sepc, t1
    csrw sscratch, t2
    # 恢复除 sp(x2)、tp(x4) 外的通用寄存器。
    ld x1, 1*8(sp)
    ld x3, 3*8(sp)
    .set n, 5
    .rept 27
        LOAD_GP %n
        .set n, n + 1
    .endr
    # 释放 TrapContext 占用的栈空间。
    addi sp, sp, 34*8
    # 再把 sp 与 sscratch 换回来：sp -> 用户栈，sscratch -> 内核栈，为下次 trap 备用。
    csrrw sp, sscratch, sp
    sret   # 依据 sepc 与 sstatus 回到用户态
