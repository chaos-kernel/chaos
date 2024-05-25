//! The main module and entrypoint
//!
//! Various facilities of the kernels are implemented as submodules. The most
//! important ones are:
//!
//! - [`trap`]: Handles all cases of switching from userspace to the kernel
//! - [`task`]: Task management
//! - [`syscall`]: System call handling and implementation
//! - [`mm`]: Address map using SV39
//! - [`sync`]: Wrap a static data structure inside it so that we are able to access it without any `unsafe`.
//! - [`fs`]: Separate user from file system with some structures
//!
//! The operating system also starts in this module. Kernel code starts
//! executing from `entry.asm`, after which [`rust_main()`] is called to
//! initialize various pieces of functionality. (See its source code for
//! details.)
//!
//! We then call [`task::run_tasks()`] and for the first time go to
//! userspace.

#![deny(missing_docs)]
#![no_std]
#![no_main]
#![feature(panic_info_message)]
#![feature(alloc_error_handler)]

use core::arch::global_asm;

#[macro_use]
extern crate log;

extern crate alloc;

#[macro_use]
extern crate bitflags;

#[path = "boards/qemu.rs"]
mod board;

#[macro_use]
mod console;
pub mod config;
pub mod drivers;
pub mod fs;
pub mod lang_items;
pub mod logging;
pub mod mm;
pub mod sbi;
pub mod sync;
pub mod syscall;
pub mod task;
pub mod timer;
pub mod trap;
pub mod block;

global_asm!(include_str!("entry.asm"));

fn clear_bss() {
    extern "C" {
        fn sbss();
        fn ebss();
    }
    unsafe {
        core::slice::from_raw_parts_mut(sbss as usize as *mut u8, ebss as usize - sbss as usize)
            .fill(0);
    }
}

fn show_logo() {
    println!(
        r#"
 .d88888b.                     .d88888b.   .d8888b.
d88P" "Y88b 888               d88P" "Y88b d88P  Y88b
888     888 888               888     888 Y88b.
888         888d88b.  .d88b.8 888     888  "Y888b.
888         888PY888 d8P""Y88 888     888     "Y88b.
888     888 888  888 888  888 888     888       "888
Y88b. .d88P 888  888 Y8b..d88 Y88b. .d88P Y88b  d88P
 "Y88888P"  888  888  "Y88P`8b "Y88888P"   "Y8888P" 
"#
    );
}

const ALL_TASKS: [&str; 32] = [
    "brk",
    "dup",
    "execve",
    "fork",
    "getcwd",
    "getpid",
    "gettimeofday",
    "mmap",
    "mount",
    "open",
    "pipe",
    "test_echo",
    "times",
    "uname",
    "wait",
    "write",
    "chdir",
    "close",
    "dup2",
    "exit",
    "fstat",
    "getdents",
    "getppid",
    "mkdir_",
    "munmap",
    "openat",
    "read",
    "sleep",
    "umount",
    "unlink",
    "waitpid",
    "yield",
];

#[no_mangle]
/// the rust entry-point of os
pub fn rust_main() -> ! {
    show_logo();
    clear_bss();
    println!("[kernel] Hello, world!");
    logging::init();
    mm::init();
    mm::remap_test();
    trap::init();
    trap::enable_timer_interrupt();
    timer::set_next_trigger();
    task::add_all_files(ALL_TASKS.to_vec());
    task::run_tasks();
    panic!("Unreachable in rust_main!");
}
