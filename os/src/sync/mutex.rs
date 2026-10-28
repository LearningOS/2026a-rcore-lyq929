//! Mutex (spin-like and blocking(sleep))

use super::UPSafeCell;
use crate::task::TaskControlBlock;
use crate::task::{block_current_and_run_next, current_task, current_tid, wakeup_task};
use crate::task::suspend_current_and_run_next;
use alloc::{collections::VecDeque, sync::Arc};

/// Mutex trait
pub trait Mutex: Sync + Send {
    /// Lock the mutex
    fn lock(&self);
    /// Unlock the mutex
    fn unlock(&self);
    /// Whether the mutex is currently locked
    ///
    /// Only blocking mutexes answer this precisely; a spin mutex never sleeps
    /// while holding the lock.
    fn is_locked(&self) -> bool {
        false
    }
    /// The tid of the task that holds the mutex, meaningful only for blocking
    /// mutexes
    fn owner(&self) -> Option<usize> {
        None
    }
    /// Whether locking this mutex may put the caller to sleep
    fn is_blocking(&self) -> bool {
        false
    }
}

/// Spinlock Mutex struct
pub struct MutexSpin {
    locked: UPSafeCell<bool>,
}

impl MutexSpin {
    /// Create a new spinlock mutex
    pub fn new() -> Self {
        Self {
            locked: unsafe { UPSafeCell::new(false) },
        }
    }
}

impl Mutex for MutexSpin {
    /// Lock the spinlock mutex
    fn lock(&self) {
        trace!("kernel: MutexSpin::lock");
        loop {
            let mut locked = self.locked.exclusive_access();
            if *locked {
                drop(locked);
                suspend_current_and_run_next();
                continue;
            } else {
                *locked = true;
                return;
            }
        }
    }

    fn unlock(&self) {
        trace!("kernel: MutexSpin::unlock");
        let mut locked = self.locked.exclusive_access();
        *locked = false;
    }
}

/// Blocking Mutex struct
pub struct MutexBlocking {
    inner: UPSafeCell<MutexBlockingInner>,
}

pub struct MutexBlockingInner {
    locked: bool,
    /// the tid of the task holding the lock, consulted by deadlock detection
    owner: Option<usize>,
    wait_queue: VecDeque<Arc<TaskControlBlock>>,
}

impl MutexBlocking {
    /// Create a new blocking mutex
    pub fn new() -> Self {
        trace!("kernel: MutexBlocking::new");
        Self {
            inner: unsafe {
                UPSafeCell::new(MutexBlockingInner {
                    locked: false,
                    owner: None,
                    wait_queue: VecDeque::new(),
                })
            },
        }
    }
}

impl Mutex for MutexBlocking {
    /// lock the blocking mutex
    fn lock(&self) {
        trace!("kernel: MutexBlocking::lock");
        let mut inner = self.inner.exclusive_access();
        if inner.locked {
            inner.wait_queue.push_back(current_task().unwrap());
            drop(inner);
            block_current_and_run_next();
            // The unlocker transferred the lock to us, so we are the owner now.
            self.inner.exclusive_access().owner = Some(current_tid());
        } else {
            inner.locked = true;
            inner.owner = Some(current_tid());
        }
    }

    /// unlock the blocking mutex
    fn unlock(&self) {
        trace!("kernel: MutexBlocking::unlock");
        let mut inner = self.inner.exclusive_access();
        assert!(inner.locked);
        inner.owner = None;
        if let Some(task) = inner.wait_queue.pop_front() {
            wakeup_task(task);
        } else {
            inner.locked = false;
        }
    }

    fn is_locked(&self) -> bool {
        self.inner.exclusive_access().locked
    }

    fn owner(&self) -> Option<usize> {
        self.inner.exclusive_access().owner
    }

    fn is_blocking(&self) -> bool {
        true
    }
}
