# rCore 实验 Lab3报告（进程与任务管理）

## 一、问答题

### 1. 两个 pass = 10 的进程，用 8bit 无符号整数存储 stride：p1.stride = 255，p2.stride = 250。在 p2 执行一个时间片后，实际轮到谁执行？为什么？

**实际轮到 p2，而不是理论上应该轮到的 p1。**

原因在于**无符号整数的溢出回绕（wrap-around）**破坏了 pass 之间的相对大小关系：

- p2 执行完一个时间片后，按算法 `pass += stride`：
  `p2.pass = 10 + 250 = 260`。
- 但 stride 用 **8bit 无符号整数** 存储，最大只能表示 255，260 溢出后回绕为：
  `260 mod 256 = 4`，即 `p2.pass` 变成了 4。
- 此时 `p1.pass = 10`，`p2.pass = 4`，调度器按"取 pass 最小者"选出的是 **p2**。

而理论上，p2 已经运行过一个时间片，它的真实 pass（260）应当比 p1 的 10 大，下一次应轮到 p1。溢出让 p2 的 pass 回绕成了一个远小于 p1 的值，导致调度器误判，p2 被重复调度。

### 2. 为什么在进程优先级全部 >= 2 时，不溢出的情况下有 `STRIDE_MAX - STRIDE_MIN <= BigStride / 2`？简单说明。

要点：每个进程的 `stride = BigStride / priority`，而 `priority >= 2`，所以**任意进程的 stride 都不超过 `BigStride / 2`**。再结合 Stride 调度的选取规则，就能把 pass 的跨度限制住：

1. 调度器总是选取 **当前 pass 最小** 的进程运行。
2. 被选中的进程执行完一个时间片后，`pass` 增加它自己的 `stride`（≤ `BigStride / 2`）。
3. 增加后，该进程的 pass 至多比"调度前的最小 pass"大一个 stride（因为其它进程的 pass 都不小于调度前的最小 pass）。
4. 因此，任意时刻全体进程的 pass 都落在区间 `[min, min + BigStride/2]` 内。

于是 `STRIDE_MAX - STRIDE_MIN <= BigStride / 2`。这个结论的价值在于：**pass 的差值始终被约束在 `2^(n-1)` 以内**（`BigStride = 2^n` 时），这恰好是无符号回绕判断（Q3）能用最高位区分"真实大小"的前提——正是优先级 >= 2 这条限制为溢出安全比较器提供了保证。

### 3. 已知上述结论，在考虑溢出的情况下，为 Stride 设计比较器，让 `BinaryHeap<Stride>` 的 `pop()` 返回真正最小的 Stride。补全 `partial_cmp`。

**核心思路**：由于任意两个 Stride 的真实差值被约束在 `[0, BigStride/2]` 内（即小于 `2^(n-1)`），计算回绕差 `self.0.wrapping_sub(other.0)` 后，**其最高位恰好指示了 self 相对 other 的"真实"大小**：

- 若 self 真正更大（`self >= other`，真实差非负且 < `2^(n-1)`），回绕差的高位为 **0**；
- 若 self 真正更小（真实差为负，回绕后落在 `[2^(n-1), 2^n)`），回绕差的高位为 **1**。

`BinaryHeap` 是最大堆，`pop()` 返回"最大"元素。要让 `pop()` 返回**真正最小**的 Stride，就应把"真正更小"的 Stride 判为 `Greater`（视为更大而置于堆顶）：

```rust
use core::cmp::Ordering;

struct Stride(u64);

impl PartialOrd for Stride {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        // 回绕差：self - other（溢出时自动回绕）
        let diff = self.0.wrapping_sub(other.0);
        if diff >> 63 == 1 {
            // self 在回绕意义下“真正”小于 other → 视为更大，置顶，pop 返回真正最小
            Some(Ordering::Greater)
        } else {
            Some(Ordering::Less)
        }
    }
}

impl PartialEq for Stride {
    fn eq(&self, other: &Self) -> bool {
        false // 依题意两个 Stride 永不相等
    }
}
```

**用 TIPS 的 8bit 例子验证**（此时用最高位 bit7、即 `1 << 7 = 128` 判断，`BigStride = 255`）：

- 比较 `self = 125` 与 `other = 255`：`diff = 125.wrapping_sub(255) = -130 → 回绕 = 126`，最高位为 0，返回 `Less`，即 125 并非"真正更小"，对应 `(125 < 255) == false` ✓；
- 比较 `self = 129` 与 `other = 255`：`diff = 129.wrapping_sub(255) = -126 → 回绕 = 130`，最高位为 1，返回 `Greater`，即 129"真正更小"，对应 `(129 < 255) == true` ✓。

说明：若不需要配合最大堆、只想得到回绕安全的"真实大小顺序"，把 `Greater`/`Less` 两分支对调即可。上面的版本按题目要求，直接让 `BinaryHeap::pop()` 返回真正最小的 Stride。

## 二、功能总结

本实验完成了 rCore 第 5 章进程与任务管理的核心实现：引入以 `TaskControlBlock` 为核心的进程控制结构（含 pid、内核栈、独立地址空间与父子关系），实现 fork、exec、spawn、waitpid、exit 等进程生命周期系统调用；将就绪队列改造为 Stride 调度算法，按 pass 选取最小者运行、pass 随 stride 增长，并新增 set_priority 调整进程优先级；补齐 get_time、mmap、munmap 系统调用；实现 initproc 初始化与僵尸进程回收。

## 三、荣誉准则

请在提交前核对以下说明是否与你的实际情况一致，并在代码对应位置补充注释。

1. 在完成本次实验的过程（含此前学习的过程）中，我曾分别与 以下各位 就（与本次实验相关的）以下方面做过交流，还在代码中对应的位置以注释形式记录了具体的交流对象及内容。

2. 此外，我也参考了 **以下资料**，还在代码中对应的位置以注释形式记录了具体的参考来源及内容：

- rCore-Tutorial-Guide 第 5 章《进程与任务管理》官方文档（https://learningos.cn/rCore-Tutorial-Guide/chapter5/）
- 某份已完成 ch5 实验的实现（用于对照、理解 Stride 调度与系统调用的正确写法）

3. 我独立完成了本次实验除以上方面之外的所有工作，包括代码与文档。我清楚地知道，从以上方面获得的信息在一定程度上降低了实验难度，可能会影响起评分。

4. 我从未使用过他人的代码，不管是原封不动地复制，还是经过了某些等价转换。我未曾也不会向他人（含此后各届同学）复制或公开我的实验代码，我有义务妥善保管好它们。我提交至本实验的评测系统的代码，均无意于破坏或妨碍任何计算机系统的正常运转。我清楚地知道，以上情况均为本课程纪律所禁止，若违反，对应的实验成绩将按“-100”分计。

## 四、（可选）对本次实验设计及难度/工作量的看法

本次实验把"进程"这一抽象完整落地：从 TaskControlBlock、父子关系，到 fork/exec/spawn/waitpid/exit 的完整生命周期，再到 Stride 调度的实现与溢出处理，知识点密集但逻辑自洽，做完后对操作系统的进程管理有整体把握。其中 Stride 调度的溢出问答题是很好的加分设计，把"算法正确性"上升到"工程上的溢出与比较器"层面，值得保留。建议：可以在评测中增加对调度公平性（如不同优先级下各进程 CPU 占比）的定量校验，而不仅是功能正确；同时报告可给出评测用例与期望行为，方便定位实现问题。
