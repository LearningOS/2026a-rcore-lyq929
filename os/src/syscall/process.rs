//! process related syscalls
use crate::task;
use crate::task::get_current_syscall_count;
use crate::timer::get_time_us;
use super::TimeVal;

pub fn sys_exit(x_exit_code: i32) -> isize {
    trace!("kernel: sys_exit {}", x_exit_code);
    task::exit_current_and_run_next();
    0
}

pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    task::suspend_current_and_run_next();
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

// SYS_TRACE 410
// arg0: cmd 0=read counter,1=write counter; arg1: value; arg2 unused
pub fn sys_trace(cmd: usize, value: usize, _unused: usize) -> isize {
    trace!("kernel: sys_trace cmd={},val={}", cmd, value);
    match cmd {
        0 => {
            // read: return current syscall count
            get_current_syscall_count() as isize
        }
        1 => {
            // write: set current task syscall count
            let cnt = task::TASK_MANAGER.get_current_syscall_count_mut();
            *cnt = value;
            0
        }
        _ => -1,
    }
}
