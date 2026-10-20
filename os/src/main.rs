//! 内核入口模块。
//!
//! 内核并不是从本模块的代码开始执行，而是从汇编入口 `entry.asm` 开始：
//! 它先把栈指针切到内核启动栈，然后调用 `rust_main`。从这里开始，我们才
//! 进入 Rust 世界，逐步把内核的各个子系统初始化好，最后切进第一个用户态
//! 程序。
//!
//! 各子模块的分工大致是：
//! - [`trap`]：处理所有“用户态 → 内核态”的切换（trap）；
//! - [`task`]：任务调度与上下文切换；
//! - [`syscall`]：系统调用分发与实现；
//! - [`loader`]：把用户程序加载进内存；
//! - [`timer`]：驱动时间片轮转的时钟。

#![deny(missing_docs)]
#![deny(warnings)]
#![no_std]
#![no_main]
#![feature(panic_info_message)]
#![feature(alloc_error_handler)]

#[macro_use]
extern crate log;

extern crate alloc;

#[macro_use]
mod console;
pub mod config;
mod heap_alloc;
pub mod lang_items;
mod loader;
pub mod logging;
pub mod sbi;
pub mod sync;
pub mod syscall;
pub mod task;
pub mod timer;
pub mod trap;

// 汇编入口与打包好的用户程序数据在编译期被一并嵌入。
core::arch::global_asm!(include_str!("entry.asm"));
core::arch::global_asm!(include_str!("link_app.S"));

/// 把 BSS 段清零。
///
/// BSS 存放的是未初始化（初始值为 0）的静态数据。QEMU 载入镜像时并不保证
/// 这块内存是干净的，所以内核自己动手把 `[sbss, ebss)` 整段清成 0，否则
/// 那些“本应是 0”的变量会带着上电时的随机值。
fn zero_bss() {
    extern "C" {
        fn sbss();
        fn ebss();
    }
    unsafe {
        core::slice::from_raw_parts_mut(sbss as usize as *mut u8, ebss as usize - sbss as usize)
            .fill(0);
    }
}

/// 打印内核镜像的内存布局，用于调试时确认各段被链到了预期的地址。
///
/// 各段边界符号由链接脚本 `linker.ld` 定义，这里把它们以不同日志级别打印，
/// 方便一眼看出 `.text / .rodata / .data / .bss / boot_stack` 的落点。
fn dump_memory_layout() {
    extern "C" {
        fn stext();
        fn etext();
        fn srodata();
        fn erodata();
        fn sdata();
        fn edata();
        fn sbss();
        fn ebss();
        fn boot_stack_lower_bound();
        fn boot_stack_top();
    }
    logging::init();
    println!("[kernel] Hello, world!");
    trace!(
        "[kernel] .text [{:#x}, {:#x})",
        stext as usize,
        etext as usize
    );
    debug!(
        "[kernel] .rodata [{:#x}, {:#x})",
        srodata as usize, erodata as usize
    );
    info!(
        "[kernel] .data [{:#x}, {:#x})",
        sdata as usize, edata as usize
    );
    warn!(
        "[kernel] boot_stack top=bottom={:#x}, lower_bound={:#x}",
        boot_stack_top as usize, boot_stack_lower_bound as usize
    );
    error!("[kernel] .bss [{:#x}, {:#x})", sbss as usize, ebss as usize);
}

#[no_mangle]
/// 内核在 Rust 世界的真正入口：按依赖顺序完成所有初始化后启动第一个任务。
///
/// 注意这个函数的返回类型是 `!`——一旦启动第一个任务，控制流就再也不回来
/// 了；如果真的执行到了末尾，说明发生了不该发生的情况，直接 panic。
pub fn rust_main() -> ! {
    zero_bss();                  // 1. 清 BSS
    dump_memory_layout();        // 2. 初始化日志并打印镜像布局
    heap_alloc::init_heap();     // 3. 初始化内核堆（动态分配依赖它）
    trap::init();                // 4. 设置 stvec，让 trap 进内核
    loader::load_apps();         // 5. 把用户程序搬进内存
    trap::enable_timer_interrupt(); // 6. 打开 S 态时钟中断
    timer::set_next_trigger();   // 7. 设定第一个时间片
    task::run_first_task();      // 8. 切进第一个用户程序
    panic!("Unreachable in rust_main!");
}
