#![no_std]
#![feature(linkage)]
#![feature(panic_info_message)]
#![feature(alloc_error_handler)]

#[macro_use]
pub mod console;
mod lang_items;
mod syscall;

extern crate alloc;
extern crate core;
#[macro_use]
extern crate bitflags;

use alloc::vec::Vec;
use buddy_system_allocator::LockedHeap;
pub use console::{flush, STDIN, STDOUT};
pub use syscall::*;

const USER_HEAP_SIZE: usize = 16384;

static mut HEAP_SPACE: [u8; USER_HEAP_SIZE] = [0; USER_HEAP_SIZE];

#[global_allocator]
static HEAP: LockedHeap = LockedHeap::empty();

#[alloc_error_handler]
pub fn handle_alloc_error(layout: core::alloc::Layout) -> ! {
    panic!("Heap allocation error, layout = {:?}", layout);
}

fn clear_bss() {
    extern "C" {
        fn start_bss();
        fn end_bss();
    }
    unsafe {
        core::slice::from_raw_parts_mut(
            start_bss as usize as *mut u8,
            end_bss as usize - start_bss as usize,
        )
        .fill(0);
    }
}

#[no_mangle]
#[link_section = ".text.entry"]
pub extern "C" fn _start(argc: usize, argv: usize) -> ! {
    clear_bss();
    unsafe {
        HEAP.lock()
            .init(HEAP_SPACE.as_ptr() as usize, USER_HEAP_SIZE);
    }
    let mut v: Vec<&'static str> = Vec::new();
    for i in 0..argc {
        let str_start =
            unsafe { ((argv + i * core::mem::size_of::<usize>()) as *const usize).read_volatile() };
        let len = (0usize..)
            .find(|i| unsafe { ((str_start + *i) as *const u8).read_volatile() == 0 })
            .unwrap();
        v.push(
            core::str::from_utf8(unsafe {
                core::slice::from_raw_parts(str_start as *const u8, len)
            })
            .unwrap(),
        );
    }
    exit(main(argc, v.as_slice()));
}

#[linkage = "weak"]
#[no_mangle]
fn main(_argc: usize, _argv: &[&str]) -> i32 {
    panic!("Cannot find main!");
}

