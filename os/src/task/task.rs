//! 任务控制块（TCB）与任务状态的定义。

use super::TaskContext;

/// 任务控制块：描述一个任务“当前处于什么状态、下一次切换时的寄存器现场在哪”。
///
/// 第三章的任务体量很轻，所以 TCB 里只放了状态和上下文两样东西；后面章节
/// 引入地址空间、文件描述符等，会在这里不断扩充字段。
#[derive(Copy, Clone)]
pub struct TaskControlBlock {
    /// 任务在其生命周期中所处的状态。
    pub task_status: TaskStatus,
    /// 任务被换出时保存、被换入时恢复的上下文。
    pub task_cx: TaskContext,
}

/// 任务状态机。
///
/// 合法流转大致是：
/// `UnInit → Ready → Running → Ready/Exited`，其中 `Running` 每轮至多一个。
#[derive(Copy, Clone, PartialEq)]
pub enum TaskStatus {
    /// 尚未初始化。
    UnInit,
    /// 就绪，等待被调度。
    Ready,
    /// 正在 CPU 上运行。
    Running,
    /// 已退出，不再参与调度。
    Exited,
}
