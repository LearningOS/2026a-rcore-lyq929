//! 面向主控台的输出驱动。
//!
//! 提供 `print!/println!` 两个宏，最终都落到一次 SBI 控制台写字符调用。
//! 没有走标准库的 `std::io`，因为 `no_std` 下根本没有它。

use crate::sbi::console_putchar;
use core::fmt::{self, Write};

/// 一个“输出目标”抽象：把每个字符转发给 SBI 控制台驱动。
struct Stdout;

impl Write for Stdout {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            console_putchar(c as usize);
        }
        Ok(())
    }
}

/// 把格式化参数写到主控台。
pub fn print(args: fmt::Arguments) {
    Stdout.write_fmt(args).unwrap();
}

/// 输出一行（自动追加换行）到主控台。
#[macro_export]
macro_rules! println {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::console::print(format_args!(concat!($fmt, "\n") $(, $($arg)+)?))
    }
}

/// 输出到主控台（不追加换行）。
#[macro_export]
macro_rules! print {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::console::print(format_args!($fmt $(, $($arg)+)?))
    }
}
