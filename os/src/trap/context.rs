//! [`TrapContext`]：进出用户态时的寄存器现场。

use riscv::register::sstatus::{self, Sstatus, SPP};

/// 陷阱上下文：保存 `sstatus`、`sepc` 以及通用寄存器 `x0~x31`。
///
/// 布局必须与 `trap.S` 里的存取偏移严格一致，所以用 `#[repr(C)]` 固定字段
/// 顺序，避免编译器重排。
#[repr(C)]
#[derive(Debug)]
pub struct TrapContext {
    /// 通用寄存器 x0~x31（x0 恒为 0，仅占位）。
    pub x: [usize; 32],
    /// 状态寄存器（含特权级等信息）。
    pub sstatus: Sstatus,
    /// 异常返回程序计数器。
    pub sepc: usize,
}

impl TrapContext {
    /// 把 `x[2]`（即 sp）设成 `sp`。
    pub fn set_sp(&mut self, sp: usize) {
        self.x[2] = sp;
    }

    /// 构造一个“用户程序首次进入”时的初始上下文。
    ///
    /// 核心是两步：把 `sstatus` 的 SPP 置为用户态（这样 `sret` 会回到 U 态），
    /// 并把 `sepc` 指向程序入口、`sp` 指向用户栈顶。
    pub fn app_init_context(entry: usize, sp: usize) -> Self {
        // 读当前 sstatus，改掉 SPP 字段：表示“被恢复后处于用户模式”。
        let mut sstatus = sstatus::read();
        sstatus.set_spp(SPP::User);
        let mut cx = Self {
            x: [0; 32],
            sstatus,
            sepc: entry, // 程序的入口地址
        };
        cx.set_sp(sp); // 用户栈指针
        cx // 返回这份初始现场
    }
}
