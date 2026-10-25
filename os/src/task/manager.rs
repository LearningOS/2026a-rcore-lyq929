//! The ready queue with a stride scheduler.
//!
//! Each process carries a `pass` counter. The scheduler always picks the
//! process with the smallest `pass` to run. Before a process is put back on
//! the queue we add its own `stride` to its `pass`, so a high-priority
//! process (small stride) gets selected more often, yet every process still
//! makes forward progress in the long run.

use super::TaskControlBlock;
use crate::sync::UPSafeCell;
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use lazy_static::*;

/// Owns the ready queue and implements the stride picking rule.
pub struct TaskManager {
    ready_queue: VecDeque<Arc<TaskControlBlock>>,
}

impl TaskManager {
    /// Start with an empty queue.
    pub fn new() -> Self {
        Self {
            ready_queue: VecDeque::new(),
        }
    }

    /// Re-queue a process and charge it for the time slice it just ran.
    pub fn add(&mut self, task: Arc<TaskControlBlock>) {
        // Charge one stride: the next time this process is fetched, its
        // `pass` already reflects the slice it just consumed.
        let mut inner = task.inner_exclusive_access();
        inner.pass += inner.stride;
        drop(inner);
        self.ready_queue.push_back(task);
    }

    /// Take out the process with the smallest accumulated `pass`.
    pub fn fetch(&mut self) -> Option<Arc<TaskControlBlock>> {
        let idx = self
            .ready_queue
            .iter()
            .enumerate()
            .min_by_key(|(_, t)| t.inner_exclusive_access().pass)?
            .0;
        self.ready_queue.remove(idx)
    }
}

lazy_static! {
    /// Global ready queue shared by all cores.
    pub static ref TASK_MANAGER: UPSafeCell<TaskManager> =
        unsafe { UPSafeCell::new(TaskManager::new()) };
}

/// Push a task onto the global ready queue.
pub fn add_task(task: Arc<TaskControlBlock>) {
    TASK_MANAGER.exclusive_access().add(task);
}

/// Pop the next task off the global ready queue.
pub fn fetch_task() -> Option<Arc<TaskControlBlock>> {
    TASK_MANAGER.exclusive_access().fetch()
}
