# 2026a rCore ch3实验报告

## 一、实现功能总结

本实验在第三章完成了基于时间片轮转的多任务内核。用户程序在编译期被打包进内核镜像，由 loader 加载到各自的固定物理区间，并为每个任务初始化 `TrapContext`。trap 模块统一处理异常、中断与系统调用；task 模块维护任务状态与上下文切换，配合时钟中断实现时间片轮转调度。实现了 `write`、`exit`、`yield`、`gettime`、`trace` 五个系统调用，并完善了 U/S 态切换、内核/用户栈隔离，以及非法访存、非法指令的处理。

## 二、问答题

### 问题 1：正确进入 U 态后，程序的特征——使用 S 态特权指令、访问 S 态寄存器会报错

**测试环境**：QEMU virt 机器，SBI 为 **RustSBI 0.4.1**（使用仓库 `bootloader/rustsbi-qemu.bin`，对应 `rustsbi-qemu.toml` 中 `rustsbi_version = "0.4.1"`、tag `v0.4.1`）。

正确进入 U 态后，CPU 处于用户特权级，硬件会禁止执行任何 S 态（supervisor）特权指令、禁止访问 S 态 CSR。一旦用户程序尝试这些操作，就会触发 `IllegalInstruction` 异常，控制权交给内核；同时，用户程序也不能访问内核只允许访问的地址区间，越界访存会触发访存错误。三个测例正好覆盖了这几类非法行为，内核都将其识别为非法并终止肇事任务：

| 测例 | 用户程序行为 | 触发异常 | 内核处理与输出 |
| --- | --- | --- | --- |
| `ch2b_bad_address` | 向地址 `0x0` 执行一次写（`(0x0 as *mut u8).write_volatile(0)`） | `StoreFault`（存储访问错误） | 打印 `[kernel] PageFault in application, bad addr = 0x0, bad instruction = <指令地址>, kernel killed it.`，调用 `exit_current_and_run_next()` 杀掉该任务 |
| `ch2b_bad_instructions` | 在 U 态执行 S 态特权指令 `sret` | `IllegalInstruction`（非法指令） | 打印 `[kernel] IllegalInstruction in application, kernel killed it.`，杀掉该任务 |
| `ch2b_bad_register` | 在 U 态读取 S 态 CSR `sstatus`（`csrr sstatus`） | `IllegalInstruction`（非法指令） | 打印 `[kernel] IllegalInstruction in application, kernel killed it.`，杀掉该任务 |

**分析**：`ch2b_bad_instructions` 与 `ch2b_bad_register` 都落在 `trap_handler` 的 `IllegalInstruction` 分支——说明硬件在 U 态下对特权指令和 S 态 CSR 的访问一律判为非法，直接交 trap 给内核；`ch2b_bad_address` 落在 `StoreFault | StorePageFault` 分支——说明用户程序只能访问属于自己的地址空间，对地址 `0x0` 这种非法区间（不在内核可访问的 RAM 范围内）的访问同样会被硬件捕获并交给内核。三者共同印证：进入 U 态后，用户程序的"特权能力"被严格限制，任何越权/越界行为都会触发 trap，由内核兜底处理并终止肇事程序。

### 问题 2：深入理解 `trap.S` 中 `__alltraps` 与 `__restore`

> 以下行号均指 `os/src/trap/trap.S`（教程原始布局）。

#### 1. L40：刚进入 `__restore` 时，`sp` 代表什么值？`__restore` 的两种使用情景

刚进入 `__restore` 时，`sp` 指向**当前任务内核栈**上保存的那块 `TrapContext` 的起始地址——即在 `__alltraps` 中 `addi sp, sp, -34*8` 之后、`mv a0, sp` 传给 `trap_handler` 的那个地址。此时 `sscratch` 保存的是**用户栈栈顶**。

`__restore` 的两种使用情景：

