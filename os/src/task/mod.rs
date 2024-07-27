//! Implementation of process [`ProcessControlBlock`] and task(thread) [`TaskControlBlock`] management mechanism
//!
//! Here is the entry for task scheduling required by other modules
//! (such as syscall or clock interrupt).
//! By suspending or exiting the current task, you can
//! modify the task state, manage the task queue through TASK_MANAGER (in task/manager.rs) ,
//! and switch the control flow through PROCESSOR (in task/processor.rs) .
//!
//! Be careful when you see [`__switch`]. Control flow around this function
//! might not be what you expect.

mod context;
mod manager;
pub mod process;
mod processor;
mod res;
mod signal;
mod switch;
#[allow(clippy::module_inception)]
mod task;

use self::manager::add_block_task;
use crate::{
    board::QEMUExit,
    fs::{inode::ROOT_INODE, open_file, OpenFlags},
    timer::remove_timer,
};
use alloc::{sync::Arc, vec::Vec};
use lazy_static::*;
use manager::{add_stopping_task, fetch_task};
pub use process::CloneFlags;
pub use process::CSIGNAL;
use switch::__switch;

pub use context::TaskContext;
pub use manager::{add_task, pid2process, remove_from_pid2process, remove_task, wakeup_task};
pub use processor::{
    current_kstack_top, current_pid, current_task, current_tid, current_trap_cx,
    current_trap_cx_user_va, current_user_token, run_tasks, schedule, take_current_task,
};
pub use res::{kstack_alloc, pid_alloc, KernelStack, PidHandle, IDLE_PID};
pub use signal::SignalFlags;
pub use task::{TaskControlBlock, TaskStatus};

/// Make current task suspended and switch to the next task
pub fn suspend_current_and_run_next() {
    trace!(
        "kernel: pid[{}] suspend_current_and_run_next",
        current_task().unwrap().pid.0
    );
    // There must be an application running.
    let task = take_current_task().unwrap();

    // ---- access current TCB exclusively
    let mut task_inner = task.inner_exclusive_access(file!(), line!());
    let task_cx_ptr = &mut task_inner.task_cx as *mut TaskContext;
    // Change status to Ready
    task_inner.task_status = TaskStatus::Ready;
    drop(task_inner);
    // ---- release current TCB

    // push back to ready queue.
    add_task(task);
    // jump to scheduling cycle
    schedule(task_cx_ptr);
}

/// Make current task blocked and switch to the next task.
pub fn block_current_and_run_next() {
    trace!(
        "kernel: pid[{}] block_current_and_run_next",
        current_task().unwrap().pid.0
    );
    let task = take_current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access(file!(), line!());
    let task_cx_ptr = &mut task_inner.task_cx as *mut TaskContext;
    task_inner.task_status = TaskStatus::Blocked;
    drop(task_inner);
    add_block_task(task);
    schedule(task_cx_ptr);
}

