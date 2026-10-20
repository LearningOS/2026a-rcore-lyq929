//! RISC-V 时钟相关功能。
//!
//! RISC-V 提供一个单调递增的 `time` CSR，它以 `CLOCK_FREQ` 为频率不断累加。
//! 我们在这里把它换算成毫秒/微秒，并且负责“下一个时间片什么时候到期”的设定。

use crate::config::CLOCK_FREQ;
use crate::sbi::set_timer;
use riscv::register::time;

/// 每秒包含多少个时间片（即每秒触发多少次时钟中断）。
const TICKS_PER_SEC: usize = 100;

/// 一秒含多少毫秒。
#[allow(dead_code)]
const MSEC_PER_SEC: usize = 1000;

/// 一秒含多少微秒。
#[allow(dead_code)]
const MICRO_PER_SEC: usize = 1_000_000;

/// 读取当前时刻的原始 tick 数。
pub fn get_time() -> usize {
    time::read()
}

/// 当前时刻换算成毫秒。
///
/// 换算公式：`ticks * 1000 / CLOCK_FREQ`。目前没有内核使用它，保留备用。
#[allow(dead_code)]
pub fn get_time_ms() -> usize {
    time::read() * MSEC_PER_SEC / CLOCK_FREQ
}

/// 当前时刻换算成微秒（`sys_get_time` 会用）。
pub fn get_time_us() -> usize {
    time::read() * MICRO_PER_SEC / CLOCK_FREQ
}

/// 设定下一次时钟中断到来的时刻。
///
/// 基准是“现在”，间隔是一个时间片（`CLOCK_FREQ / TICKS_PER_SEC` tick）。
/// 内核在每个时间片结束时都会重设一次，从而让时钟中断稳定地周期性到来，
/// 这正是时间片轮转调度的“心跳”。
pub fn set_next_trigger() {
    set_timer(get_time() + CLOCK_FREQ / TICKS_PER_SEC);
}
