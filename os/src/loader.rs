//! 用户应用的加载与内核/用户栈分配。
//!
//! 第三章还没有内存管理单元/分页，所有用户程序其实是被“打包”进内核二进制
//! 的一段只读数据（由 `build.rs` 在编译期把 `../user/build/bin/*.bin` 拼进
//! `link_app.S`）。所以我们所谓的“加载”本质上就是一次内存拷贝：把某段只读
//! 数据原样搬到指定的物理区间。
//!
//! 同时，这里也为每个任务静态分配了独立的内核栈与用户栈。栈空间用编译期
//! 定长的数组表达，天然避开了 `no_std` 下动态分配的麻烦。

use crate::config::*;
use crate::trap::TrapContext;
use core::arch::asm;

/// 内核栈：8 KiB，按 4096 对齐。
///
/// 之所以要对齐，是因为后续要做栈指针相关操作时，对齐的基地址更容易保证
/// 栈顶也能保持对齐，避免某些指令（如需要 16 字节对齐的访存）出错。
#[repr(align(4096))]
#[derive(Copy, Clone)]
struct KernelStack {
    data: [u8; KERNEL_STACK_SIZE],
}

/// 用户栈：4 KiB，同样按 4096 对齐。
#[repr(align(4096))]
#[derive(Copy, Clone)]
struct UserStack {
    data: [u8; USER_STACK_SIZE],
}

/// 每个任务各有一块内核栈与用户栈，用数组索引直接对应任务编号。
static KERNEL_STACK: [KernelStack; MAX_APP_NUM] = [KernelStack {
    data: [0; KERNEL_STACK_SIZE],
}; MAX_APP_NUM];

static USER_STACK: [UserStack; MAX_APP_NUM] = [UserStack {
    data: [0; USER_STACK_SIZE],
}; MAX_APP_NUM];

impl KernelStack {
    /// 栈顶地址 = 数组起始地址 + 栈大小（RISC-V 栈向下增长，栈顶在最高处）。
    fn top(&self) -> usize {
        self.data.as_ptr() as usize + KERNEL_STACK_SIZE
    }

    /// 在栈顶腾出一块空间放下 `TrapContext`，返回它在栈上的地址。
    ///
    /// 这个地址之后会交给汇编里的 `__restore`，它会把寄存器状态从这块
    /// 结构体里还原出来，从而“无缝”跳进用户态。
    pub fn push_trap_context(&self, trap_cx: TrapContext) -> usize {
        let trap_cx_ptr =
            (self.top() - core::mem::size_of::<TrapContext>()) as *mut TrapContext;
        unsafe {
            *trap_cx_ptr = trap_cx;
        }
        trap_cx_ptr as usize
    }
}

impl UserStack {
    /// 用户栈顶地址，作为初始化 `TrapContext` 时的初始 `sp`。
    fn top(&self) -> usize {
        self.data.as_ptr() as usize + USER_STACK_SIZE
    }
}

/// 查询用户程序的总数。
///
/// `_num_app` 是 `build.rs` 生成在数据段里的一个符号，它指向一个 `usize`，
/// 记录的正是被打包进来的程序个数。这里用 volatile 读保证不被打乱顺序。
pub fn get_num_app() -> usize {
    extern "C" {
        fn _num_app();
    }
    unsafe { (_num_app as usize as *const usize).read_volatile() }
}

/// 第 `app_id` 个程序应被拷贝到的目标基地址。
///
/// 每个程序占用 `APP_SIZE_LIMIT` 长的固定区间，区间彼此不重叠，所以
/// 用“基地址 + 编号 × 区间长”就能算出它的落脚点。
fn app_load_base(app_id: usize) -> usize {
    APP_BASE_ADDRESS + app_id * APP_SIZE_LIMIT
}

/// 把第 `app_id` 个程序搬进它的固定区间。
///
/// 先把区间清成 0，再把源数据逐字节拷贝过去。清 0 是为了保证即使程序
/// 比区间短，残留的高位地址里也不会出现上一次运行的“脏数据”。
fn copy_app_into(app_id: usize, app_start: &[usize]) {
    let base = app_load_base(app_id);

    // 1) 清空目标区间。
    (base..base + APP_SIZE_LIMIT)
        .for_each(|addr| unsafe { (addr as *mut u8).write_volatile(0) });

    // 2) 从数据段拷贝程序本体。
    let src = unsafe {
        core::slice::from_raw_parts(app_start[app_id] as *const u8, app_start[app_id + 1] - app_start[app_id])
    };
    let dst = unsafe { core::slice::from_raw_parts_mut(base as *mut u8, src.len()) };
    dst.copy_from_slice(src);
}

/// 把所有用户程序从内核镜像中加载到各自的固定物理区间。
///
/// 加载完成后执行 `fence.i`：我们刚刚往内存里写了指令（程序代码），而
/// 指令缓存可能还保留着旧内容，必须用这条屏障让之后的取指看到最新写入。
/// 相关语义见 RISC-V 非特权规范第三章的 Zifencei 扩展。
pub fn load_apps() {
    extern "C" {
        fn _num_app();
    }
    let num_app_ptr = _num_app as usize as *const usize;
    let num_app = get_num_app();
    let app_start = unsafe { core::slice::from_raw_parts(num_app_ptr.add(1), num_app + 1) };
    for i in 0..num_app {
        copy_app_into(i, app_start);
    }
    unsafe {
        asm!("fence.i");
    }
}

/// 为第 `app_id` 个任务构造初始 `TrapContext` 并存进它的内核栈，返回栈地址。
///
/// 入口地址指向该程序加载区间的首地址，初始栈指针指向该任务用户栈的栈顶。
/// 这个返回的地址后续会被装进任务的 `TaskContext`，作为第一次 `__switch`
/// 的目标，从而让任务从“用户态程序入口”跑起来。
pub fn init_app_cx(app_id: usize) -> usize {
    KERNEL_STACK[app_id].push_trap_context(TrapContext::app_init_context(
        app_load_base(app_id),
        USER_STACK[app_id].top(),
    ))
}
