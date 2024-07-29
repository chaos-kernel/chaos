#![no_std]
#![no_main]

extern crate user_lib;

use user_lib::{exec, fork, wait, yield_, println};
const ALL_TASKS: [&str; 1] = [
    "time-test\0"
];

#[no_mangle]
fn main() -> i32 {

    println!("[initproc] Start running...");

    let mut app_num = 0;

    for app in ALL_TASKS {
        app_num += 1;
        if fork() == 0 {
            println!("[initproc] Running app: {}", app);
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