//! process related syscalls
use crate::task::{read_current_syscall_count, write_current_syscall_count};
use crate::task;
use crate::timer::get_time_us;
use super::TimeVal;

pub fn inc_syscall_write() {
    task::inc_current_syscall_count();
}

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
pub fn sys_trace(cmd: usize, value: usize, _unused: usize) -> isize {
    trace!("kernel: sys_trace cmd={},val={}", cmd, value);
    match cmd {
        0 => {
            // read counter
            read_current_syscall_count() as isize
        }
        1 => {
            // set counter
            write_current_syscall_count(value);
            0
        }
        _ => -1,
    }
}
