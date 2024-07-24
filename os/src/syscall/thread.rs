use crate::{
    mm::kernel_token,
    task::{add_task, current_task, kstack_alloc, TaskControlBlock},
    trap::{trap_handler, TrapContext},
};
use alloc::{sync::Arc, vec::Vec};
/// thread create syscall
pub fn sys_thread_create(entry: usize, arg: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_thread_create",
        current_task().unwrap().pid.0,
        current_task().unwrap().tid
    );
    // let task = current_task().unwrap();
    // let kstack = kstack_alloc();
    // // create a new thread
    // let new_task = Arc::new(task.clone2(
    //     Arc::clone(&process),
    //     task.inner_exclusive_access(file!(), line!()).user_stack_top,
    //     0,
    //     true,
    // ));
    // // add new task to scheduler
    // add_task(Arc::clone(&new_task));
    // let new_task_tid = new_task.tid;
    // let mut process_inner = process.inner_exclusive_access(file!(), line!());
    // // add new thread to current process
    // let tasks = &mut process_inner.tasks;
    // while tasks.len() < new_task_tid + 1 {
    //     tasks.push(None);
    // }
    // tasks[new_task_tid] = Some(Arc::clone(&new_task));
    // let finish = &mut process_inner.finish;
    // while finish.len() < new_task_tid + 1 {
    //     finish.push(false);
    // }
    // finish[new_task_tid] = false;
    // let new_task_trap_cx = new_task.get_trap_cx();
    // *new_task_trap_cx = TrapContext::app_init_context(
    //     entry,
    //     new_task.inner_exclusive_access(file!(), line!()).user_stack_top,
    //     kernel_token(),
    //     new_task.kstack.get_top(),
    //     trap_handler as usize,
    // );
    // (*new_task_trap_cx).x[10] = arg;
    // new_task_tid as isize
    0 as isize
}
/// get current thread id syscall
pub fn sys_gettid() -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_gettid",
        current_task().unwrap().pid.0,
        current_task().unwrap().tid
    );
    current_task().unwrap().tid as isize
}

/// wait for a thread to exit syscall
///
/// thread does not exist, return -1
/// thread has not exited yet, return -2
/// otherwise, return thread's exit code
pub fn sys_waittid(tid: usize) -> i32 {
    trace!(
        "kernel:pid[{}] tid[{}] sys_waittid",
        current_task().unwrap().pid.0,
        current_task().unwrap().tid
    );
    // let task = current_task().unwrap();
    // let task_inner = task.inner_exclusive_access(file!(), line!());
    // // a thread cannot wait for itself
    // if task.tid == tid {
    //     return -1;
    // }
    // let mut exit_code: Option<i32> = None;
    // let waited_task = task_inner.tasks[tid].as_ref();
    // if let Some(waited_task) = waited_task {
    //     if let Some(waited_exit_code) = waited_task.inner_exclusive_access(file!(), line!()).exit_code {
    //         exit_code = waited_task.inner_exclusive_access(file!(), line!()).exit_code;
    //     }
    // } else {
    //     // waited thread does not exist
    //     return -1;
    // }
    // if let Some(exit_code) = exit_code {
    //     // dealloc the exited thread
    //     task_inner.tasks[tid] = None;
    //     exit_code
    // } else {
    //     // waited thread has not exited
    //     -2
    // }
    -1
}
