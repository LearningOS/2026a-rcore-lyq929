# rCore-Tutorial ch3 实验报告：多道程序与分时多任务

## 一、实验概述与目标

本章在前两章（应用程序执行环境、批处理系统）的基础上，引入**多道程序与分时多任务**：操作系统能够同时加载多个用户程序，通过**任务调度**让它们交替占用 CPU，并由**时钟中断**实现按时间片自动抢占，从而营造出"多个程序同时运行"的假象。

本实验需要达成的目标：

1. 理解任务（Task）的抽象与生命周期，掌握任务状态机。
2. 掌握基于保存/恢复上下文的**任务切换**机制（`__switch`）。
3. 掌握基于**时钟中断 + 时间片轮转**的分时调度原理。
4. 完成实验模板中预留的系统调用 `sys_trace`，实现用户程序对指定内存地址的读写与 syscall 调用次数统计。

## 二、实验环境

| 项目 | 版本/值 |
|---|---|
| 操作系统 | Ubuntu / WSL2（Linux） |
| Rust 工具链 | nightly-2024-05-02 |
| 目标平台 | riscv64gc-unknown-none-elf |
| RustSBI 固件 | 0.4.1（随仓库提供） |
| QEMU 模拟器 | qemu-system-riscv64（virt 机器） |
| 内核时钟频率 `CLOCK_FREQ` | 12_500_000（12.5 MHz） |
| 时间片 | `CLOCK_FREQ / 100 = 125000` ticks ≈ 10 ms |
| 用户程序上限 `MAX_APP_NUM` | 16 |

## 三、实验原理与核心机制

### 3.1 多道程序与任务的抽象

在 ch3 中，所有用户程序仍然以二进制的形式被打包进内核镜像（`link_app.S`）。启动时，`loader::load_apps()` 会把每个程序加载到固定物理地址：

```
APP_BASE_ADDRESS + i * APP_SIZE_LIMIT      // 0x80400000 + i * 0x20000
```

但与批处理不同，这些程序**不会"一次一个、退出即完"**，而是被抽象成若干个可独立调度、可暂停再继续的任务。

### 3.2 任务状态机与任务控制块

每个任务用 `TaskControlBlock`（TCB）描述：

```rust
pub struct TaskControlBlock {
    pub task_status: TaskStatus,   // 任务当前状态
    pub task_cx: TaskContext,      // 任务的上下文（用于切换）
}
```

任务生命周期由如下状态机刻画：

```
UnInit ──初始化──▶ Ready ──调度选中──▶ Running
                                    │  ▲
                yield / 时钟中断 ────┘（回到 Ready，等待下一次被调度）
                                    │
                              exit ─┴▶ Exited（不再参与调度）
```

- `Ready`：已就绪、可被调度，但未占用 CPU。
- `Running`：正在 CPU 上执行。
- 处于 `Running` 的任务，遇到 `sys_yield` 或时钟中断时回到 `Ready`，由调度器选出下一个任务。

### 3.3 任务调度器 `TaskManager`

内核维护全局单例 `TASK_MANAGER`，其中保存任务数组 `tasks: [TaskControlBlock; MAX_APP_NUM]` 与当前任务号 `current_task`。

初始化时，对每个任务调用 `TaskContext::goto_restore(init_app_cx(i))`：把该任务上下文的 `ra` 设为 `__restore`（trap 返回入口）、`sp` 设为该任务内核栈上已压好的 `TrapContext` 地址。这样，第一次被调度到该任务时，就会通过 trap 返回直接进入用户态运行用户程序。

调度相关的核心操作：

- `find_next_task()`：从 `current_task + 1` 开始按模 `num_app` 循环扫描，返回第一个 `Ready` 的任务 id；找不到则说明全部任务已 `Exited`，内核结束。
- `mark_current_suspended()` / `mark_current_exited()`：把当前任务状态改为 `Ready` / `Exited`。
- `run_next_task()`：选中下一个任务，把状态置为 `Running`，更新 `current_task`，然后调用 `__switch` 保存当前上下文、恢复下一个任务的上下文。
- `run_first_task()`：实验开始时运行 0 号任务。

模块级封装：

```rust
pub fn suspend_current_and_run_next() {   // 主动让出：当前任务 -> Ready，切到下一个
    mark_current_suspended();
    run_next_task();
}
pub fn exit_current_and_run_next() {      // 退出：当前任务 -> Exited，切到下一个
    mark_current_exited();
    run_next_task();
}
```

### 3.4 上下文切换 `__switch`

任务切换的本质是**保存当前任务的所有"现场"、恢复目标任务的"现场"**。`TaskContext` 保存了 `ra`、`sp` 以及 s0~s11 共 12 个被调用者保存（callee-saved）寄存器：

```rust
pub struct TaskContext {
    ra: usize,            // 切换回来时该从哪继续执行
    sp: usize,            // 内核栈指针
    s: [usize; 12],       // s0-s11
}
```

