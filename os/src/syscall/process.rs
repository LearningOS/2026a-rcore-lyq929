//! process related syscalls
use crate::task::{get_syscall_cnt, TASK_MANAGER};
use crate::timer::get_time_us;
use super::TimeVal;

/// syscall id: write
pub const SYS_WRITE: usize = 64;
/// syscall id: get time of day
pub const SYS_GETTIMEOFDAY: usize = 169;
/// syscall id: yield
pub const SYS_YIELD: usize = 124;
/// syscall id: exit
pub const SYS_EXIT: usize = 93;
/// syscall id: trace
pub const SYS_TRACE: usize = 410;

/// TraceRequest definition (match user lib.rs)
/// Read = 0, Write = 1, Syscall = 2
const TRACE_READ: usize = 0;
const TRACE_WRITE: usize = 1;
const TRACE_SYSCALL: usize = 2;

/// sys_exit: terminate current task
pub fn sys_exit(x_exit_code: i32) -> isize {
    trace!("kernel: sys_exit {}", x_exit_code);
    TASK_MANAGER.exit_current_and_run_next();
    0
}

/// sys_yield: give up CPU and switch to next task
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    TASK_MANAGER.suspend_current_and_run_next();
    0
}

/// sys_get_time: get wall time
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

/// sys_trace: 3 args, match user lib.rs
/// req: TraceRequest, arg1, arg2
/// TRACE_SYSCALL: req=2, arg1 = task_id(app_id), arg2 = syscall_id
pub fn sys_trace(req: usize, arg1: usize, arg2: usize) -> isize {
    trace!("kernel: sys_trace req={}, arg1={}, arg2={}", req, arg1, arg2);
    match req {
        TRACE_SYSCALL => {
            // arg1 = app_id，arg2 = syscall_id，顺序不能写反！
            if arg1 >= crate::config::MAX_APP_NUM || arg2 >= crate::config::SYS_CALL_COUNT {
                return -1;
            }
            get_syscall_cnt(arg1, arg2) as isize
        }
        TRACE_READ | TRACE_WRITE => -1,
        _ => -1,
    }
}
