//! 进程（任务）管理相关的系统调用：退出、让出 CPU、取时间、trace。

use crate::{
    config::MEMORY_END,
    task::{current_syscall_count, exit_current_and_run_next, suspend_current_and_run_next},
    timer::get_time_us,
};

/// `trace` 请求类型：从用户内存读一个字节。
const TRACE_READ: usize = 0;
/// `trace` 请求类型：往用户内存写一个字节。
const TRACE_WRITE: usize = 1;
/// `trace` 请求类型：查询某个系统调用被调用了多少次。
const TRACE_SYSCALL: usize = 2;

/// 内核可以直接访问的物理内存的起始地址。
const MEMORY_START: usize = 0x8000_0000;

/// 判断 `addr` 是否落在 RAM 内，即内核能否安全地解引用它。
///
/// 第三章没有分页、所有应用共享同一地址空间，整块物理内存都直接可访问，
/// 因此唯一需要拒绝的地址就是超出 RAM 范围的那些。
fn is_valid_user_addr(addr: usize) -> bool {
    (MEMORY_START..MEMORY_END).contains(&addr)
}

/// 秒/微秒结构体，`gettime` 用它把时间写回用户内存。
#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// 让当前任务退出，并附上退出码。
///
/// 退出后立刻把控制权交给下一个任务；理论上是不可达函数，若真的继续往下
/// 走说明调度出了问题。
pub fn do_exit(exit_code: i32) -> ! {
    trace!("[kernel] Application exited with code {}", exit_code);
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// 当前任务主动让出 CPU，供其它任务运行。
pub fn do_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// 取当前时刻，以秒 + 微秒的形式写进 `ts` 指向的用户结构体。
pub fn do_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let us = get_time_us();
    unsafe {
        *ts = TimeVal {
            sec: us / 1_000_000,
            usec: us % 1_000_000,
        };
    }
    0
}

/// 实现 `trace`：对当前任务进行三种观测之一。
///
/// - `Read`：读用户地址 `id` 处的 1 字节并返回，失败返回 -1；
/// - `Write`：把 `data` 的最低字节写到用户地址 `id`，成功返回 0、失败 -1；
/// - `Syscall`：返回当前任务调用系统调用 `id` 的次数。
///
/// 因为计数器在每次系统调用入口就更新，这里查到的次数已包含当前这次
/// `trace` 自身。
pub fn do_trace(trace_request: usize, id: usize, data: usize) -> isize {
    match trace_request {
        TRACE_READ => {
            if !is_valid_user_addr(id) {
                return -1;
            }
            unsafe { core::ptr::read_volatile(id as *const u8) as isize }
        }
        TRACE_WRITE => {
            if !is_valid_user_addr(id) {
                return -1;
            }
            unsafe {
                core::ptr::write_volatile(id as *mut u8, data as u8);
            }
            0
        }
        TRACE_SYSCALL => current_syscall_count(id) as isize,
        _ => -1,
    }
}
