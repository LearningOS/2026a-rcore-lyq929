//! SBI 调用封装。
//!
//! SBI（Supervisor Binary Interface）是运行在 M 态（machine mode）的固件
//! 提供给 S 态内核的底层服务，例如设置定时器、往控制台写字符、关机等。
//! 内核想要使用这些服务，只需构造好寄存器参数后执行一条 `ecall` 指令。

use core::arch::asm;

/// SBI 扩展号：设置下一次定时器中断。
const SBI_SET_TIMER: usize = 0;
/// SBI 扩展号：向控制台输出一个字符。
const SBI_CONSOLE_PUTCHAR: usize = 1;
/// SBI 扩展号：系统复位（关机属于复位的一种）。
const SBI_SYSTEM_RESET: usize = 0x5352_5354;

/// 通用的 SBI 调用入口。
///
/// 约定：功能号放在 `x17`（a7），`x16`（a6）置 0，参数依次放在
/// `x10/x11/x12`，返回值从 `x10`（a0）取回。
#[inline(always)]
fn sbi_call(which: usize, arg0: usize, arg1: usize, arg2: usize) -> usize {
    let mut ret;
    unsafe {
        asm!(
            "ecall",
            inlateout("x10") arg0 => ret,
            in("x11") arg1,
            in("x12") arg2,
            in("x16") 0,
            in("x17") which,
        );
    }
    ret
}

/// 让固件在 `timer` 时刻之后触发一次时钟中断。
pub fn set_timer(timer: usize) {
    sbi_call(SBI_SET_TIMER, timer, 0, 0);
}

/// 通过固件往主控台（QEMU 的 UART）写一个字符。
pub fn console_putchar(c: usize) {
    sbi_call(SBI_CONSOLE_PUTCHAR, c, 0, 0);
}

/// 请求固件关闭系统。
///
/// 复位类型 0 表示关机，复位原因 0 表示“正常结束”。如果固件没执行成功
/// （理论上不会），就 panic。
pub fn shutdown() -> ! {
    unsafe {
        asm!(
            "ecall",
            in("a7") SBI_SYSTEM_RESET,
            in("a6") 0,
            inlateout("a0") 0usize => _,
            inlateout("a1") 0usize => _,
        );
    }
    panic!("It should shutdown!");
}
