//! Process management syscalls
use crate::{
    task::{
        exit_current_and_run_next,
        suspend_current_and_run_next,
        current_user_token,
        translated_byte_buffer,
    },
    timer::get_time_us,
};

use super::{SYSCALL_EXIT, SYSCALL_GET_TIME, SYSCALL_TRACE, SYSCALL_YIELD};

// 单核心裸机，ch3实验全局系统调用计数
static mut SYS_CALL_COUNT: [usize; 512] = [0; 512];

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// 从用户虚拟地址读1字节
fn read_user_byte(va: usize) -> Option<u8> {
    let bufs = translated_byte_buffer(current_user_token(), va, 1)?;
    if bufs.is_empty() {
        return None;
    }
    Some(bufs[0][0])
}

/// 写1字节到用户虚拟地址
fn write_user_byte(va: usize, byte: u8) -> bool {
    let Some(bufs) = translated_byte_buffer(current_user_token(), va, 1) else {
        return false;
    };
    if bufs.is_empty() {
        return false;
    }
    bufs[0][0] = byte;
    true
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
/// 0 → Read：id=user_va，读取1字节
/// 1 → Write：id=user_va，data=u8字节
/// 2 → Syscall：id=syscall_id，返回该syscall调用次数
pub fn sys_trace(trace_request: usize, id: usize, data: usize) -> isize {
    trace!("kernel: sys_trace req={},id={},data={}", trace_request, id, data);
    unsafe { SYS_CALL_COUNT[SYSCALL_TRACE] += 1; }

    match trace_request {
        0 => {
            match read_user_byte(id) {
                Some(b) => b as isize,
                None => -1,
            }
        }
        1 => {
            let val = (data & 0xff) as u8;
            if write_user_byte(id, val) {
                0
            } else {
                -1
            }
        }
        2 => {
            let cnt = unsafe { SYS_CALL_COUNT[id] };
            cnt as isize
        }
        _ => -1,
    }
}
