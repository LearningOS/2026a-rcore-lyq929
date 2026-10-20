# __switch 汇编实现：任务上下文切换。
#
# 约定（与 TaskContext 结构体字段一一对应，Rust 侧用 #[repr(C)] 保证布局）：
#   a0 = current_task_cx_ptr（旧任务上下文的指针，用于保存现场）
#   a1 = next_task_cx_ptr    （新任务上下文的指针，用于恢复现场）
# 内存布局：offset 0 = ra，offset 8 = sp，offset 16.. = s0~s11（每格 8 字节）。

.altmacro
# 把 s<n> 保存到 a0 指向的 (n+2)*8 偏移处（n 从 0 开始对应 s0）。
.macro STORE_SREGS n
    sd s\n, (\n+2)*8(a0)
.endm
# 从 a1 指向的 (n+2)*8 偏移处装载 s<n>。
.macro LOAD_SREGS n
    ld s\n, (\n+2)*8(a1)
.endm

    .section .text
    .globl __switch
__switch:
    # 1) 先保存“旧现场”里 ra 和 sp，它们是切换回来继续执行的关键。
    sd sp, 8(a0)   # sp -> offset 8
    sd ra, 0(a0)   # ra -> offset 0

    # 2) 循环保存被调用者保存的 s0~s11（它们不能被 caller 破坏，必须由被调者负责）。
    .set n, 0
    .rept 12
        STORE_SREGS %n
        .set n, n + 1
    .endr

    # 3) 反过来，把新现场装载进来：先 ra 再 s0~s11，最后 sp。
    ld ra, 0(a1)
    .set n, 0
    .rept 12
        LOAD_SREGS %n
        .set n, n + 1
    .endr
    ld sp, 8(a1)

    # 4) ra/sp 都换好了，跳回新任务（ret 会跳到新 ra，等于“落进”新任务）。
    ret
