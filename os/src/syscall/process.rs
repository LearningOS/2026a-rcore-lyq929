//! Process management syscalls
use crate::{
    task::{
        exit_current_and_run_next,
        suspend_current_and_run_next,
    },
    timer::get_time_us,
};

use super::{SYSCALL_EXIT, SYSCALL_GET_TIME, SYSCALL_TRACE, SYSCALL_YIELD, SYSCALL_WRITE};

// 单核心实验全局计数
static mut SYS_CALL_COUNT: [usize; 512] = [0; 512];

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// sys_write计数，供fs.rs调用
pub fn inc_syscall_write() {
    unsafe {
        SYS_CALL_COUNT[SYSCALL_WRITE] += 1;
    }
}

/// task exits and submit an exit code
pub fn sys_exit(exit_code: i32) -> ! {
    trace!("[kernel] Application exited with code {}", exit_code);
    unsafe { SYS_CALL_COUNT[SYSCALL_EXIT] += 1; }
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    unsafe { SYS_CALL_COUNT[SYSCALL_YIELD] += 1; }
    suspend_current_and_run_next();
    0
}

/// get time with second and microsecond
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    unsafe { SYS_CALL_COUNT[SYSCALL_GET_TIME] += 1; }
    let us = get_time_us();
    unsafe {
        *ts = TimeVal {
            sec: us / 1_000_000,
            usec: us % 1_000_000,
        };
    }
    0
}

/// sys_trace syscall id=410
/// trace_request：
/// 0 → Read：id=user_va，暂不实现返回‑1
/// 1 → Write：id=user_va，data=u8字节，暂不实现返回‑1
/// 2 → Syscall：id=syscall_id，返回该syscall调用次数
pub fn sys_trace(trace_request: usize, id: usize, data: usize) -> isize {
    trace!("kernel: sys_trace req={},id={},data={}", trace_request, id, data);
    unsafe { SYS_CALL_COUNT[SYSCALL_TRACE] += 1; }

    match trace_request {
        // TraceRequest::Read
        0 => -1,
        // TraceRequest::Write
        1 => -1,
        // TraceRequest::Syscall 查询系统调用计数
        2 => {
            let cnt = unsafe { SYS_CALL_COUNT[id] };
            cnt as isize
        }
        _ => -1,
    }
}
