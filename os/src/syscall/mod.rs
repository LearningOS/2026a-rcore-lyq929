//! syscall module
mod fs;
mod process;
use fs::*;
use process::*;

/// Time value for sys_get_time
#[derive(Debug, Clone, Copy)]
pub struct TimeVal {
    /// seconds
    pub tv_sec: usize,
    /// microseconds
    pub tv_usec: usize,
}

/// system call entry
pub fn syscall(sys_id: usize, args: [usize; 3]) -> isize {
    use crate::task::{inc_syscall_cnt, TASK_MANAGER};
    // 【关键修复】所有系统调用先计数，放在match外面
    let app_id = TASK_MANAGER.get_current_task_id();
    inc_syscall_cnt(app_id, sys_id);

    match sys_id {
        SYS_WRITE => sys_write(args[0], args[1] as *const u8, args[2]),
        SYS_GETTIMEOFDAY => sys_get_time(args[0] as *mut TimeVal, args[1]),
        SYS_YIELD => sys_yield(),
        SYS_EXIT => sys_exit(args[0] as i32),
        SYS_TRACE => sys_trace(args[0], args[1], args[2]),
        _ => -1,
    }
}
