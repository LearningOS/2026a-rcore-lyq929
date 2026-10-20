//! 单处理器下的内部可变性封装。
//!
//! 内核里大量全局静态量既需要共享，又需要能在运行期拿到可变引用。直接写
//! `static mut` 会引入大量 `unsafe`。这里提供一个 `UPSafeCell<T>`，把
//! `RefCell` 包一层并声明 `Sync`，从而在单核上可以安全地“独占访问”内部数据。

use core::cell::{RefCell, RefMut};

/// 单核下用于包裹静态数据、提供内部可变性的容器。
///
/// 只有满足以下前提才安全：
/// 1. 运行在单处理器上（否则 `Sync` 是假的）；
/// 2. 任何时候至多一个持有者通过 `exclusive_access` 借用内部数据。
///
/// 因为我们在访问期间用一个 `RefMut` 临时持有可变借用，一旦重复借用就会
/// panic，从而把“数据竞争”从内存级错误降级成可定位的运行时错误。
pub struct UPSafeCell<T> {
    /// 真正的内部数据。
    inner: RefCell<T>,
}

unsafe impl<T> Sync for UPSafeCell<T> {}

impl<T> UPSafeCell<T> {
    /// 构造一个新实例。
    ///
    /// 之所以是 `unsafe fn`：调用方要承诺接下来的用法符合上面列出的前提。
    pub unsafe fn new(value: T) -> Self {
        Self {
            inner: RefCell::new(value),
        }
    }

    /// 独占访问内部数据，返回一个可变借用。
    ///
    /// 若数据已被其它借用占用，会 panic。
    pub fn exclusive_access(&self) -> RefMut<'_, T> {
        self.inner.borrow_mut()
    }
}
