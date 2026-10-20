//! 内核堆分配器。
//!
//! `no_std` 环境下没有系统的 `malloc`，内核一旦要在运行期动态分配内存（例如
//! 未来任务控制块的指针、各种链表节点），就必须自己提供一个全局分配器。
//! 这里采用 buddy system 分配器，并把一块编译期就预留好的静态数组当作堆。

use crate::config::KERNEL_HEAP_SIZE;
use buddy_system_allocator::LockedHeap;

#[global_allocator]
/// 全局堆分配器实例（内部用锁保护，可并发访问）。
static HEAP_ALLOCATOR: LockedHeap = LockedHeap::empty();

/// 内核堆本体：一块 `KERNEL_HEAP_SIZE` 大小的静态数组。
static mut HEAP_SPACE: [u8; KERNEL_HEAP_SIZE] = [0; KERNEL_HEAP_SIZE];

/// 初始化堆分配器。
///
/// 把分配器的管理范围指向 `HEAP_SPACE` 数组的起止地址。之后内核代码里所有
/// `alloc`/`Vec`/`Box` 等操作都会落到这块空间上。
pub fn init_heap() {
    unsafe {
        HEAP_ALLOCATOR
            .lock()
            .init(HEAP_SPACE.as_ptr() as usize, KERNEL_HEAP_SIZE);
    }
}

#[alloc_error_handler]
/// 堆分配失败时的兜底处理。
///
/// 分配器把可用空间用完时就会走到这里。内存耗尽属于内核级致命错误，直接
/// panic 让系统停下来，比继续在缺内存的状态下运行更安全。
pub fn handle_alloc_error(layout: core::alloc::Layout) -> ! {
    panic!("Heap allocation error, layout = {:?}", layout);
}
