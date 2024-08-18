#![no_std]
#![no_main]
#![feature(naked_functions)]
#![feature(panic_info_message)]
#![feature(asm_const)]
#![allow(unused)]
extern crate alloc;

use core::panic::PanicInfo;

use boot::{sleep_ms_until, sleep_us};
// use fatfs2::init_fatfs2;
// use vf2_driver::sd::SdHost;
// use vf2_driver::serial;
use visionfive2_sd::{SDIo, SleepOps, Vf2SdDriver};

use crate::boot::{hart_id, sleep_ms};
use crate::config::UART_BASE;
use crate::console::PrePrint;
use crate::fatfs::init_fatfs;
use crate::sbi::shutdown;
use preprint::init_print;

mod boot;
mod config;
mod console;
mod fatfs;
mod static_keys;
// mod fatfs2;
mod sbi;

pub struct SdIoImpl;
pub const SDIO_BASE: usize = 0x16020000;
impl SDIo for SdIoImpl {
    fn read_data_at(&self, offset: usize) -> u64 {
        let addr = (SDIO_BASE + offset) as *mut u64;
        unsafe { addr.read_volatile() }
    }
    fn read_reg_at(&self, offset: usize) -> u32 {
        let addr = (SDIO_BASE + offset) as *mut u32;
        unsafe { addr.read_volatile() }
    }
    fn write_data_at(&mut self, offset: usize, val: u64) {
        let addr = (SDIO_BASE + offset) as *mut u64;
        unsafe { addr.write_volatile(val) }
    }
    fn write_reg_at(&mut self, offset: usize, val: u32) {
        let addr = (SDIO_BASE + offset) as *mut u32;
        unsafe { addr.write_volatile(val) }
    }
}

pub struct SleepOpsImpl;

impl SleepOps for SleepOpsImpl {
    fn sleep_ms(ms: usize) {
        sleep_ms(ms)
    }
    fn sleep_ms_until(ms: usize, f: impl FnMut() -> bool) {
        sleep_ms_until(ms, f)
    }
}

pub fn main() {
    boot::clear_bss();
    boot::init_heap();
    console::init_uart(UART_BASE);
    console::init_logger();
    println!("boot hart_id: {}", hart_id());

    unsafe {
        static_keys::test();
    }

    // init_print(&PrePrint);
    let mut sd = Vf2SdDriver::<_, SleepOpsImpl>::new(SdIoImpl);
    sd.init();
    // serial::init_log(log::LevelFilter::Error).unwrap();
    // let sd = SdHost;
    // sd.init().unwrap();
    println!("sd init ok");
    let mut buf = [0; 512];
    sd.read_block(0, &mut buf);
    println!("buf: {:x?}", &buf[..16]);

    // init_fatfs2(sd);
    init_fatfs(sd);
    println!("shutdown.....");
    shutdown();
}

#[panic_handler]
fn panic_handler(info: &PanicInfo) -> ! {
    if let Some(p) = info.location() {
        println!(
            "line {}, file {}: {}",
            p.line(),
            p.file(),
            info.message().unwrap()
        );
    } else {
        println!("no location information available");
    }
    loop {}
}
