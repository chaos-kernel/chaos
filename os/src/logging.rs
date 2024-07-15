//! Global logger

use core::fmt;

use alloc::string::{String, ToString};
use log::{Level, LevelFilter, Log, Metadata, Record};

use crate::task::{current_pid, current_process, current_task, current_tid};

/// Add escape sequence to print with color in Linux console
macro_rules! with_color {
    ($args: ident, $color_code: ident) => {{
        format_args!("\u{1B}[{}m{}\u{1B}[0m", $color_code as u8, $args)
    }};
}

/// Print msg with color
pub fn print_in_color(args: fmt::Arguments, color_code: u8) {
    // use crate::arch::io;
    // let _guard = LOG_LOCK.lock();
    // io::putfmt(with_color!(args, color_code));
    print!("{}", with_color!(args, color_code));
}

/// a simple logger
struct SimpleLogger;

impl Log for SimpleLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }
    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let color = match record.level() {
            Level::Error => 31, // Red
            Level::Warn => 93,  // BrightYellow
            Level::Info => 34,  // Blue
            Level::Debug => 32, // Green
            Level::Trace => 90, // BrightBlack
        };
        let pid: isize;
        if let Some(res) = current_pid() {
            pid = res as isize;
        } else {
            pid = -1; // -1 代表当前没有在任何进程内
        }
        // let tid = current_tid().map_or_else(|| "None".to_string(), |tid| tid.to_string());
        print_in_color(
            format_args!(
                "[{:>5}][{}:{}][{}] {}\n",
                record.level(),
                record.file().unwrap(),
                record.line().unwrap(),
                pid,
                // tid,
                record.args()
            ),
            color,
        );
    }
    fn flush(&self) {}
}

/// initiate logger
pub fn init() {
    static LOGGER: SimpleLogger = SimpleLogger;
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(match option_env!("LOG") {
        Some("ERROR") => LevelFilter::Error,
        Some("WARN") => LevelFilter::Warn,
        Some("INFO") => LevelFilter::Info,
        Some("DEBUG") => LevelFilter::Debug,
        Some("TRACE") => LevelFilter::Trace,
        _ => LevelFilter::Error,
    });
}
