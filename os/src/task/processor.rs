//! Implementation of [`Processor`] and Intersection of control flow
//!
//! Here, the continuous operation of user apps in CPU is maintained,
//! the current running state of CPU is recorded,
//! and the replacement and transfer of control flow of different applications are executed.

use alloc::sync::Arc;

use lazy_static::*;
use riscv::register::satp;

use super::{__switch, fetch_task, TaskContext, TaskControlBlock, TaskStatus};
use crate::{
    mm::{VirtAddr, KERNEL_SPACE},
    sync::UPSafeCell,
    timer::get_time_ms,
    trap::TrapContext,
};

/// Processor management structure
pub struct Processor {
    current: Option<Arc<TaskControlBlock>>,

    ///The basic control flow of each core, helping to select and switch process
    idle_task_cx: TaskContext,
}

impl Processor {
    pub fn new() -> Self {
        Self {
            current:      None,
            idle_task_cx: TaskContext::zero_init(),
        }
    }

    ///Get mutable reference to `idle_task_cx`
    fn get_idle_task_cx_ptr(&mut self) -> *mut TaskContext {
        &mut self.idle_task_cx as *mut _
    }

    ///Get current task in moving semanteme
    pub fn take_current(&mut self) -> Option<Arc<TaskControlBlock>> {
        self.current.take()
    }

    ///Get current task in cloning semanteme
    pub fn current(&self) -> Option<Arc<TaskControlBlock>> {
        self.current.clone()
    }
}

lazy_static! {
    pub static ref PROCESSOR: UPSafeCell<Processor> = unsafe { UPSafeCell::new(Processor::new()) };
}

///The main part of process execution and scheduling
///Loop `fetch_task` to get the process that needs to run, and switch the process through `__switch`
pub fn run_tasks() {
    loop {
        debug!("start new turn of scheduling");
        let mut processor = PROCESSOR.exclusive_access(file!(), line!());
        if let Some(task) = fetch_task() {
            let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
            // access coming task TCB exclusively
            let mut task_inner = task.inner_exclusive_access(file!(), line!());
            let next_task_cx_ptr = &task_inner.task_cx as *const TaskContext;
            task_inner.task_status = TaskStatus::Running;
            if task_inner.first_time.is_none() {
                task_inner.first_time = Some(get_time_ms());
            }

            // 切换进程也要切换页表
            task_inner.task_cx.ra = match task.pid.0 {
                0 => crate::trap::initproc_entry as usize,
                _ => crate::trap::user_entry as usize,
            };

            //被调度，开始计算进程时钟时间
            task_inner.clock_time_refresh();
            // release coming task_inner manually
            drop(task_inner);
            // release coming task TCB manually
            processor.current = Some(task);
            // release processor manually
            drop(processor);
            info!("switch task to pid now");

            unsafe {
                __switch(idle_task_cx_ptr, next_task_cx_ptr);
            }
        } else {
            return;
        }
    }
}

/// Get current task through take, leaving a None in its place
pub fn take_current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access(file!(), line!()).take_current()
}

/// Get a copy of the current task
pub fn current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access(file!(), line!()).current()
}

/// get current pid
pub fn current_pid() -> Option<usize> {
    if let Some(task) = current_task() {
        return Some(task.pid.0);
    }
    None
}

pub fn current_tid() -> Option<usize> {
    if let Some(task) = current_task() {
        return Some(task.tid);
    }
    None
}

/// Get the current user token(addr of page table)
pub fn current_user_token() -> usize {
    let task = current_task().unwrap();
    task.get_user_token()
}

/// Get the mutable reference to trap context of current task
pub fn current_trap_cx() -> &'static mut TrapContext {
    current_task().unwrap().get_trap_cx()
}

/// get the user virtual address of trap context
pub fn current_trap_cx_user_va() -> VirtAddr {
    current_task().unwrap().trap_cx_user_va()
}

/// get the top addr of kernel stack
pub fn current_kstack_top() -> usize {
    current_task().unwrap().kstack.get_top()
}

/// Return to idle control flow for new scheduling
pub fn schedule(switched_task_cx_ptr: *mut TaskContext) {
    let mut processor = PROCESSOR.exclusive_access(file!(), line!());
    let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
    drop(processor);
    unsafe {
        __switch(switched_task_cx_ptr, idle_task_cx_ptr);
    }
}
