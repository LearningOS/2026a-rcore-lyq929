//! 内核的全局参数配置。
//!
//! 所有跟硬件资源划分、运行约束相关的“魔法数字”都集中在这里，而不是散落在
//! 各个模块里。这样一旦 QEMU 机器参数或用户程序布局发生变化，只需要改这一个
//! 文件，其它模块通过 `crate::config::*` 引用即可。

/// 应用的用户栈大小：8 KiB。
///
/// 第三章里每个任务共享同一个用户地址空间，用户栈的物理空间在编译期就被
/// 静态分配好了（见 `loader` 模块）。
pub const USER_STACK_SIZE: usize = 4096;

/// 内核栈大小：8 KiB。
///
/// 每个任务的内核栈也是静态数组，内核态（trap 处理、系统调用）时 CPU 会
/// 切到这块栈上工作，避免污染用户栈。
pub const KERNEL_STACK_SIZE: usize = 4096 * 2;

/// 内核堆大小：128 KiB，用于内核态里需要动态分配的数据。
pub const KERNEL_HEAP_SIZE: usize = 0x20000;

/// 最多支持的并发任务数量：16。
///
/// 这里用的是固定上限而非动态增长，是为了能用“数组 + 编译期定长”的方式
/// 表达任务表，规避 `no_std` 环境下动态内存分配的不确定性。
pub const MAX_APP_NUM: usize = 16;

/// 用户程序被加载到的起始物理地址（0x8040_0000）。
///
/// 第三章没有开分页，所有应用共享同一份地址空间，每个应用被放到一段
/// 互不重叠的固定区间里，区间首地址就是 `APP_BASE_ADDRESS + id * APP_SIZE_LIMIT`。
pub const APP_BASE_ADDRESS: usize = 0x8040_0000;

/// 单个用户程序占用的最大区间长度：128 KiB。
pub const APP_SIZE_LIMIT: usize = 0x20000;

/// `trace` 系统调用需要统计的系统调用种类上限：512。
///
/// 用一个二维计数器表 `[task][syscall_id]` 记录每个任务调用每种系统调用的次数，
/// 这里定义的就是那张表的第二维长度。
pub const MAX_SYSCALL_NUM: usize = 512;

/// 时钟频率：QEMU virt 机器默认的 time CSR 递增频率。
pub const CLOCK_FREQ: usize = 12_500_000;

/// 物理内存的结束地址：0x8800_0000（128 MiB 起始处的上方）。
///
/// 内核在第三章直接访问物理内存，所以用 `[MEMORY_START, MEMORY_END)` 来判定
/// 一个用户地址是否“安全可解引用”。
pub const MEMORY_END: usize = 0x8800_0000;