bitflags! {
    pub struct OpenFlags: u32 {
        const RDONLY = 0;
        const WRONLY = 1 << 0;
        const RDWR = 1 << 1;
        const CREATE = 1 << 9;
        const TRUNC = 1 << 10;
    }
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

impl TimeVal {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum TaskStatus {
    UnInit,
    Ready,
    Running,
    Exited,
}

#[derive(Copy, Clone, Debug)]
pub struct SyscallInfo {
    pub id: usize,
    pub times: usize,
}

const MAX_SYSCALL_NUM: usize = 500;

#[derive(Debug)]
pub struct TaskInfo {
    pub status: TaskStatus,
    pub syscall_times: [u32; MAX_SYSCALL_NUM],
    pub time: usize,
}

impl TaskInfo {
    pub fn new() -> Self {
        TaskInfo {
            status: TaskStatus::UnInit,
            syscall_times: [0; MAX_SYSCALL_NUM],
            time: 0,
        }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct Stat {
    /// ID of device containing file
    pub dev: u64,
    /// inode number
    pub ino: u64,
    /// file type and mode
    pub mode: StatMode,
    /// number of hard links
    pub nlink: u32,
    /// unused pad
    pad: [u64; 7],
}

impl Stat {
    pub fn new() -> Self {
        Stat {
            dev: 0,
            ino: 0,
            mode: StatMode::NULL,
            nlink: 0,
            pad: [0; 7],
        }
    }
}

impl Default for Stat {
    fn default() -> Self {
        Self::new()
    }
}

bitflags! {
    pub struct StatMode: u32 {
        const NULL  = 0;
        /// directory
        const DIR   = 0o040000;
        /// ordinary regular file
        const FILE  = 0o100000;
    }
}

const AT_FDCWD: isize = -100;

pub fn open(path: &str, flags: OpenFlags) -> isize {
    sys_openat(AT_FDCWD as usize, path, flags.bits, OpenFlags::RDWR.bits)
}

pub fn close(fd: usize) -> isize {
    if fd == STDOUT {
        console::flush();
    }
    sys_close(fd)
}

pub fn read(fd: usize, buf: &mut [u8]) -> isize {
    sys_read(fd, buf)
}

pub fn write(fd: usize, buf: &[u8]) -> isize {
    sys_write(fd, buf)
}

pub fn link(old_path: &str, new_path: &str) -> isize {
    sys_linkat(AT_FDCWD as usize, old_path, AT_FDCWD as usize, new_path, 0)
}

pub fn unlink(path: &str) -> isize {
    sys_unlinkat(AT_FDCWD as usize, path, 0)
}

pub fn fstat(fd: usize, st: &mut Stat) -> isize {
    sys_fstat(fd, st)
}

pub fn mail_read(buf: &mut [u8]) -> isize {
    sys_mail_read(buf)
}

pub fn mail_write(pid: usize, buf: &[u8]) -> isize {
    sys_mail_write(pid, buf)
}

pub fn exit(exit_code: i32) -> ! {
    console::flush();
    sys_exit(exit_code);
}

pub fn yield_() -> isize {
    sys_yield()
}

pub fn get_time() -> isize {
    let mut time = TimeVal::new();
    match sys_get_time(&mut time, 0) {
        0 => ((time.sec & 0xffff) * 1000 + time.usec / 1000) as isize,
        _ => -1,
    }
}

pub fn getpid() -> isize {
    sys_getpid()
}

pub fn fork() -> isize {
    sys_fork()
}

pub fn exec(path: &str, args: &[*const u8]) -> isize {
    sys_exec(path, args)
}

pub fn set_priority(prio: isize) -> isize {
    sys_set_priority(prio)
}

pub fn wait(exit_code: &mut i32) -> isize {
    loop {
        match sys_waitpid(-1, exit_code as *mut _) {
            -2 => {
                sys_yield();
            }
            n => {
                return n;
            }
        }
    }
}

pub fn waitpid(pid: usize, exit_code: &mut i32) -> isize {
    loop {
        match sys_waitpid(pid as isize, exit_code as *mut _) {
            -2 => {
                sys_yield();
            }
            n => {
                return n;
            }
        }
    }
}

pub fn sleep_blocking(sleep_ms: usize) {
    sys_sleep(sleep_ms);
}

pub fn sleep(period_ms: usize) {
    let start = get_time();
    while get_time() < start + period_ms as isize {
        sys_yield();
    }
}
pub fn mmap(start: usize, len: usize, prot: usize) -> isize {
    sys_mmap(start, len, prot)
}

pub fn munmap(start: usize, len: usize) -> isize {
    sys_munmap(start, len)
}

pub fn sbrk(size: i32) -> isize {
    sys_sbrk(size)
}

pub fn spawn(path: &str) -> isize {
    sys_spawn(path)
}

pub fn dup(fd: usize) -> isize {
    sys_dup(fd)
}
pub fn pipe(pipe_fd: &mut [usize]) -> isize {
    sys_pipe(pipe_fd)
}

pub fn task_info(info: &mut TaskInfo) -> isize {
    sys_task_info(info)
}

pub fn thread_create(entry: usize, arg: usize) -> isize {
    sys_thread_create(entry, arg)
}
pub fn gettid() -> isize {
    sys_gettid()
}
pub fn waittid(tid: usize) -> isize {
    loop {
        match sys_waittid(tid) {
            -2 => {
                yield_();
            }
            exit_code => return exit_code,
        }
    }
}

pub fn mutex_create() -> isize {
    sys_mutex_create(false)
}
pub fn mutex_blocking_create() -> isize {
    sys_mutex_create(true)
}
pub fn mutex_lock(mutex_id: usize) -> isize {
    sys_mutex_lock(mutex_id)
}
pub fn mutex_unlock(mutex_id: usize) {
    sys_mutex_unlock(mutex_id);
}
pub fn semaphore_create(res_count: usize) -> isize {
    sys_semaphore_create(res_count)
}
pub fn semaphore_up(sem_id: usize) {
    sys_semaphore_up(sem_id);
}
pub fn enable_deadlock_detect(enabled: bool) -> isize {
    sys_enable_deadlock_detect(enabled as usize)
}
pub fn semaphore_down(sem_id: usize) -> isize {
    sys_semaphore_down(sem_id)
}
pub fn condvar_create() -> isize {
    sys_condvar_create(0)
}
pub fn condvar_signal(condvar_id: usize) {
    sys_condvar_signal(condvar_id);
}
pub fn condvar_wait(condvar_id: usize, mutex_id: usize) {
    sys_condvar_wait(condvar_id, mutex_id);
}

/// Action for a signal
#[repr(C, align(16))]
#[derive(Debug, Clone, Copy)]
pub struct SignalAction {
    pub handler: usize,
    pub mask: SignalFlags,
}

impl Default for SignalAction {
    fn default() -> Self {
        Self {
            handler: 0,
            mask: SignalFlags::empty(),
        }
    }
}

pub const SIGDEF: i32 = 0; // Default signal handling
pub const SIGHUP: i32 = 1;
pub const SIGINT: i32 = 2;
pub const SIGQUIT: i32 = 3;
pub const SIGILL: i32 = 4;
pub const SIGTRAP: i32 = 5;
pub const SIGABRT: i32 = 6;
pub const SIGBUS: i32 = 7;
pub const SIGFPE: i32 = 8;
pub const SIGKILL: i32 = 9;
pub const SIGUSR1: i32 = 10;
pub const SIGSEGV: i32 = 11;
pub const SIGUSR2: i32 = 12;
pub const SIGPIPE: i32 = 13;
pub const SIGALRM: i32 = 14;
pub const SIGTERM: i32 = 15;
pub const SIGSTKFLT: i32 = 16;
pub const SIGCHLD: i32 = 17;
pub const SIGCONT: i32 = 18;
pub const SIGSTOP: i32 = 19;
pub const SIGTSTP: i32 = 20;
pub const SIGTTIN: i32 = 21;
pub const SIGTTOU: i32 = 22;
pub const SIGURG: i32 = 23;
pub const SIGXCPU: i32 = 24;
pub const SIGXFSZ: i32 = 25;
pub const SIGVTALRM: i32 = 26;
pub const SIGPROF: i32 = 27;
pub const SIGWINCH: i32 = 28;
pub const SIGIO: i32 = 29;
pub const SIGPWR: i32 = 30;
pub const SIGSYS: i32 = 31;

bitflags! {
    pub struct SignalFlags: i32 {
        const SIGDEF = 1; // Default signal handling
        const SIGHUP = 1 << 1;
        const SIGINT = 1 << 2;
        const SIGQUIT = 1 << 3;
        const SIGILL = 1 << 4;
        const SIGTRAP = 1 << 5;
        const SIGABRT = 1 << 6;
        const SIGBUS = 1 << 7;
        const SIGFPE = 1 << 8;
        const SIGKILL = 1 << 9;
        const SIGUSR1 = 1 << 10;
        const SIGSEGV = 1 << 11;
        const SIGUSR2 = 1 << 12;
        const SIGPIPE = 1 << 13;
        const SIGALRM = 1 << 14;
        const SIGTERM = 1 << 15;
        const SIGSTKFLT = 1 << 16;
        const SIGCHLD = 1 << 17;
        const SIGCONT = 1 << 18;
        const SIGSTOP = 1 << 19;
        const SIGTSTP = 1 << 20;
        const SIGTTIN = 1 << 21;
        const SIGTTOU = 1 << 22;
        const SIGURG = 1 << 23;
        const SIGXCPU = 1 << 24;
        const SIGXFSZ = 1 << 25;
        const SIGVTALRM = 1 << 26;
        const SIGPROF = 1 << 27;
        const SIGWINCH = 1 << 28;
        const SIGIO = 1 << 29;
        const SIGPWR = 1 << 30;
        const SIGSYS = 1 << 31;
    }
}

pub fn kill(pid: usize, signum: i32) -> isize {
    sys_kill(pid, signum)
}

pub fn sigaction(
    signum: i32,
    action: Option<&SignalAction>,
    old_action: Option<&mut SignalAction>,
) -> isize {
    sys_sigaction(
        signum,
        action.map_or(core::ptr::null(), |a| a),
        old_action.map_or(core::ptr::null_mut(), |a| a)
    )
}

pub fn sigprocmask(mask: u32) -> isize {
    sys_sigprocmask(mask)
}

pub fn sigreturn() -> isize {
    sys_sigreturn()
}