1. **作为 `trap_handler` 返回后的落点**：`__alltraps` 里 `call trap_handler` 与 `__restore` 紧邻，二者之间没有 `ret`，所以 `trap_handler` 返回地址正好就是 `__restore`。每次 trap 处理完，控制流自动落入 `__restore`，用它（可能被 `trap_handler` 修改过的）`TrapContext` 恢复现场、返回用户态。
2. **作为 `__switch` 的跳转目标（任务首次被调度）**：任务初始化时，`TaskContext::goto_restore` 把 `ra` 设为 `__restore`、`sp` 设为该任务内核栈上 `TrapContext` 的地址。当 `__switch` 第一次切到这个任务时，它从旧任务的上下文装载出新任务上下文后 `ret`，就跳到了 `__restore`，从而依据该任务的 `TrapContext` 第一次进入用户态。

#### 2. L43-L48：这几行特殊处理了哪些寄存器？这些值对进入用户态有何意义？

```asm
ld t0, 32*8(sp)   # 从 TrapContext 取出 sstatus
ld t1, 33*8(sp)   # 取出 sepc
ld t2, 2*8(sp)    # 取出用户栈栈顶（TrapContext.x[2]）
csrw sstatus, t0
csrw sepc, t1
csrw sscratch, t2
```

这三行处理的是**三个 CSR（控制状态寄存器）**，而不是普通通用寄存器——普通寄存器用 `ld` 直接装载即可，但 CSR 必须先用 `ld` 读到临时寄存器 `t0/t1/t2`，再用 `csrw` 写回。它们的值对进入用户态分别意味着：

- **`sstatus`**：其 `SPP` 位记录"当前应该返回的特权级"。`app_init_context` 在构造现场时已把 `SPP` 置为 `User`，写回它后，接下来的 `sret` 就会回到用户态；同时它还携带全局中断使能等状态。
- **`sepc`**：用户态发生 trap 时的下一条指令地址。对系统调用而言，`trap_handler` 已把它 `+4`（跳过 `ecall`）。写回它后，`sret` 就从这条指令继续执行用户程序。
- **`sscratch`**：用户栈栈顶。写回它后，紧接着的 `csrrw sp, sscratch, sp` 才能把 `sp` 换回用户栈；同时它也为"下一次用户态 trap 的栈交换"备好了正确的值。

#### 3. L50-L56：为何跳过了 `x2` 和 `x4`？

```asm
ld x1, 1*8(sp)
ld x3, 3*8(sp)
.set n, 5
.rept 27
    LOAD_GP %n
    .set n, n+1
.endr
```

这段把通用寄存器 `x1`、`x3` 以及 `x5~x31` 从栈上装回。

- **跳过 `x2`（`sp`）**：`sp` 此刻还指着内核栈上的 `TrapContext`，是后续 `ld`/`csrw` 的寻址基址，绝不能在这里覆盖。`sp` 的真正恢复放在最后：先用 `addi sp, sp, 34*8` 释放 `TrapContext` 空间，再用 `csrrw sp, sscratch, sp` 换回用户栈顶（L60）。
- **跳过 `x4`（`tp`）**：`tp` 是线程指针（Thread Pointer），rCore 的用户程序并不使用它，而且它是 ABI 约定的专用寄存器，与其恢复一个不确定的值，不如直接不恢复。（`x0` 恒为 0，也无需恢复。）

#### 4. L60：`csrrw sp, sscratch, sp` 之后，`sp` 和 `sscratch` 中的值分别有什么意义？

```asm
addi sp, sp, 34*8          # L58：释放 TrapContext，sp 回到内核栈顶
csrrw sp, sscratch, sp     # L60：原子交换 sp 与 sscratch
```

L60 时：`sp` 已经回到**内核栈顶**，`sscratch` 是**用户栈顶**（L55 刚写回）。`csrrw` 是原子交换，执行后：

- `sp = 用户栈栈顶`——`sret` 后用户代码就在这个栈上运行；
- `sscratch = 内核栈栈顶`——为下一次用户态 trap 做好准备：一旦用户态再发生 trap，`__alltraps` 的第一条 `csrrw`（L13）就会把内核栈栈顶换进 `sp`。

