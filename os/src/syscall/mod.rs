//! 系统调用的实现与分发。
//!
//! 用户态通过 `ecall` 发起系统调用，硬件把它变成一个“来自 U 态的环境调用”
//! 异常，最终由 [`crate::trap::trap_handler`] 交到这里。`syscall()` 是唯一
//! 入口，按系统调用号把请求分发到对应的具体实现。每个具体实现写成独立的
//! `do_xxx` 函数，方便阅读和后续扩展。

/// write 系统调用号。
const SYSCALL_WRITE: usize = 64;
/// exit 系统调用号。
const SYSCALL_EXIT: usize = 93;
/// yield 系统调用号。
const SYSCALL_YIELD: usize = 124;
/// gettime 系统调用号。
const SYSCALL_GET_TIME: usize = 169;
/// trace 系统调用号。
const SYSCALL_TRACE: usize = 410;

mod fs;
mod process;

use fs::*;
use process::*;

/// 处理一次系统调用：`syscall_id` 是系统调用号，`args` 是三个参数。
pub fn syscall(syscall_id: usize, args: [usize; 3]) -> isize {
    // 分发前先记账，保证 trace(Syscall, id) 查询到的计数已经包含当前这次调用。
    crate::task::inc_current_syscall_count(syscall_id);
    match syscall_id {
        SYSCALL_WRITE => write_to_stdout(args[0], args[1] as *const u8, args[2]),
        SYSCALL_EXIT => do_exit(args[0] as i32),
        SYSCALL_YIELD => do_yield(),
        SYSCALL_GET_TIME => do_get_time(args[0] as *mut TimeVal, args[1]),
        SYSCALL_TRACE => do_trace(args[0], args[1], args[2]),
        _ => panic!("Unsupported syscall_id: {}", syscall_id),
    }
}
