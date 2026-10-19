//! Process management syscalls
use crate::{
    task::{exit_current_and_run_next, suspend_current_and_run_next},
    timer::get_time_us,
};
use spin::Mutex;

use super::{SYSCALL_EXIT, SYSCALL_GET_TIME, SYSCALL_TRACE, SYSCALL_YIELD};

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// 使用spin Mutex，static全局可安全共享
pub static SYS_CALL_COUNT: Mutex<[usize; 512]> = Mutex::new([0; 512]);

/// 从用户虚拟地址读1字节，返回None表示失败
fn read_user_byte(va: usize) -> Option<u8> {
    let mut buf = 0u8;
    // 课程框架提供：从用户虚拟地址拷贝1字节到内核buf
    if crate::mm::copy_one_byte_from_user(&mut buf, va) {
        Some(buf)
    } else {
        None
    }
}

/// 写1字节到用户虚拟地址
fn write_user_byte(va: usize, byte: u8) -> bool {
    crate::mm::copy_one_byte_to_user(byte, va)
}

/// task exits and submit an exit code
pub fn sys_exit(exit_code: i32) -> ! {
    trace!("[kernel] Application exited with code {}", exit_code);
    SYS_CALL_COUNT.lock()[SYSCALL_EXIT] += 1;
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    SYS_CALL_COUNT.lock()[SYSCALL_YIELD] += 1;
    suspend_current_and_run_next();
    0
}

/// get time with second and microsecond
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    SYS_CALL_COUNT.lock()[SYSCALL_GET_TIME] += 1;
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
    SYS_CALL_COUNT.lock()[SYSCALL_TRACE] += 1;

    match trace_request {
        // TraceRequest::Read =0
        0 => {
            match read_user_byte(id) {
                Some(b) => b as isize,
                None => -1,
            }
        }
        // TraceRequest::Write =1
        1 => {
            let val = (data & 0xff) as u8;
            if write_user_byte(id, val) {
                0
            } else {
                -1
            }
        }
        // TraceRequest::Syscall =2 查询系统调用计数
        2 => {
            let cnt = SYS_CALL_COUNT.lock()[id];
            cnt as isize
        }
        _ => -1,
    }
}
