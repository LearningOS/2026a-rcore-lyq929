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

use crate::config::MAX_APP_NUM;
use crate::loader::{get_num_app, init_app_cx};
use crate::sync::UPSafeCell;
use lazy_static::*;
use switch::__switch;
pub use task::{TaskControlBlock, TaskStatus};
pub use context::TaskContext;

// ========== 全局系统调用计数器（重点！不侵入TCB）==========
use crate::syscall::process::SYS_CALL_COUNT;
static mut SYSCALL_COUNT: [[usize; SYS_CALL_COUNT]; MAX_APP_NUM] = [[0; SYS_CALL_COUNT]; MAX_APP_NUM];

/// 给当前任务增加对应syscall计数
pub fn inc_syscall_cnt(app_id: usize, sys_id: usize) {
    unsafe {
        SYSCALL_COUNT[app_id][sys_id] += 1;
    }
}
/// 获取指定任务指定系统调用的计数，给 sys_trace
pub fn get_syscall_cnt(app_id: usize, sys_id: usize) -> usize {
    unsafe {
        SYSCALL_COUNT[app_id][sys_id]
    }
}
// ======================================================

/// The task manager, where all the tasks are managed.
pub struct TaskManager {
    num_app: usize,
    inner: UPSafeCell<TaskManagerInner>,
}

struct TaskManagerInner {
    tasks: [TaskControlBlock; MAX_APP_NUM],
    current_task: usize,
}

lazy_static! {
    pub static ref TASK_MANAGER: TaskManager = {
        let num_app = get_num_app();
        let mut tasks = [TaskControlBlock {
            task_status: TaskStatus::UnInit,
            task_cx: TaskContext::zero_init(),
        }; MAX_APP_NUM];
        for i in 0..num_app {
            tasks[i].task_status = TaskStatus::Ready;
            // 注意：这里保存的是 TaskContext，不是 TrapContext！
            // init_app_cx 返回 TrapContext 地址，是 trap 入口用的，不要混！
            tasks[i].task_cx = TaskContext::goto_trap_return(init_app_cx(i));
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
    /// Run the first task in task list.
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

    /// Yield current task, switch to next ready task (时间片轮转)
    pub fn yield_current(&self) {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].task_status = TaskStatus::Ready;
        // 找下一个就绪任务，轮转
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

    /// Sleep 系统调用：把当前任务置为UnInit（ch3 sleep要求）
    pub fn sleep_current(&self) {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].task_status = TaskStatus::UnInit;
        // 找下一个就绪任务
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
