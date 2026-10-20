//! 陷阱（trap）处理：所有从用户态进入内核态的入口。
//!
//! 无论异常、中断还是 `ecall` 系统调用，硬件都会把 CPU 切进 S 态并跳转到
//! `stvec` 指向的入口。rCore 只有一个入口点 `__alltraps`（定义在 `trap.S`）。
//! 汇编部分负责保存现场、切换到内核栈，然后调用 `trap_handler`；Rust 侧
//! 则依据 trap 原因做具体分发（时钟中断 → 抢占，系统调用 → 转发给
//! `syscall`，非法访存等 → 杀掉肇事任务）。

mod context;

use crate::syscall::syscall;
use crate::task::{exit_current_and_run_next, suspend_current_and_run_next};
use crate::timer::set_next_trigger;
use core::arch::global_asm;
use riscv::register::{
    mtvec::TrapMode,
    scause::{self, Exception, Interrupt, Trap},
    sie, stval, stvec,
};

global_asm!(include_str!("trap.S"));

/// 初始化陷阱处理：把 `stvec` 指向 `__alltraps` 入口。
pub fn init() {
    extern "C" {
        fn __alltraps();
    }
    unsafe {
        stvec::write(__alltraps as usize, TrapMode::Direct);
    }
}

/// 打开 S 态时钟中断（在 SIE 寄存器里置 STIE 位）。
pub fn enable_timer_interrupt() {
    unsafe {
        sie::set_stimer();
    }
}

/// 统一的 trap 处理函数，返回处理后的 `TrapContext`。
///
/// `cx` 里保存着当前任务被切进内核前的完整现场。我们可能修改它（例如让
/// `sepc` 跳过 `ecall`、把返回值写进 `a0`），改完之后返回同一个 `cx`，
/// 汇编侧会用它把现场还原回用户态。
#[no_mangle]
pub fn trap_handler(cx: &mut TrapContext) -> &mut TrapContext {
    let scause = scause::read(); // 引发 trap 的原因
    let stval = stval::read();   // 附加信息（如出错地址）
    match scause.cause() {
        // 用户环境调用（ecall 系统调用）
        Trap::Exception(Exception::UserEnvCall) => {
            // 让返回地址跳过本条 ecall，否则会无限触发同一个系统调用。
            cx.sepc += 4;
            // 参数：a7=系统调用号，a0/a1/a2=三个参数；返回值写回 a0。
            cx.x[10] = syscall(cx.x[17], [cx.x[10], cx.x[11], cx.x[12]]) as usize;
        }
        // 用户程序访存越界 / 缺页（第三章没有分页，出现即视为非法访存）
        Trap::Exception(Exception::StoreFault) | Trap::Exception(Exception::StorePageFault) => {
            println!(
                "[kernel] PageFault in application, bad addr = {:#x}, bad instruction = {:#x}, kernel killed it.",
                stval, cx.sepc
            );
            exit_current_and_run_next();
        }
        // 非法指令
        Trap::Exception(Exception::IllegalInstruction) => {
            println!("[kernel] IllegalInstruction in application, kernel killed it.");
            exit_current_and_run_next();
        }
        // S 态时钟中断：时间片到，重设下一次触发并挂起当前任务
        Trap::Interrupt(Interrupt::SupervisorTimer) => {
            set_next_trigger();
            suspend_current_and_run_next();
        }
        _ => {
            panic!(
                "Unsupported trap {:?}, stval = {:#x}!",
                scause.cause(),
                stval
            );
        }
    }
    cx
}

pub use context::TrapContext;
