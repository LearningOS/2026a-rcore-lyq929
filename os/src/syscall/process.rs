//! process related syscalls
use crate::task::{get_syscall_cnt, TASK_MANAGER};
use crate::timer::get_time_us;
use super::TimeVal;

pub const SYS_WRITE: usize = 64;
pub const SYS_GETTIMEOFDAY: usize = 169;
pub const SYS_YIELD: usize = 124;
pub const SYS_EXIT: usize = 93;
pub const SYS_TRACE: usize = 410;

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

pub fn sys_trace(req: usize, arg1: usize, arg2: usize) -> isize {
    trace!("kernel: sys_trace req={}, arg1={}, arg2={}", req, arg1, arg2);
    match req {
        TRACE_SYSCALL => {
            // arg1 = app_id(任务ID), arg2 = syscall编号，顺序千万不能颠倒
            if arg1 >= crate::config::MAX_APP_NUM || arg2 >= crate::config::SYS_CALL_COUNT {
                return -1;
            }
            get_syscall_cnt(arg1, arg2) as isize
        }
        TRACE_READ | TRACE_WRITE => -1,
        _ => -1,
    }
}
