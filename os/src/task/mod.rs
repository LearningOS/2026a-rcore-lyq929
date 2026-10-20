//! 任务管理：任务从创建、调度到上下文切换的全过程。
//!
//! 内核里只维护一个全局的任务管理器 `TASK_MANAGER`，它掌握着：
//! - 一张定长的任务表（`tasks`，长度为 `MAX_APP_NUM`）；
//! - 当前正在运行的“任务”是哪一个（`current_task`）。
//!
//! 调度采用最简单的时间片轮转：时钟中断一到，就把当前任务挂起，轮着去
//! 找下一个处于 `Ready` 状态的任务接管 CPU。
//!
//! ⚠️ 特别注意 `switch.S` 里的 `__switch`：它只是“保存旧任务的上下文、装载
//! 新任务的上下文”，并不会主动改变控制流，真正让它生效的是被调用后寄存器
//! 被换掉了这一事实——所以它两边的代码执行顺序可能和你直觉相反。

mod context;
mod switch;
#[allow(clippy::module_inception)]
mod task;

use crate::config::{MAX_APP_NUM, MAX_SYSCALL_NUM};
use crate::loader::{get_num_app, init_app_cx};
use crate::sync::UPSafeCell;
use lazy_static::*;
use switch::__switch;
pub use task::{TaskControlBlock, TaskStatus};

pub use context::TaskContext;

/// 每个任务调用每种系统调用的次数统计表，索引是 `[task_id][syscall_id]`。
///
/// 这张表没有挂在 `TaskControlBlock` 里，而是单独放一个静态数组：一来避免
/// 任务表初始化时把这块较大的数组在小启动栈上拷贝来拷贝去；二来 `trace`
/// 系统调用可以直接用裸索引访问，不必绕道任务结构体。
static mut SYSCALL_COUNTS: [[u32; MAX_SYSCALL_NUM]; MAX_APP_NUM] =
    [[0; MAX_SYSCALL_NUM]; MAX_APP_NUM];

/// 任务管理器。
///
/// 它本身很小，真正可变的状态都藏在 `inner` 字段里，靠 `UPSafeCell` 在
/// 运行期做借用检查。好处是：对外只需持有一个 `&self` 就能完成所有状态
/// 迁移，不用到处传 `&mut`。
pub struct TaskManager {
    /// 任务总数。
    num_app: usize,
    /// 内层状态（任务表 + 当前任务编号）。
    inner: UPSafeCell<TaskManagerInner>,
}

/// 任务管理器的内部可变状态。
pub struct TaskManagerInner {
    /// 定长任务表。
    tasks: [TaskControlBlock; MAX_APP_NUM],
    /// 当前正在运行的任务编号。
    current_task: usize,
}

lazy_static! {
    /// 全局唯一任务管理器。
    ///
    /// 初始化时给每个任务装上 `TaskContext`：让它的 `ra` 指向汇编里的
    /// `__restore`、`sp` 指向自己的内核栈上保存的 `TrapContext`，这样第一次
    /// 切到该任务时，`__switch` 会跳到 `__restore`，进而进入用户态程序入口。
    pub static ref TASK_MANAGER: TaskManager = {
        let num_app = get_num_app();
        let mut tasks = [TaskControlBlock {
            task_cx: TaskContext::zero_init(),
            task_status: TaskStatus::UnInit,
        }; MAX_APP_NUM];
        for (i, task) in tasks.iter_mut().enumerate() {
            task.task_cx = TaskContext::goto_restore(init_app_cx(i));
            task.task_status = TaskStatus::Ready;
        }
        TaskManager {
            num_app,
            inner: unsafe {
                UPSafeCell::new(TaskManagerInner {
                    tasks,
                    current_task: 0,
                })
            },
        }
    };
}