`__switch` 由汇编实现（`switch.S`），大致流程：把当前 `ra/sp/s0-s11` 保存到 `current_task_cx_ptr` 指向的内存，再从 `next_task_cx_ptr` 恢复，最后 `ret` 到下一个任务的 `ra`。由于被调用者保存寄存器不依赖调用方，`__switch` 返回后控制流便落到了目标任务上次离开的位置。

关键理解点：**`__switch` 只是"换了一组寄存器的值"，它自己并不知道切到了哪个任务**；而"切到谁"由调度器决定（传入不同的上下文指针）。这使 CPU 的核心得以复用，仅通过寄存器状态就实现了任务间的交替执行。

### 3.5 时间片轮转与时钟中断

仅靠 `sys_yield` 主动让出还不够——若某个程序死循环不主动让出 CPU，其他程序将永远得不到运行。因此 ch3 引入**抢占式调度**：

- 内核初始化时调用 `trap::enable_timer_interrupt()` 使能 S 态时钟中断，并调用 `timer::set_next_trigger()` 设置下一次时钟中断。
- `set_next_trigger()` 通过 SBI 的 `set_timer` 设定下一次中断时间为 `get_time() + CLOCK_FREQ/100`，即约 10ms 一个时间片。
- 当时间片耗尽，CPU 产生 `SupervisorTimer` 中断进入 `trap_handler`：

```rust
Trap::Interrupt(Interrupt::SupervisorTimer) => {
    set_next_trigger();            // 重新设置下一个时间片
    suspend_current_and_run_next(); // 当前任务让出，切换下一个任务
}
```

这样，每个任务最多连续运行一个时间片就会被强制让出，实现了**公平的分时轮转**。

### 3.6 相关系统调用

- `sys_yield`（编号 124）：主动放弃 CPU，供程序在等待时调用，体现"协作式"的一面。
- `sys_exit`（编号 93）：以退出码结束当前任务。
- `sys_get_time`（编号 169）：返回当前时间（秒 + 微秒）。
- `sys_trace`（编号 410）：本次实验需要实现，见下一节。

## 四、本次实验实现：`sys_trace` 系统调用

### 4.1 需求分析

模板在 `os/src/syscall/process.rs` 中预留了 `sys_trace` 的 TODO，初值为直接返回 `-1`。根据用户侧封装（`user/src/lib.rs`）与测试程序 `ch3_trace.rs`，`sys_trace` 需要支持三种请求，由 `TraceRequest` 枚举的判别值区分：

```rust
#[repr(C)]
pub enum TraceRequest {
    Read,     // = 0
    Write,    // = 1
    Syscall,  // = 2
}
```

对应三种功能：

| 请求 | 语义 | 期望行为 |
|---|---|---|
| `Read` | 读用户内存 | 返回地址 `id` 处 1 个字节的值（读成功返回字节值） |
| `Write` | 写用户内存 | 把字节 `data` 写入地址 `id`，返回 0 |
| `Syscall` | 统计调用次数 | 返回编号为 `id` 的 syscall 累计被调用的次数 |

测试程序对计数的要求（注意"sys_trace 调用本身也计入"）：

```rust
assert_eq!(2, count_syscall(SYSCALL_TRACE));   // 前两次 trace 调用后
assert_eq!(7, count_syscall(SYSCALL_TRACE));   // 更多 trace 调用后
assert_eq!(0, count_syscall(SYSCALL_WRITE));   // 尚未调用 write
```

### 4.2 设计思路

1. **计数表**：需要一个按 syscall id 索引的全局计数数组。ch3 是单核、无并发抢占临界问题的（同一时刻只有一个任务在跑），因此用单处理器内部可变性原语 `UPSafeCell<[usize; 512]>` 包住数组即可，下标即 syscall id。
2. **计数时机**：为了让"trace 本身也计入"，在 `syscall()` 分发入口处、对**任意** syscall 调用都先执行 `record_syscall(id)` 自增，再按 `syscall_id` 分发到具体实现。这样统计的是"从内核启动以来，编号为 id 的 syscall 一共被调用过多少次"。
3. **内存读写**：ch3 还没有地址空间/页表，用户程序直接加载进内核所在的物理内存空间运行，用户程序中的地址就是真实物理地址。因此 `Read`/`Write` 可以用裸指针直接读写，**无需权限检查**（权限检查是 ch4 引入页表后才需要考虑的）。

### 4.3 实现代码

在 `os/src/syscall/process.rs` 顶部新增计数表与记录函数：

```rust
use crate::sync::UPSafeCell;
use lazy_static::*;

/// syscall id 的上界，用于给调用次数表定大小
const SYSCALL_ID_MAX: usize = 512;

lazy_static! {
    /// 记录每个 syscall id 被调用的累计次数（sys_trace 本身也算一次）
    static ref SYSCALL_COUNT: UPSafeCell<[usize; SYSCALL_ID_MAX]> = unsafe {
        UPSafeCell::new([0usize; SYSCALL_ID_MAX])
    };
}

/// 每次进入任意 syscall 时调用，把该 syscall id 的计数加一
pub fn record_syscall(id: usize) {
    if id < SYSCALL_ID_MAX {
        SYSCALL_COUNT.exclusive_access()[id] += 1;
    }
}
```

