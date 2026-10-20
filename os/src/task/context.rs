//! [`TaskContext`] 的实现。
//!
//! 它保存的是“任务切换点”处的寄存器现场：`ra`、`sp` 以及被调用者保存的
//! `s0~s11`。`__switch` 汇编就是围绕这个布局来回存取的。

/// 任务上下文结构：保存一组切换所需的寄存器。
#[derive(Copy, Clone)]
#[repr(C)]
pub struct TaskContext {
    /// 切换点之后的返回地址（决定切回来之后从哪继续执行）。
    ra: usize,
    /// 栈指针。
    sp: usize,
    /// 被调用者保存寄存器 s0~s11。
    s: [usize; 12],
}

impl TaskContext {
    /// 构造一个“全空”的上下文，寄存器全 0。
    ///
    /// 主要用来给“旧任务”占位——第一次切换时我们并没有真正的旧现场要保存。
    pub fn zero_init() -> Self {
        Self {
            ra: 0,
            sp: 0,
            s: [0; 12],
        }
    }

    /// 构造一个“将要首次进入用户态”的上下文。
    ///
    /// 把 `ra` 设成 `__restore` 的地址，`sp` 设为该任务内核栈上 `TrapContext`
    /// 所在位置。这样 `__switch` 切进来后会跳到 `__restore`，后者再依据
    /// `TrapContext` 完成寄存器恢复并 `sret` 进入用户态。
    pub fn goto_restore(kstack_ptr: usize) -> Self {
        extern "C" {
            fn __restore();
        }
        Self {
            ra: __restore as usize,
            sp: kstack_ptr,
            s: [0; 12],
        }
    }
}
