#![no_std]
#![no_main]

extern crate user_lib;

use user_lib::{exec, fork, wait, yield_, println};
const ALL_TASKS: [&str; 32] = [
    "read\0",
    "clone\0",
    "write\0",
    "dup2\0",
    "times\0",
    "uname\0",
    "wait\0",
    "gettimeofday\0",
    "waitpid\0",
    "brk\0",
    "getpid\0",
    "fork\0",
    "close\0",
    "dup\0",
    "exit\0",
    "sleep\0",
    "yield\0",
    "getppid\0",
    "open\0",
    "openat\0",
    "getcwd\0",
    "execve\0",
    "mkdir_\0",
    "chdir\0",
    "fstat\0",
    "mmap\0",
    "munmap\0",
    "pipe\0",
    "mount\0",
    "umount\0",
    "getdents\0",
    "unlink\0",
];

#[no_mangle]
fn main() -> i32 {

    let mut app_num = 0;

    for app in ALL_TASKS {
        app_num += 1;
        let pid = fork();
        println!("[initproc] now in = {}", pid);
        if pid == 0 {
            // 在子进程中执行应用程序
            exec(app, &[core::ptr::null::<u8>()]);
        }
    }

    // 父进程等待所有子进程结束
    while app_num > 0 {
        let mut exit_code: i32 = 0;
        let pid = wait(&mut exit_code);
        if pid == -1 {
            println!("[initproc] running child process");
            yield_();
            continue;
        }
        app_num -= 1;
        
        println!(
            "[initproc] Released a zombie process, pid={}, exit_code={}",
            pid,
            exit_code,
        );
        
    }

    0
}