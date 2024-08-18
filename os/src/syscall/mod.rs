//! Implementation of syscalls
//!
//! The single entry point to all system calls, [`syscall()`], is called
//! whenever userspace wishes to perform a system call using the `ecall`
//! instruction. In this case, the processor raises an 'Environment call from
//! U-mode' exception, which is handled as one of the cases in
//! [`crate::trap::trap_handler`].
//!
//! For clarity, each single syscall is implemented as its own function, named
//! `sys_` then the name of the syscall. You can find functions like this in
//! submodules, and you should also implement syscalls this way.
///
pub mod errno;

pub const SYSCALL_GETCWD: usize = 17;
pub const SYSCALL_DUP: usize = 23;
pub const SYSCALL_DUP3: usize = 24;
pub const SYSCALL_FCNTL: usize = 25;
pub const SYSCALL_IOCTL: usize = 29;
pub const SYSCALL_MKDIRAT: usize = 34;
pub const SYSCALL_UNLINKAT: usize = 35;
pub const SYSCALL_LINKAT: usize = 37;
pub const SYSCALL_UMOUNT2: usize = 39;
pub const SYSCALL_MOUNT: usize = 40;
pub const SYSCALL_CHDIR: usize = 49;
pub const SYSCALL_OPENAT: usize = 56;
pub const SYSCALL_CLOSE: usize = 57;
pub const SYSCALL_GETDENTS64: usize = 61;
pub const SYSCALL_READ: usize = 63;
pub const SYSCALL_WRITE: usize = 64;
pub const SYSCALL_WRITEV: usize = 66;
pub const SYSCALL_SENDFILE: usize = 71;
pub const SYSCALL_PPOLL: usize = 73;
pub const SYSCALL_FSTAT: usize = 80;
pub const SYSCALL_EXIT: usize = 93;
pub const SYSCALL_EXIT_GROUP: usize = 94;
pub const SYSCALL_SETTID: usize = 96;
pub const SYSCALL_SLEEP: usize = 101;
pub const SYSCALL_CLOCK_GETTIME: usize = 113;
pub const SYSCALL_YIELD: usize = 124;
pub const SYSCALL_KILL: usize = 129;
pub const SYSCALL_SIGACTION: usize = 134;
pub const SYSCALL_SIGPROCMASK: usize = 135;
pub const SYSCALL_SIGTIMEDWAIT: usize = 137;
pub const SYSCALL_SIGRETURN: usize = 139;
pub const SYSCALL_TIMES: usize = 153;
pub const SYSCALL_UNAME: usize = 160;
pub const SYSCALL_GETTIMEOFDAY: usize = 169;
pub const SYSCALL_GETPID: usize = 172;
pub const SYSCALL_GETPPID: usize = 173;
pub const SYSCALL_GETUID: usize = 174;
pub const SYSCALL_GETEUID: usize = 175;
pub const SYSCALL_GETGID: usize = 176;
pub const SYSCALL_GETEGID: usize = 177;
pub const SYSCALL_GETTID: usize = 178;
pub const SYSCALL_CLONE: usize = 220;
pub const SYSCALL_EXECVE: usize = 221;
pub const SYSCALL_WAIT4: usize = 260;
pub const SYSCALL_SET_PRIORITY: usize = 140;
pub const SYSCALL_BRK: usize = 214;
pub const SYSCALL_MUNMAP: usize = 215;
pub const SYSCALL_MMAP: usize = 222;
pub const SYSCALL_SPAWN: usize = 400;
/*
pub const SYSCALL_MAIL_READ: usize = 401;
pub const SYSCALL_MAIL_WRITE: usize = 402;
*/
pub const SYSCALL_PIPE: usize = 59;
pub const SYSCALL_TASK_INFO: usize = 410;
pub const SYSCALL_THREAD_CREATE: usize = 460;
pub const SYSCALL_WAITTID: usize = 462;
pub const SYSCALL_MUTEX_CREATE: usize = 463;
pub const SYSCALL_MUTEX_LOCK: usize = 464;
pub const SYSCALL_MUTEX_UNLOCK: usize = 466;
pub const SYSCALL_SEMAPHORE_CREATE: usize = 467;
pub const SYSCALL_SEMAPHORE_UP: usize = 468;
pub const SYSCALL_ENABLE_DEADLOCK_DETECT: usize = 469;
pub const SYSCALL_SEMAPHORE_DOWN: usize = 470;
pub const SYSCALL_CONDVAR_CREATE: usize = 471;
pub const SYSCALL_CONDVAR_SIGNAL: usize = 472;
pub const SYSCALL_CONDVAR_WAIT: usize = 473;

mod fs;
mod ppoll;
mod process;
mod signal;
mod sync;
mod thread;
mod time;

use fs::*;
use ppoll::{sys_ppoll, PollFd};
use process::*;
use signal::{sys_sigaction, sys_sigprocmask, sys_sigtimedwait};
use thread::*;
use time::sys_clock_gettime;

use crate::{
    fs::inode::Stat,
    task::{current_task, sigaction::SignalAction, signal::SigInfo, SignalFlags},
    timer::TimeSpec,
};

