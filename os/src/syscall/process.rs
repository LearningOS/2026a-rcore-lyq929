//! Process management syscalls
use crate::{
    task::{exit_current_and_run_next, suspend_current_and_run_next},
    timer::get_time_us,
    mm::{copy_from_user, copy_to_user, UserPtr},
};
use core::cell::RefCell;

use super::{SYSCALL_EXIT, SYSCALL_GET_TIME, SYSCALL_TRACE, SYSCALL_YIELD, SYSCALL_WRITE};

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// 全局系统调用计数，供 sys_trace / count_syscall 使用
pub static SYS_CALL_COUNT: RefCell<[usize; 512]> = RefCell::new([0; 512]);

/// task exits and submit an exit code
pub fn sys_exit(exit_code: i32) -> ! {
    trace!("[kernel] Application exited with code {}", exit_code);
    SYS_CALL_COUNT.borrow_mut()[SYSCALL_EXIT] += 1;
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    SYS_CALL_COUNT.borrow_mut()[SYSCALL_YIELD] += 1;
    suspend_current_and_run_next();
    0
}

/// get time with second and microsecond
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    SYS_CALL_COUNT.borrow_mut()[SYSCALL_GET_TIME] += 1;
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
/// trace_request（来自用户 TraceRequest枚举）
/// 0 → Read：id=user_va，读取1字节
/// 1 → Write：id=user_va，data=u8字节
/// 2 → Syscall：id=syscall_id，返回该syscall调用次数
pub fn sys_trace(trace_request: usize, id: usize, data: usize) -> isize {
    trace!("kernel: sys_trace req={},id={},data={}", trace_request, id, data);
    SYS_CALL_COUNT.borrow_mut()[SYSCALL_TRACE] += 1;

    match trace_request {
        // TraceRequest::Read = 0 读用户地址一字节
        0 => {
            let user_ptr = UserPtr(id as *const u8);
            let mut byte = 0u8;
            if copy_from_user(&mut byte, user_ptr, 1) == 0 {
                byte as isize
            } else {
                -1
            }
        }
        // TraceRequest::Write =1 写用户地址一字节
        1 => {
            let val = (data & 0xff) as u8;
            let user_ptr = UserPtr(id as *mut u8);
            if copy_to_user(user_ptr, &val, 1) == 0 {
                0
            } else {
                -1
            }
        }
        // TraceRequest::Syscall =2 查询系统调用计数
        2 => {
            let cnt = SYS_CALL_COUNT.borrow()[id];
            cnt as isize
        }
        _ => -1,
    }
}
