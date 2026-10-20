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

/// The task manager, where all the tasks are managed.
pub struct TaskManager {
    /// total number of tasks
    num_app: usize,
    /// use inner value to get mutable access
    inner: UPSafeCell<TaskManagerInner>,
}

/// Inner of Task Manager
struct TaskManagerInner {
    /// task list
    tasks: [TaskControlBlock; MAX_APP_NUM],
    /// id of current `Running` task
    current_task: usize,
}

lazy_static! {
    /// Global variable: TASK_MANAGER
    pub static ref TASK_MANAGER: TaskManager = {
        let num_app = get_num_app();
        let mut tasks = [TaskControlBlock {
            task_status: TaskStatus::UnInit,
            task_cx: TaskContext::zero_init(),
            syscall_cnt: [0; 411],
        }; MAX_APP_NUM];
        for (i, task) in tasks.iter_mut().enumerate() {
            if i < num_app {
                // init_app_cx 参数是app index，原版loader里面init_app_cx接收索引
                task.task_cx = init_app_cx(i);
                task.task_status = TaskStatus::Ready;
                task.syscall_cnt = [0; 411];
            } else {
                task.task_status = TaskStatus::UnInit;
            }
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
    ///
    /// Generally, the first task in task list is an idle task (we call it zero process later).
    /// But in ch3, we load apps statically, so the first task is a real app.
    pub fn run_first_task(&self) -> ! {
        let mut inner = self.inner.exclusive_access();
        let task0 = &mut inner.tasks[0];
        task0.task_status = TaskStatus::Running;
        let next_task_cx_ptr = &task0.task_cx as *const TaskContext;
        drop(inner);
        let mut _unused = TaskContext::zero_init();
        // before this, we should drop the exclusive lock
        unsafe {
            __switch(&mut _unused as *mut TaskContext, next_task_cx_ptr);
        }
        panic!("unreachable in run_first_task!");
    }

    /// Change the status of current `Running` task into `Ready`.
    fn mark_current_suspended(&self) {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].task_status = TaskStatus::Ready;
    }

    /// Change the status of current `Running` task into `Exited`.
    fn mark_current_exited(&self) {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].task_status = TaskStatus::Exited;
    }

    /// Find next task to run and return task id.
    ///
    /// In this case, we only return the first `Ready` task in task list.
    fn find_next_task(&self) -> Option<usize> {
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;
        (current + 1..current + self.num_app + 1)
            .map(|id| id % self.num_app)
            .find(|id| inner.tasks[*id].task_status == TaskStatus::Ready)
    }

    /// Switch current `Running` task to the task we have found,
    /// or there is no `Ready` task and we can exit with all applications completed
    fn run_next_task(&self) {
        if let Some(next) = self.find_next_task() {
            let mut inner = self.inner.exclusive_access();
            let current = inner.current_task;
            inner.tasks[next].task_status = TaskStatus::Running;
            inner.current_task = next;
            let current_task_cx_ptr = &mut inner.tasks[current].task_cx as *mut TaskContext;
            let next_task_cx_ptr = &inner.tasks[next].task_cx as *const TaskContext;
            drop(inner);
            // before this, we should drop the exclusive lock
            unsafe {
                __switch(current_task_cx_ptr, next_task_cx_ptr);
            }
            // go back to user mode
        } else {
            panic!("All applications completed!");
        }
    }

    /// add count +=1 for given syscall id of current running task
    pub fn inc_syscall_cnt(&self, sys_id: usize) {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].syscall_cnt[sys_id] += 1;
    }

    /// read the counter of given syscall id for current running task
    pub fn get_syscall_cnt(&self, sys_id: usize) -> usize {
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].syscall_cnt[sys_id]
    }

    /// reset the counter of given syscall id for current running task to zero
    pub fn reset_syscall_cnt(&self, sys_id: usize) {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].syscall_cnt[sys_id] = 0;
    }

    /// Run the first task in task list; generally, it's used after init.
    pub fn run(&self) -> ! {
        self.run_first_task()
    }

    /// Suspend current task and run next ready task
    pub fn suspend_current_and_run_next(&self) {
        self.mark_current_suspended();
        self.run_next_task();
    }

    /// Exit the current task and run next ready task
    pub fn exit_current_and_run_next(&self) {
        self.mark_current_exited();
        self.run_next_task();
    }
}

/// increment counter of a syscall for current task
pub fn inc_syscall_cnt(sys_id: usize) {
    TASK_MANAGER.inc_syscall_cnt(sys_id);
}

/// read counter of a syscall for current task
pub fn get_syscall_cnt(sys_id: usize) -> usize {
    TASK_MANAGER.get_syscall_cnt(sys_id)
}

/// reset counter of a syscall for current task
pub fn reset_syscall_cnt(sys_id: usize) {
    TASK_MANAGER.reset_syscall_cnt(sys_id);
}

/// Suspend current task and run next ready task (对外导出，trap/process用)
pub fn suspend_current_and_run_next() {
    TASK_MANAGER.suspend_current_and_run_next();
}

/// Exit current task and run next ready task (对外导出，trap/process用)
pub fn exit_current_and_run_next() {
    TASK_MANAGER.exit_current_and_run_next();
}

/// run first task (main.rs调用)
pub fn run_first_task() -> ! {
    TASK_MANAGER.run_first_task()
}