/// handle syscall exception with `syscall_id` and other arguments
pub fn syscall(syscall_id: usize, args: [usize; 6]) -> isize {
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access(file!(), line!());
    inner.syscall_times[syscall_id] += 1;
    drop(inner);
    drop(task);
    match syscall_id {
        SYSCALL_GETCWD => sys_getcwd(args[0] as *mut u8, args[1]),
        SYSCALL_DUP => sys_dup(args[0]),
        SYSCALL_DUP3 => sys_dup3(args[0], args[1]),
        SYSCALL_LINKAT => sys_linkat(args[1] as *const u8, args[3] as *const u8),
        SYSCALL_UNLINKAT => sys_unlinkat(args[1] as *const u8),
        SYSCALL_OPENAT => sys_openat(args[0] as i32, args[1] as *const u8, args[2] as i32),
        SYSCALL_CLOSE => sys_close(args[0]),
        SYSCALL_PIPE => sys_pipe(args[0] as *mut u32),
        SYSCALL_READ => sys_read(args[0], args[1] as *mut u8, args[2]),
        SYSCALL_WRITE => sys_write(args[0], args[1] as *const u8, args[2]),
        SYSCALL_WRITEV => sys_writev(args[0], args[1], args[2]),
        SYSCALL_FSTAT => sys_fstat(args[0], args[1] as *mut Stat),
        SYSCALL_EXIT => sys_exit(args[0] as i32),
        SYSCALL_EXIT_GROUP => sys_exit_group(args[0] as i32),
        SYSCALL_SETTID => sys_set_tid_address(args[0]),
        // SYSCALL_SLEEP => sys_sleep(args[0] as *const u64, args[1] as *mut u64),
        SYSCALL_CLOCK_GETTIME => sys_clock_gettime(args[0], args[1] as *mut TimeSpec),
        SYSCALL_YIELD => sys_yield(),
        SYSCALL_TIMES => sys_times(args[0] as *mut Tms),
        SYSCALL_UNAME => sys_uname(args[0] as *mut Utsname),
        SYSCALL_GETPID => sys_getpid(),
        SYSCALL_GETPPID => sys_getppid(),
        SYSCALL_GETUID => sys_getuid(),
        SYSCALL_GETEUID => sys_geteuid(),
        SYSCALL_GETGID => sys_getgid(),
        SYSCALL_GETEGID => sys_getegid(),
        SYSCALL_GETTID => sys_gettid(),
        SYSCALL_SIGACTION => sys_sigaction(
            args[0],
            args[1] as *const SignalAction,
            args[2] as *mut SignalAction,
        ),
        SYSCALL_SIGPROCMASK => {
            sys_sigprocmask(args[0], args[1] as *mut usize, args[2] as *mut usize, false)
        }
        SYSCALL_SIGTIMEDWAIT => sys_sigtimedwait(
            args[0] as *mut usize,
            args[1] as *mut SigInfo,
            args[2] as *const TimeSpec,
            args[3],
        ),
        SYSCALL_CLONE => sys_clone(
            args[0],
            args[1],
            args[2] as *mut usize,
            args[3],
            args[4] as *mut usize,
        ),
        SYSCALL_BRK => sys_brk(args[0]),
        SYSCALL_EXECVE => sys_execve(
            args[0] as *const u8,
            args[1] as *const usize,
            args[2] as *const usize,
        ),
        SYSCALL_WAIT4 => sys_wait4(
            args[0] as isize,
            args[1] as *mut i32,
            args[2] as u32,
            args[3],
        ),
        SYSCALL_GETTIMEOFDAY => sys_gettimeofday(args[0] as *mut TimeVal, args[1]),
        SYSCALL_MMAP => sys_mmap(args[0], args[1], args[2], args[3], args[4], args[5]),
        SYSCALL_MUNMAP => sys_munmap(args[0], args[1]),
        SYSCALL_SET_PRIORITY => sys_set_priority(args[0] as isize),
        SYSCALL_TASK_INFO => sys_task_info(args[0] as *mut TaskInfo),
        SYSCALL_SPAWN => sys_spawn(args[0] as *const u8),
        SYSCALL_THREAD_CREATE => sys_thread_create(args[0], args[1]),
        SYSCALL_WAITTID => sys_waittid(args[0]) as isize,
        // SYSCALL_MUTEX_CREATE => sys_mutex_create(args[0] == 1),
        // SYSCALL_MUTEX_LOCK => sys_mutex_lock(args[0]),
        // SYSCALL_MUTEX_UNLOCK => sys_mutex_unlock(args[0]),
        // SYSCALL_SEMAPHORE_CREATE => sys_semaphore_create(args[0]),
        // SYSCALL_SEMAPHORE_UP => sys_semaphore_up(args[0]),
        // SYSCALL_SEMAPHORE_DOWN => sys_semaphore_down(args[0]),
        // SYSCALL_CONDVAR_CREATE => sys_condvar_create(),
        // SYSCALL_CONDVAR_SIGNAL => sys_condvar_signal(args[0]),
        // SYSCALL_CONDVAR_WAIT => sys_condvar_wait(args[0], args[1]),
        SYSCALL_KILL => sys_kill(args[0], args[1] as u32),
        SYSCALL_CHDIR => sys_chdir(args[0] as *const u8),
        SYSCALL_MKDIRAT => sys_mkdirat64(args[0] as i32, args[1] as *const u8, args[2] as u32),
        SYSCALL_GETDENTS64 => sys_getdents64(args[0] as i32, args[1] as *mut u8, args[2]),
        SYSCALL_UMOUNT2 => sys_umount2(args[0] as *const u8, args[1] as i32),
        SYSCALL_MOUNT => sys_mount(
            args[0] as *const u8,
            args[1] as *const u8,
            args[2] as *const u8,
            args[3] as u32,
            args[4] as *const u8,
        ),
        SYSCALL_IOCTL => sys_ioctl(args[0], args[1], args[2]),
        SYSCALL_FCNTL => sys_fcntl(args[0], args[1] as i32, args[2]),
        SYSCALL_PPOLL => sys_ppoll(
            args[0] as *mut PollFd,
            args[1],
            args[2] as *const TimeSpec,
            args[3] as *const SignalFlags,
        ),
        SYSCALL_SENDFILE => sys_sendfile(args[0], args[1], args[2], args[3]),
        _ => panic!("Unsupported syscall_id: {}", syscall_id),
    }
}
