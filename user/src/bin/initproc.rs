#![no_std]
#![no_main]

extern crate user_lib;

use user_lib::{exec, fork, wait, yield_};
const ALL_TASKS: [&str; 32] = [
    "read",
    "clone",
    "write",
    "dup2",
    "times",
    "uname",
    "wait",
    "gettimeofday",
    "waitpid",
    "brk",
    "getpid",
    "fork",
    "close",
    "dup",
    "exit",
    "sleep",
    "yield",
    "getppid",
    "open",
    "openat",
    "getcwd",
    "execve",
    "mkdir_",
    "chdir",
    "fstat",
    "mmap",
    "munmap",
    "pipe",
    "mount",
    "umount",
    "getdents",
    "unlink",
];

#[no_mangle]
fn main() -> i32 {

    let mut app_num = 0;

    for app in ALL_TASKS {
        if fork() == 0 {
            app_num += 1;
            // 在子进程中执行应用程序
            exec(app, &[core::ptr::null::<u8>()]);
        }
    }

    // 父进程等待所有子进程结束
    while app_num > 0 {
        let mut exit_code: i32 = 0;
        let pid = wait(&mut exit_code);
        if pid == -1 {
            yield_();
            continue;
        }
        app_num -= 1;
        /*
        println!(
            "[initproc] Released a zombie process, pid={}, exit_code={}",
            pid,
            exit_code,
        );
        */
    }

    0
}