/// Exit the current 'Running' task and run the next task in task list.
pub fn exit_current_and_run_next(exit_code: i32) {
    trace!(
        "kernel: pid[{}] exit_current_and_run_next",
        current_task().unwrap().pid.0
    );
    // take from Processor
    let task = take_current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access(file!(), line!());
    let tid = task.tid;
    // record exit code
    task_inner.exit_code = Some(exit_code);
    // here we do not remove the thread since we are still using the kstack
    // it will be deallocated when sys_waittid is called
    // drop(task_inner);
    // drop(task);

    // Move the task to stop-wait status, to avoid kernel stack from being freed
    // if tid == task.pid.0 {
    //     add_stopping_task(task.clone());
    // } else {
    //     drop(task);
    // }
    // however, if this is the main thread of current process
    // the process should terminate at once
    if tid == task.pid.0 {
        let pid = task.pid.0;
        if pid == IDLE_PID {
            println!(
                "[kernel] Idle process exit with exit_code {} ...",
                exit_code
            );
            if exit_code != 0 {
                //crate::sbi::shutdown(255); //255 == -1 for err hint
                crate::board::QEMU_EXIT_HANDLE.exit_failure();
            } else {
                //crate::sbi::shutdown(0); //0 for success hint
                crate::board::QEMU_EXIT_HANDLE.exit_success();
            }
        }
        remove_from_pid2process(pid);
        // mark this process as a zombie process
        task_inner.is_zombie = true;
        // record exit code of main process
        task_inner.exit_code = Some(exit_code);

        {
            // move all child processes under init process
            let mut initproc_inner = INITPROC.inner_exclusive_access(file!(), line!());
            for child in task_inner.children.iter() {
                child.inner_exclusive_access(file!(), line!()).parent =
                    Some(Arc::downgrade(&INITPROC));
                initproc_inner.children.push(child.clone());
            }
        }

        // deallocate user res (including tid/trap_cx/ustack) of all threads
        // it has to be done before we dealloc the whole memory_set
        // otherwise they will be deallocated twice
        /*
         * now we removed TaskUserRes, so we do not need to deallocate it here.
         * 这里应该是要移除所有子线程，但是目前既没有用到线程，也没有写获取所有子线程的方法
         * 子线程的唯一标识也理论上没有，只能查找所有tid一样，且tid和pid不一样的然后移除
         * 还没写， 这里先空着
         *
         * 两个小时之后
         *
         * 更新了，加了一个threads Vec管理所有线程，现在直接全部取出来都删掉就行了
         */
        for task in task_inner.threads.iter().filter(|t| t.is_some()) {
            let task = task.as_ref().unwrap();
            // if other tasks are Ready in TaskManager or waiting for a timer to be
            // expired, we should remove them.
            //
            // Mention that we do not need to consider Mutex/Semaphore since they
            // are limited in a single process. Therefore, the blocked tasks are
            // removed when the PCB is deallocated.
            trace!("kernel: exit_current_and_run_next .. remove_inactive_task");
            remove_inactive_task(Arc::clone(&task));
        }
        // dealloc_tid and dealloc_user_res require access to PCB inner, so we
        // need to collect those user res first, then release process_inner
        // for now to avoid deadlock/double borrow problem.
        drop(task_inner);

        let mut task_inner = task.inner_exclusive_access(file!(), line!());
        task_inner.children.clear();
        // deallocate other data in user space i.e. program code/data section
        task_inner.memory_set.recycle_data_pages();
        // drop file descriptors
        task_inner.fd_table.clear();
        // remove all threads
        task_inner.threads.clear();
        drop(task_inner);
    }
    // we do not have to save task context
    let mut _unused = TaskContext::zero_init();
    schedule(&mut _unused as *mut _);
}

lazy_static! {
    /// Creation of initial process
    ///
    /// the name "initproc" may be changed to any other app name like "usertests",
    /// but we have user_shell, so we don't need to change it.
    pub static ref INITPROC: Arc<TaskControlBlock> = {
        unsafe {
            extern "C" {
                fn initproc_start();
                fn initproc_end();
            }
            let start = initproc_start as usize as *const usize as *const u8;
            let len = initproc_end as usize - initproc_start as usize;
            let data = core::slice::from_raw_parts(start, len);
            TaskControlBlock::init_task(data)
        }
    };
}

///Add init process to the manager
pub fn add_initproc() {
    debug!("kernel: add_initproc");
    let _initproc = INITPROC.clone();
}

/// Run all files in the root directory
pub fn add_file(file: &str) {
    // 引入初始进程后已弃用
    debug!("kernel: open file Inode: {}", file);
    let inode = open_file(ROOT_INODE.as_ref(), &file, OpenFlags::RDONLY).unwrap();
    debug!("kernel: read from Inode: {}", file);
    let v = inode.read_all();
    debug!("kernel: create PCB: {}", file);
    let _tcb = TaskControlBlock::init_task(v.as_slice());
    debug!("PCB created: {}", file);
}

/// Check if the current task has any signal to handle
pub fn check_signals_of_current() -> Option<(i32, &'static str)> {
    let task = current_task().unwrap();
    let task_inner = task.inner_exclusive_access(file!(), line!());
    task_inner.signals.check_error()
}

/// Add signal to the current task
pub fn current_add_signal(signal: SignalFlags) {
    let task = current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access(file!(), line!());
    task_inner.signals |= signal;
}

/// the inactive(blocked) tasks are removed when the PCB is deallocated.(called by exit_current_and_run_next)
pub fn remove_inactive_task(task: Arc<TaskControlBlock>) {
    remove_task(Arc::clone(&task));
    trace!("kernel: remove_inactive_task .. remove_timer");
    remove_timer(Arc::clone(&task));
}