impl TaskManager {
    /// 启动第一个任务。
    ///
    /// 第三章里任务表是编译期静态装好的，没有真正的 idle 进程，所以第一个
    /// 任务就是第一个用户程序。这里用一个“不存在的空上下文”作为起点，
    /// 让 `__switch` 从旧任务（实则是内核主流程）切进 0 号任务。
    fn run_first_task(&self) -> ! {
        let mut inner = self.inner.exclusive_access();
        let task0 = &mut inner.tasks[0];
        task0.task_status = TaskStatus::Running;
        let next_task_cx_ptr = &task0.task_cx as *const TaskContext;
        drop(inner); // 切栈前必须释放所有可变借用，避免借用检查在汇编切换时捣乱
        let mut _unused = TaskContext::zero_init();
        unsafe {
            __switch(&mut _unused as *mut TaskContext, next_task_cx_ptr);
        }
        panic!("unreachable in run_first_task!");
    }

    /// 把当前正在运行的任务标记为“就绪”。
    ///
    /// 这是任务主动让出 CPU（`sys_yield`）或时间片耗尽（时钟中断）时的前一半。
    fn set_current_ready(&self) {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].task_status = TaskStatus::Ready;
    }

    /// 把当前正在运行的任务标记为“已退出”。
    ///
    /// 这是 `sys_exit` 时的前一半：任务不再参与调度，直接从任务表中摘出。
    fn set_current_exited(&self) {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].task_status = TaskStatus::Exited;
    }

    /// 挑选下一个应该运行的任务，返回它的编号；没有就绪任务则返回 `None`。
    ///
    /// 采用简单的轮转：从“当前任务的后一个”开始绕一圈找第一个 `Ready` 的
    /// 任务。这样每个任务都能被公平地轮流照顾到。
    fn pick_next_ready(&self) -> Option<usize> {
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;
        (current + 1..current + self.num_app + 1)
            .map(|id| id % self.num_app)
            .find(|id| inner.tasks[*id].task_status == TaskStatus::Ready)
    }

    /// 真正执行调度：把当前任务让出去，切换到一个就绪任务。
    ///
    /// 如果没有任何就绪任务，说明所有程序都已跑完，直接 panic 结束（在
    /// 真实场景这里应该由内核自行收尾）。
    fn run_next_task(&self) {
        if let Some(next) = self.pick_next_ready() {
            let mut inner = self.inner.exclusive_access();
            let current = inner.current_task;
            // 更新状态：下一个任务变成 Running，当前任务维持现状（Ready/Exited）。
            inner.tasks[next].task_status = TaskStatus::Running;
            inner.current_task = next;
            // 拿到新旧两个上下文的指针后，必须先释放借用再切栈。
            let current_task_cx_ptr = &mut inner.tasks[current].task_cx as *mut TaskContext;
            let next_task_cx_ptr = &inner.tasks[next].task_cx as *const TaskContext;
            drop(inner);
            unsafe {
                __switch(current_task_cx_ptr, next_task_cx_ptr);
            }
        } else {
            panic!("All applications completed!");
        }
    }
}

/// 启动第一个任务（模块级包装）。
pub fn run_first_task() {
    TASK_MANAGER.run_first_task();
}

/// 把当前任务挂起并调度到下一个任务。
///
/// `sys_yield` 主动让出 CPU 和时钟中断被动抢走 CPU 都走这条路。
pub fn suspend_current_and_run_next() {
    TASK_MANAGER.set_current_ready();
    TASK_MANAGER.run_next_task();
}

/// 让当前任务退出并调度到下一个任务。
pub fn exit_current_and_run_next() {
    TASK_MANAGER.set_current_exited();
    TASK_MANAGER.run_next_task();
}

/// 记录当前任务又调用了一次系统调用 `id`。
///
/// 在每次系统调用分发前调用，所以 `trace(Syscall, id)` 查询到的那次计数
/// 已经把“正在问的这次”算进去了。
pub fn inc_current_syscall_count(id: usize) {
    if id >= MAX_SYSCALL_NUM {
        return;
    }
    let current = TASK_MANAGER.inner.exclusive_access().current_task;
    unsafe {
        SYSCALL_COUNTS[current][id] += 1;
    }
}

/// 查询当前任务调用系统调用 `id` 的次数。
pub fn current_syscall_count(id: usize) -> usize {
    if id >= MAX_SYSCALL_NUM {
        return 0;
    }
    let current = TASK_MANAGER.inner.exclusive_access().current_task;
    unsafe { SYSCALL_COUNTS[current][id] as usize }
}