实现 `sys_trace`：

```rust
pub fn sys_trace(trace_request: usize, id: usize, data: usize) -> isize {
    trace!("kernel: sys_trace");
    match trace_request {
        // Read：读地址 id 处的字节并返回其值
        0 => unsafe { core::ptr::read_volatile(id as *const u8) as isize },
        // Write：把 data 的低 8 位写入地址 id
        1 => {
            unsafe {
                core::ptr::write_volatile(id as *mut u8, data as u8);
            }
            0
        }
        // Syscall：返回该 syscall id 的累计调用次数
        2 => {
            if id < SYSCALL_ID_MAX {
                SYSCALL_COUNT.exclusive_access()[id] as isize
            } else {
                0
            }
        }
        _ => -1,
    }
}
```

并在 `os/src/syscall/mod.rs` 的分发入口处先记录计数：

```rust
pub fn syscall(syscall_id: usize, args: [usize; 3]) -> isize {
    record_syscall(syscall_id);     // 先记录，sys_trace 自身也会被计入
    match syscall_id { /* 原有分发逻辑不变 */ }
}
```

### 4.4 关键点分析

- **为什么用 `read_volatile` / `write_volatile`**：对用户内存的读写本质是对外部/可变内存的副作用，用 volatile 读写可防止编译器把这类访问优化掉，保证每次真实访问内存。
- **为什么计数放在分发入口而非 `sys_trace` 内部**：若放在 `sys_trace` 内部，`count_syscall(SYSCALL_TRACE)` 查表时"本次 trace 调用"还未入表，会少算一次，无法满足 `assert_eq!(2, count_syscall(SYSCALL_TRACE))` 的断言。放到 `syscall()` 入口，保证任意一次调用（包括 trace 本身）在真正处理前已先入账。
- **`#![deny(warnings)]`**：内核 crate 开启了对告警的严格检查，因此新增代码要避免产生未使用告警；`record_syscall` 声明为 `pub` 并被调用，不会触发 dead_code。

## 五、测试与验证

本章涉及的测试用户程序：

| 程序 | 验证内容 |
|---|---|
| `ch3_sleep.rs` | `get_time` 返回正确时间、`yield` 主动让出后可回到用户态 |
| `ch3_sleep1.rs` | `sleep`（定时睡眠）行为正确 |
| `ch3_trace.rs` | `sys_trace` 的读、写、syscall 计数三功能 |

`ch3_trace.rs` 主要断言：`get_time` 调用次数、`trace` 调用次数（含自身）、`write` 调用次数、`yield`/`exit` 次数，以及 `trace_read`/`trace_write` 对变量与代码段地址的读写正确性；全部通过后打印 `Test trace OK!`。`ch3_sleep.rs` 通过后打印 `Test sleep OK!`。

在已配置好的 Linux 实验环境中执行（本地评测，`CHAPTER=3`）：

```bash
make -C os run                # 编译内核 + 用户程序并启动 QEMU 观察
make -C ci-user test CHAPTER=3   # 使用课程检查器评测（全部测试 + 报告检查）
```

## 六、遇到的问题与解决

- **（示例）"trace 调用本身也计入"导致计数对不上**：一开始若把计数放在 `sys_trace` 内，`count_syscall(SYSCALL_TRACE)` 结果会比期望少 1。解决：把计数上移到 `syscall()` 分发入口统一记录。
- **（示例）内存读写被优化**：直接 `*(id as *const u8)` 可能被编译器优化掉，改用 `read_volatile`/`write_volatile` 保证真实读写。
- 切换分支后 `user/` 目录若未存在，需按课程说明先克隆固定版本的 `rCore-Tutorial-Test` 到 `user/`。

## 七、总结与体会

1. **"假并行"的本质**：分时多任务并非真的同时执行多个程序，而是通过频繁的上下文切换 + 时间片抢占，让每个程序"轮流快速前进"，宏观上呈现并行。
2. **上下文切换的代价与边界**：`__switch` 只保存/恢复被调用者保存寄存器，因此切换非常轻量；但正因如此，涉及跨任务的栈、页表、资源时必须显式处理，这为后续 ch4（地址空间）、ch5（进程）打下伏笔。
3. **抢占式 vs 协作式**：`sys_yield` 是协作式让出，时钟中断是抢占式让出；二者结合才能既公平又可控。
4. 通过实现 `sys_trace`，我理解了系统调用从用户态发起、经 `trap_handler` 分发、到内核具体实现的完整链路，并体会了"在哪里计数"这类边界条件对测试结果的影响。
