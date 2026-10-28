//! Semaphore

use crate::sync::UPSafeCell;
use crate::task::{
    block_current_and_run_next, current_task, current_tid, wakeup_task, TaskControlBlock,
};
use alloc::{collections::VecDeque, sync::Arc};

/// semaphore structure
pub struct Semaphore {
    /// semaphore inner
    pub inner: UPSafeCell<SemaphoreInner>,
}

pub struct SemaphoreInner {
    pub count: isize,
    pub wait_queue: VecDeque<Arc<TaskControlBlock>>,
    /// tids of the tasks currently holding a resource of this semaphore, in
    /// the order the resources were acquired. Only used by deadlock detection.
    pub holders: VecDeque<usize>,
}

impl Semaphore {
    /// Create a new semaphore
    pub fn new(res_count: usize) -> Self {
        trace!("kernel: Semaphore::new");
        Self {
            inner: unsafe {
                UPSafeCell::new(SemaphoreInner {
                    count: res_count as isize,
                    wait_queue: VecDeque::new(),
                    holders: VecDeque::new(),
                })
            },
        }
    }

    /// The current number of free resources; negative means how many tasks are
    /// waiting.
    pub fn count(&self) -> isize {
        self.inner.exclusive_access().count
    }

    /// The tids of the tasks that currently hold a resource of this semaphore.
    pub fn holders(&self) -> Vec<usize> {
        self.inner
            .exclusive_access()
            .holders
            .iter()
            .copied()
            .collect()
    }

    /// up operation of semaphore
    pub fn up(&self) {
        trace!("kernel: Semaphore::up");
        let mut inner = self.inner.exclusive_access();
        inner.count += 1;
        let waking_task = if inner.count <= 0 {
            inner.wait_queue.pop_front()
        } else {
            None
        };
        // The resource released by the current task either goes to a waiting
        // task (which becomes a holder once it wakes) or back to the free pool.
        let cur = current_tid();
        if let Some(pos) = inner.holders.iter().rposition(|tid| *tid == cur) {
            inner.holders.remove(pos);
        } else {
            inner.holders.pop_back();
        }
        if let Some(task) = waking_task {
            wakeup_task(task);
        }
    }

    /// down operation of semaphore
    pub fn down(&self) {
        trace!("kernel: Semaphore::down");
        let mut inner = self.inner.exclusive_access();
        inner.count -= 1;
        if inner.count < 0 {
            inner.wait_queue.push_back(current_task().unwrap());
            drop(inner);
            block_current_and_run_next();
            // an up() of another task transferred a resource to us
            self.inner.exclusive_access().holders.push_back(current_tid());
        } else {
            inner.holders.push_back(current_tid());
        }
    }
}
