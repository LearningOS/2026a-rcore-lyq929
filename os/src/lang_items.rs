//! panic 处理。
//!
//! `no_std` 内核没有标准库的 panic 机制，必须自己实现 `#[panic_handler]`。
//! 一旦某处 panic，统一到这里打印出错位置与消息，然后请求固件关机。

use crate::sbi::shutdown;
use core::panic::PanicInfo;

#[panic_handler]
/// 内核 panic 的统一出口。
///
/// 能定位到源码位置就打印“文件:行号 + 消息”，否则只打印消息。打印完成后
/// 直接关机，避免系统在异常状态下继续运行造成不可预期后果。
fn panic(info: &PanicInfo) -> ! {
    if let Some(location) = info.location() {
        println!(
            "[kernel] Panicked at {}:{} {}",
            location.file(),
            location.line(),
            info.message().unwrap()
        );
    } else {
        println!("[kernel] Panicked: {}", info.message().unwrap());
    }
    shutdown()
}
