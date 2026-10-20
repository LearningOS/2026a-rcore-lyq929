//! 全局日志器。
//!
//! 用一个极简的 logger 把内核各处的 `trace!/debug!/info!/warn!/error!` 输出
//! 到主控台，并按照日志级别给文本上色，方便在终端里快速区分不同级别。

use log::{Level, LevelFilter, Log, Metadata, Record};

/// 一个非常简单的日志器：把每条记录按级别着色后打印。
struct SimpleLogger;

impl Log for SimpleLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        // 每种级别对应一种 ANSI 颜色：错误红、警告亮黄、信息蓝、调试绿、跟踪亮黑。
        let color = match record.level() {
            Level::Error => 31,
            Level::Warn => 93,
            Level::Info => 34,
            Level::Debug => 32,
            Level::Trace => 90,
        };
        println!(
            "\u{1B}[{}m[{:>5}] {}\u{1B}[0m",
            color,
            record.level(),
            record.args(),
        );
    }

    fn flush(&self) {}
}

/// 初始化日志器，并依据环境变量 `LOG` 决定输出到哪个级别为止。
///
/// 例如 `LOG=INFO make run` 会只打印 INFO 及以上级别；不设置则默认全部关闭。
pub fn init() {
    static LOGGER: SimpleLogger = SimpleLogger;
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(match option_env!("LOG") {
        Some("ERROR") => LevelFilter::Error,
        Some("WARN") => LevelFilter::Warn,
        Some("INFO") => LevelFilter::Info,
        Some("DEBUG") => LevelFilter::Debug,
        Some("TRACE") => LevelFilter::Trace,
        _ => LevelFilter::Off,
    });
}
