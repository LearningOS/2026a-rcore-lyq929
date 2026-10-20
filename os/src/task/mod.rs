//! Task management implementation
//!
//! Everything about task management, like starting and switching tasks is
//! implemented here.
//!
//! A single global instance of [`TaskManager`] called `TASK_MANAGER` controls
//! all the tasks in the operating system.
//!
//! Be careful when you see `__switch` ASM function in `switch.S`. Control flow around this function
//! might not be what you expect.
mod context;
mod switch;
#[allow(clippy::module_inception)]
mod task;

use core::ptr;
use crate::config::{MAX_APP_NUM, SYS_CALL_COUNT};
use crate::loader::{get_num_app, init_app_cx};
use crate::sync::UPSafeCell;
use lazy_static::*;
use switch::__switch;

pub use task::{TaskControlBlock, TaskStatus};
pub use context::TaskContext;

// 全局二维系统调用计数器：[app_id][sys_id]
static mut SYSCALL_COUNT: [[usize; SYS_CALL_COUNT]; MAX_APP_NUM] = [[0; SYS_CALL_COUNT]; MAX_APP_NUM];

/// 给当前任务的某个系统调用计数+1（volatile 防编译器优化）
pub fn inc_syscall_cnt(app_id: usize, sys_id: usize) {
    unsafe {
        let p = &mut SYSCALL_COUNT[app_id][sys_id] as *mut usize;
        let val = ptr::read_volatile(p);
        ptr::write_volatile(p, val + 1);
    }
}

/// 查询指定任务、指定系统调用的次数（给sys_trace，volatile读）
pub fn get_syscall_cnt(app_id: usize, sys_id: usize) -> usize {
    unsafe {
        let p = &SYSCALL_COUNT[app_id][sys_id] as *const usize;
        ptr::read_volatile(p)
    }
}

/// The task manager, where all the tasks are managed.
pub struct TaskManager {
    num_app: usize,
    inner: UPSafeCell<TaskManagerInner>,
}

struct TaskManagerInner {
    tasks: [TaskControlBlock; MAX_APP_NUM],
    current_task: usize,
}

// Global task manager instance, manages all user applications
lazy_static! {
    pub static ref TASK_MANAGER: TaskManager = {
        let num_app = get_num_app();
        let mut tasks = [TaskControlBlock {
            task_status: TaskStatus::UnInit,
            task_cx: TaskContext::zero_init(),
        }; MAX_APP_NUM];
        for i in 0..num_app {
            tasks[i].task_status = TaskStatus::Ready;
            // 原版模板：goto_restore，不是 goto_trap_return！
            tasks[i].task_cx = TaskContext::goto_restore(init_app_cx(i));
        }
        TaskManager {
            num_app,
            inner: unsafe { UPSafeCell::new(TaskManagerInner {
                tasks,
                current_task: 0,
            }) },
        }
    };
}

impl TaskManager {
    /// Start the first task, never return
    pub fn run_first_task(&self) -> ! {
        let mut inner = self.inner.exclusive_access();
        let task0 = &mut inner.tasks[0];
        task0.task_status = TaskStatus::Running;
        let next_task_cx_ptr = &task0.task_cx as *const TaskContext;
        drop(inner);
        let mut _unused = TaskContext::zero_init();
        unsafe {
            __switch(&mut _unused as *mut TaskContext, next_task_cx_ptr);
        }
        panic!("run_first_task returned!");
    }

    /// Get id of currently running task
    pub fn get_current_task_id(&self) -> usize {
        let inner = self.inner.exclusive_access();
        inner.current_task
    }

    /// Suspend current task and switch to next ready task
    pub fn suspend_current_and_run_next(&self) {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].task_status = TaskStatus::Ready;
        // 轮转找下一个就绪任务
        let mut next = (current + 1) % self.num_app;
        while inner.tasks[next].task_status != TaskStatus::Ready {
            next = (next + 1) % self.num_app;
        }
        inner.tasks[next].task_status = TaskStatus::Running;
        inner.current_task = next;
        let current_task_cx_ptr = &mut inner.tasks[current].task_cx as *mut TaskContext;
        let next_task_cx_ptr = &inner.tasks[next].task_cx as *const TaskContext;
        drop(inner);
        unsafe {
            __switch(current_task_cx_ptr, next_task_cx_ptr);
        }
    }

    /// Exit current task and switch to next ready task
    pub fn exit_current_and_run_next(&self) {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].task_status = TaskStatus::Exited;
        // 找下一个就绪
        let mut next = (current + 1) % self.num_app;
        while inner.tasks[next].task_status != TaskStatus::Ready {
            next = (next + 1) % self.num_app;
        }
        inner.tasks[next].task_status = TaskStatus::Running;
        inner.current_task = next;
        let current_task_cx_ptr = &mut inner.tasks[current].task_cx as *mut TaskContext;
        let next_task_cx_ptr = &inner.tasks[next].task_cx as *const TaskContext;
        drop(inner);
        unsafe {
            __switch(current_task_cx_ptr, next_task_cx_ptr);
        }
    }
}
