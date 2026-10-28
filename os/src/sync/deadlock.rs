//! Deadlock detection for blocking mutexes and semaphores.
//!
//! A process may opt into deadlock detection through
//! `sys_enable_deadlock_detect`. Once enabled, before a task is put to sleep
//! on a blocking mutex or a semaphore, the kernel asks whether the task could
//! still make progress after sleeping. If it could not, the waiting syscall
//! returns `-0xdead` to user space instead of blocking, letting the program
//! observe and recover from the deadlock.
//!
//! A task is regarded as able to progress if:
//! - it is runnable (not blocked), or
//! - it blocks on something other than a mutex/semaphore (e.g. a timer), or
//! - it waits on a mutex whose owner can progress, or
//! - it waits on a semaphore and at least one current holder can progress (a
//!   semaphore with no holder counts as progressable, since another task may
//!   still perform an `up`).

use crate::task::{ProcessControlBlockInner, TaskStatus};
use alloc::collections::BTreeSet;

/// The resource a blocked task is waiting for.
///
/// Recorded in the task control block right before the task blocks and
/// cleared once it is woken. Tasks sleeping on a timer, a condvar, etc. carry
/// no wait target.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum WaitTarget {
    /// waiting on a blocking mutex
    Mutex(usize),
    /// waiting on a semaphore
    Semaphore(usize),
}

/// Returns `true` if the current task would deadlock were it to block on
/// `target`.
///
/// `inner` is the inner of the process that owns the current task, and `tid`
/// is the current task's tid.
pub fn detect_deadlock(
    inner: &ProcessControlBlockInner,
    tid: usize,
    target: WaitTarget,
) -> bool {
    let mut seen = BTreeSet::new();
    !resource_releasable(inner, target, tid, &mut seen)
}

/// Whether the resource named by `target` can eventually be released.
fn resource_releasable(
    inner: &ProcessControlBlockInner,
    target: WaitTarget,
    self_tid: usize,
    seen: &mut BTreeSet<usize>,
) -> bool {
    match target {
        WaitTarget::Mutex(id) => {
            let owner = inner
                .mutex_list
                .get(id)
                .and_then(|m| m.as_ref())
                .and_then(|m| m.owner());
            match owner {
                Some(owner) => task_may_advance(inner, owner, self_tid, seen),
                None => true, // the mutex is currently free
            }
        }
        WaitTarget::Semaphore(id) => {
            match inner.semaphore_list.get(id).and_then(|s| s.as_ref()) {
                Some(sem) => {
                    if sem.count() > 0 {
                        // at least one resource is free
                        return true;
                    }
                    let holders = sem.holders();
                    if holders.is_empty() {
                        // nobody holds a resource; an up() may still arrive
                        return true;
                    }
                    holders
                        .iter()
                        .any(|h| task_may_advance(inner, *h, self_tid, seen))
                }
                None => true,
            }
        }
    }
}

/// Whether the task `tid` can eventually run and release what it holds.
fn task_may_advance(
    inner: &ProcessControlBlockInner,
    tid: usize,
    self_tid: usize,
    seen: &mut BTreeSet<usize>,
) -> bool {
    if tid == self_tid {
        // this is exactly the task that is about to sleep
        return false;
    }
    let task = match inner.tasks.get(tid).and_then(|t| t.as_ref()) {
        Some(task) => task,
        // the task has already exited (or never existed)
        None => return true,
    };
    let (status, wait) = {
        let tc_inner = task.inner_exclusive_access();
        (tc_inner.task_status, tc_inner.wait_target)
    };
    if status != TaskStatus::Blocked {
        return true;
    }
    let wait = match wait {
        Some(wait) => wait,
        // blocked on a timer/condvar/...: it will be woken eventually
        None => return true,
    };
    // A cycle made only of blocked tasks can never make progress.
    if !seen.insert(tid) {
        return false;
    }
    let can = resource_releasable(inner, wait, self_tid, seen);
    seen.remove(&tid);
    can
}
