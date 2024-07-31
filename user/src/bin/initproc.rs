#![no_std]
#![no_main]

extern crate user_lib;

use user_lib::{exec, fork, wait, yield_, println};

#[no_mangle]
fn main() -> i32 {
    println!("[initproc] Start running...");

    if fork() == 0 {
        let task = "busybox\0";
        // q: 如果我想传递参数 sh 呢
        let args = [""];
        let a = args.map(|arg| arg.as_ptr());
        println!("[initproc] exec busybox sh...");
        exec(&task, &a);
    } else {
        // 父进程等待所有子进程结束
        loop {
            let mut exit_code: i32 = 0;
            let pid = wait(&mut exit_code);
            if pid == -1 {
                yield_();
                continue;
            }
        

            if pid == -10 {
                println!("[initproc] All tasks have exited, shutting down...");
                break;
            }
            else {
                println!(
                    "[initproc] Released a zombie process, pid={}, exit_code={}",
                    pid,
                    exit_code,
                );
            }
        }
    }
    0
}