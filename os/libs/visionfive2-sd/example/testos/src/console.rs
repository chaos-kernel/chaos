use core::fmt::{Arguments, Write};

use log::{self, Level, LevelFilter, Log, Metadata, Record};
use preprint::Print;
use spin::Mutex;

pub static UART: Mutex<Uart8250<4>> = Mutex::new(Uart8250::<4>::new(0));

pub struct Uart8250<const W: usize> {
    base: usize,
}

impl<const W: usize> Uart8250<W> {
    pub const fn new(base: usize) -> Self {
        Self { base }
    }
    pub fn init(&self) {
        // enable receive interrupts
        let base = self.base as *mut u8;
        unsafe {
            let ier = base.add(1 * W).read_volatile();
            base.add(1 * W).write_volatile(ier | 0x01);
        }
    }
    pub fn putc(&mut self, c: u8) {
        let base = self.base as *mut u8;
        loop {
            let lsr = unsafe { base.add(5 * W).read_volatile() };
            if lsr & 0x20 != 0 {
                break;
            }
        }
        unsafe {
            base.add(W * 0).write_volatile(c);
        }
    }
    pub fn getc(&mut self) -> Option<u8> {
        let base = self.base as *mut u8;
        let lsr = unsafe { base.add(5 * W).read_volatile() };
        if lsr & 0x01 != 0 {
            Some(unsafe { base.add(0 * W).read_volatile() })
        } else {
            None
        }
    }
}

impl<const W: usize> Write for Uart8250<W> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for c in s.bytes() {
            if c == b'\n' {
                self.putc(b'\r');
            }
            self.putc(c)
        }
        Ok(())
    }
}

pub fn init_uart(addr: usize) {
    let uart = Uart8250::new(addr);
    uart.init();
    *UART.lock() = uart;
}

pub fn __print(fmt: core::fmt::Arguments) {
    UART.lock().write_fmt(fmt).unwrap();
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        let hard_id = $crate::boot::hart_id();
        // [hart_id] xxx
        $crate::console::__print(format_args!("[{}] {}", hard_id, format_args!($($arg)*)))
    };
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($fmt:expr) => ($crate::print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::print!(
        concat!($fmt, "\n"), $($arg)*));
}

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
            Level::Info => 35,  // Blue
            Level::Debug => 32, // Green
            Level::Trace => 90, // BrightBlack
        };
        println!(
            "\u{1B}[{}m[{:>1}] {}\u{1B}[0m",
            color,
            record.level(),
            record.args(),
        );
    }
    fn flush(&self) {}
}

pub fn init_logger() {
    println!("Init logger {:?}", option_env!("LOG"));
    log::set_logger(&SimpleLogger).unwrap();
    log::set_max_level(match option_env!("LOG") {
        Some("ERROR") => LevelFilter::Error,
        Some("WARN") => LevelFilter::Warn,
        Some("INFO") => LevelFilter::Info,
        Some("DEBUG") => LevelFilter::Debug,
        Some("TRACE") => LevelFilter::Trace,
        _ => LevelFilter::Off,
    });
}

pub struct PrePrint;
impl Print for PrePrint {
    fn print(&self, args: Arguments) {
        print!("{}", args);
    }
}

impl Write for PrePrint {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        print!("{}", s);
        Ok(())
    }
}
