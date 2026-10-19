//! Process management syscalls
use crate::{
    task::{
        exit_current_and_run_next,
        suspend_current_and_run_next,
    },
    timer::get_time_us,
};

use super::{SYSCALL_EXIT, SYSCALL_GET_TIME, SYSCALL_TRACE, SYSCALL_YIELD, SYSCALL_WRITE};

// ch3全局系统调用计数，volatile读写防止编译器优化消除写操作
static mut SYS_CALL_COUNT: [usize; 512] = [0; 512];

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

fn get_count(id: usize) -> usize {
    unsafe { core::ptr::read_volatile(&SYS_CALL_COUNT[id]) }
}

fn inc_count(id: usize) {
    unsafe {
        let val = core::ptr::read_volatile(&SYS_CALL_COUNT[id]);
        core::ptr::write_volatile(&mut SYS_CALL_COUNT[id], val + 1);
    }
}

/// sys_write计数，供fs模块调用
pub fn inc_syscall_write() {
    inc_count(SYSCALL_WRITE);
}

/// ch3恒等映射：简单判断地址属于用户可用内存范围 0x80400000 ~ 0x88000000
fn is_user_memory(va: usize) -> bool {
    const USER_MEM_START: usize = 0x8040_0000;
    const USER_MEM_END: usize = 0x8800_0000;
    va >= USER_MEM_START && va < USER_MEM_END
}

/// trace_read：ch3恒等映射，va直接当作物理地址，读1字节
fn read_user_byte(va: usize) -> Option<u8> {
    if !is_user_memory(va) {
        return None;
    }
    unsafe {
        Some(core::ptr::read_volatile(va as *const u8))
    }
}

/// trace_write：ch3恒等映射，va直接当作物理地址，写1字节
fn write_user_byte(va: usize, byte: u8) -> bool {
    if !is_user_memory(va) {
        return false;
    }
    unsafe {
        core::ptr::write_volatile(va as *mut u8, byte);
        true
    }
}

pub fn sys_exit(exit_code: i32) -> ! {
    trace!("[kernel] Application exited with code {}", exit_code);
    inc_count(SYSCALL_EXIT);
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    inc_count(SYSCALL_YIELD);
    suspend_current_and_run_next();
    0
}

pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    inc_count(SYSCALL_GET_TIME);
    let us = get_time_us();
    unsafe {
        *ts = TimeVal {
            sec: us / 1_000_000,
            usec: us % 1_000_000,
        };
    }
    0
}

/// sys_trace syscall_id=410
/// trace_request:
/// 0 Read, 1 Write, 2 Syscall count
pub fn sys_trace(trace_request: usize, id: usize, data: usize) -> isize {
    trace!("kernel: sys_trace req={},id={},data={}", trace_request, id, data);
    inc_count(SYSCALL_TRACE);

    match trace_request {
        // TraceRequest::Read
        0 => match read_user_byte(id) {
            Some(b) => b as isize,
            None => -1,
        },
        // TraceRequest::Write
        1 => {
            let val = (data & 0xff) as u8;
            if write_user_byte(id, val) {
                0
            } else {
                -1
            }
        }
        // TraceRequest::Syscall 查询计数
        2 => get_count(id) as isize,
        _ => -1,
    }
}
