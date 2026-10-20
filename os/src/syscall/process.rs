//! process related syscalls
use crate::task::{get_syscall_cnt, TASK_MANAGER};
use crate::timer::get_time_us;
use super::TimeVal;

pub const SYS_WRITE: usize = 64;
pub const SYS_GETTIMEOFDAY: usize = 169;
pub const SYS_YIELD: usize = 124;
pub const SYS_EXIT: usize = 93;
pub const SYS_TRACE: usize = 410;
pub const SYS_GETPID: usize = 172; // 增加getpid系统调用号

const TRACE_READ: usize = 0;
const TRACE_WRITE: usize = 1;
const TRACE_SYSCALL: usize = 2;

pub fn sys_exit(x_exit_code: i32) -> isize {
    trace!("kernel: sys_exit {}", x_exit_code);
    TASK_MANAGER.exit_current_and_run_next();
    0
}

pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    TASK_MANAGER.suspend_current_and_run_next();
    0
}

pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let us = get_time_us();
    unsafe {
        *ts = TimeVal {
            tv_sec: us / 1_000_000,
            tv_usec: us % 1_000_000,
        };
    }
    0
}

/// 返回当前任务app_id（数组下标）
pub fn sys_getpid() -> isize {
    trace!("kernel: sys_getpid");
    let app_id = TASK_MANAGER.get_current_task_id();
    app_id as isize
}

pub fn sys_trace(req: usize, arg1: usize, arg2: usize) -> isize {
    trace!("kernel: sys_trace req={}, arg1={}, arg2={}", req, arg1, arg2);
    match req {
        TRACE_SYSCALL => {
            // arg1 = app_id(来自用户getpid返回值，是任务数组下标), arg2 = syscall id
            if arg1 >= crate::config::MAX_APP_NUM || arg2 >= crate::config::SYS_CALL_COUNT {
                return -1;
            }
            get_syscall_cnt(arg1, arg2) as isize
        }
        TRACE_READ | TRACE_WRITE => -1,
        _ => -1,
    }
}
