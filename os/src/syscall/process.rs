//! Process management syscalls
use crate::{
    task::{
        exit_current_and_run_next,
        suspend_current_and_run_next,
        TASK_MANAGER,
    },
    timer::get_time_us,
};

use super::{SYSCALL_EXIT, SYSCALL_GET_TIME, SYSCALL_TRACE, SYSCALL_YIELD, SYSCALL_WRITE};

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// 获取当前正在运行任务的可变引用
fn current_task_mut() -> &'static mut crate::task::TaskControlBlock {
    let mut inner = TASK_MANAGER.inner.exclusive_access();
    let idx = inner.current_task;
    &mut inner.tasks[idx]
}

/// ch3恒等映射 用户内存范围
fn is_user_memory(va: usize) -> bool {
    const USER_MEM_START: usize = 0x8040_0000;
    const USER_MEM_END: usize = 0x8800_0000;
    va >= USER_MEM_START && va < USER_MEM_END
}

/// trace_read：读用户虚拟地址一字节(ch3恒等映射)
fn read_user_byte(va: usize) -> Option<u8> {
    if !is_user_memory(va) {
        return None;
    }
    unsafe {
        Some(core::ptr::read_volatile(va as *const u8))
    }
}

/// trace_write：写用户虚拟地址一字节(ch3恒等映射)
fn write_user_byte(va: usize, byte: u8) -> bool {
    if !is_user_memory(va) {
        return false;
    }
    unsafe {
        core::ptr::write_volatile(va as *mut u8, byte);
        true
    }
}

/// sys_write计数回调，供fs模块调用
pub fn inc_syscall_write() {
    let t = current_task_mut();
    t.cnt_write += 1;
}

pub fn sys_exit(exit_code: i32) -> ! {
    trace!("[kernel] Application exited with code {}", exit_code);
    let t = current_task_mut();
    t.cnt_exit += 1;
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    let t = current_task_mut();
    t.cnt_yield += 1;
    suspend_current_and_run_next();
    0
}

pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let t = current_task_mut();
    t.cnt_gettime += 1;
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
/// trace_request: 0=Read,1=Write,2=Syscall count
pub fn sys_trace(trace_request: usize, id: usize, data: usize) -> isize {
    trace!("kernel: sys_trace req={},id={},data={}", trace_request, id, data);
    let t = current_task_mut();
    t.cnt_trace += 1;

    match trace_request {
        0 => match read_user_byte(id) {
            Some(b) => b as isize,
            None => -1,
        },
        1 => {
            let val = (data & 0xff) as u8;
            if write_user_byte(id, val) {
                0
            } else {
                -1
            }
        }
        2 => {
            match id {
                SYSCALL_WRITE => t.cnt_write as isize,
                SYSCALL_EXIT => t.cnt_exit as isize,
                SYSCALL_YIELD => t.cnt_yield as isize,
                SYSCALL_GET_TIME => t.cnt_gettime as isize,
                SYSCALL_TRACE => t.cnt_trace as isize,
                _ => 0,
            }
        }
        _ => -1,
    }
}
