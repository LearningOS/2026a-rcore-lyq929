//! Process management syscalls
use crate::{
    sync::UPSafeCell,
    task::{exit_current_and_run_next, suspend_current_and_run_next},
    timer::get_time_us,
};
use lazy_static::*;

/// syscall id 的上界，用于给调用次数表定大小
/// ch3 只用到 write(64)/exit(93)/yield(124)/get_time(169)/trace(410)，取 512 足够
const SYSCALL_ID_MAX: usize = 512;

lazy_static! {
    /// 记录每个 syscall id 被调用的累计次数（sys_trace 本身也算一次）
    static ref SYSCALL_COUNT: UPSafeCell<[usize; SYSCALL_ID_MAX]> = unsafe {
        UPSafeCell::new([0usize; SYSCALL_ID_MAX])
    };
}

/// 每次进入任意 syscall 时调用，把该 syscall id 的计数加一
pub fn record_syscall(id: usize) {
    if id < SYSCALL_ID_MAX {
        SYSCALL_COUNT.exclusive_access()[id] += 1;
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(exit_code: i32) -> ! {
    trace!("[kernel] Application exited with code {}", exit_code);
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// get time with second and microsecond
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let us = get_time_us();
    unsafe {
        *ts = TimeVal {
            sec: us / 1_000_000,
            usec: us % 1_000_000,
        };
    }
    0
}

/// 实现 sys_trace 系统调用（TraceRequest: Read=0, Write=1, Syscall=2）
///
/// - Read : 读出地址 `id` 处 1 个字节并返回其值
/// - Write: 把字节 `data` 写入地址 `id`
/// - Syscall: 返回 syscall id 为 `id` 的调用累计次数
///
/// ch3 还没有地址空间/页表，用户程序直接加载进内核空间运行，
/// 传入的地址就是真实物理地址，因此可以直接用裸指针读写，无需权限检查。
pub fn sys_trace(trace_request: usize, id: usize, data: usize) -> isize {
    trace!("kernel: sys_trace");
    match trace_request {
        // Read：读地址 id 处的字节
        0 => unsafe { core::ptr::read_volatile(id as *const u8) as isize },
        // Write：把 data 的低 8 位写入地址 id
        1 => {
            unsafe {
                core::ptr::write_volatile(id as *mut u8, data as u8);
            }
            0
        }
        // Syscall：返回该 syscall id 的累计调用次数
        2 => {
            if id < SYSCALL_ID_MAX {
                SYSCALL_COUNT.exclusive_access()[id] as isize
            } else {
                0
            }
        }
        _ => -1,
    }
}
