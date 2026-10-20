//! Rust 侧对 `__switch` 的封装。
//!
//! 任务切换的“真功夫”在汇编里（为什么不能写成 Rust？因为切换必须原样保存
//! 和恢复寄存器，Rust 编译器不会保证我们控制到这种程度），这里只负责把
//! `switch.S` 里的符号暴露给上层 Rust 代码调用。

use super::TaskContext;
use core::arch::global_asm;

global_asm!(include_str!("switch.S"));

extern "C" {
    /// 保存 `current_task_cx_ptr` 所指的旧上下文，并装载 `next_task_cx_ptr`
    /// 所指的新上下文，从而完成一次任务切换。
    pub fn __switch(current_task_cx_ptr: *mut TaskContext, next_task_cx_ptr: *const TaskContext);
}