#### 5. `__restore` 中发生状态切换的是哪一条指令？为何该指令执行之后会进入用户态？

发生状态切换的是 `__restore` 末尾的 **`sret`** 指令。`sret` 会做两件事：把 `pc` 设为 `sepc` 的值（即用户程序下一条要执行的指令地址，L54 已写回），并根据 `sstatus.SPP` 把特权级恢复到用户态（L53 已把 `SPP` 置为 `User`）。因此 `sret` 之后 CPU 进入 U 态，从用户程序继续执行；同时 `sp` 已是用户栈、`sscratch` 已是内核栈，后续运行与下次 trap 都就绪了。

#### 6. L13：`csrrw sp, sscratch, sp`（在 `__alltraps` 中）之后，`sp` 和 `sscratch` 中的值分别有什么意义？

```asm
__alltraps:
    csrrw sp, sscratch, sp   # L13
```

这是用户态 trap 进入内核入口 `__alltraps` 后的第一条指令。进入时：`sp` 仍是**用户栈栈顶**，`sscratch` 保存着**内核栈栈顶**（由上一次 `__restore` 的 L60 交换得到）。`csrrw` 原子交换后：

- `sp = 内核栈栈顶`——内核的 trap 处理（保存现场、跑 `trap_handler`）都在这块内核栈上进行，不污染用户栈；
- `sscratch = 用户栈栈顶`——保存起来，供 `__restore` 时取回（L55/L60），保证栈隔离与下次切换可用。

#### 7. 从 U 态进入 S 态是哪一条指令发生的？

从 U 态进入 S 态**并不是由某一条软件指令显式完成的**，而是由**硬件在 trap（异常 / 中断 / `ecall`）发生时自动完成**的：当用户态发生 trap，处理器自动把特权级切到 S 态、把返回地址等保存到 `sepc`/`sstatus`，并把 `pc` 跳转到 `stvec` 指向的入口（即 `__alltraps`）。L13 的 `csrrw` 只负责栈的切换（把 `sp` 换到内核栈），并不改变特权级；真正的 U→S 切换发生在硬件取到 trap 的那一瞬间。与之相对，S→U 的切换则是在 `__restore` 末尾的 `sret` 指令（问题 5）由硬件依据 `sepc` 与 `sstatus.SPP` 完成的。

## 三、荣誉准则

1. 在完成本次实验的过程（含此前学习的过程）中，我曾分别与 以下各位 就（与本次实验相关的）以下方面做过交流，还在代码中对应的位置以注释形式记录了具体的交流对象及内容：
《你交流的对象说明》
2. 此外，我也参考了 以下资料 ，还在代码中对应的位置以注释形式记录了具体的参考来源及内容：
《你参考的资料说明》
3. 我独立完成了本次实验除以上方面之外的所有工作，包括代码与文档。 我清楚地知道，从以上方面获得的信息在一定程度上降低了实验难度，可能会影响起评分。
4. 我从未使用过他人的代码，不管是原封不动地复制，还是经过了某些等价转换。 我未曾也不会向他人（含此后各届同学）复制或公开我的实验代码，我有义务妥善保管好它们。 我提交至本实验的评测系统的代码，均无意于破坏或妨碍任何计算机系统的正常运转。 我清楚地知道，以上情况均为本课程纪律所禁止，若违反，对应的实验成绩将按“-100”分计。

## 四、（选填）实验感想

本次实验的核心是把"特权级隔离"落到实处：用户程序在 U 态运行，一越界就被硬件 trap 交给内核。任务切换与时间片轮转看似简单，真正动手体会 `__alltraps`/`__restore` 的保存恢复、`__switch` 的上下文切换后，才理解"栈隔离 + 寄存器现场"对多任务意味着什么。整体工作量适中，难点主要在汇编与 `TrapContext`/`TaskContext` 的布局对应关系上。
