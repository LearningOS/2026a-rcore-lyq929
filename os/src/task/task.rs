//! Types related to task management
use super::TaskContext;

/// The task control block (TCB) of a task.
#[derive(Copy, Clone)]
pub struct TaskControlBlock {
    /// The task status in it's lifecycle
    pub task_status: TaskStatus,
    /// The task context
    pub task_cx: TaskContext,

    // ch3 sys‑trace per‑task syscall counters
    pub cnt_write: usize,
    pub cnt_exit: usize,
    pub cnt_yield: usize,
    pub cnt_gettime: usize,
    pub cnt_trace: usize,
}

/// The status of a task
#[derive(Copy, Clone, PartialEq)]
pub enum TaskStatus {
    /// uninitialized
    UnInit,
    /// ready to run
    Ready,
    /// running
    Running,
    /// exited
    Exited,
}
