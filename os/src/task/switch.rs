//!provides __switch asm function to switch between two task contexts  [`TaskContext`]
use core::arch::global_asm;

use super::TaskContext;

global_asm!(include_str!("switch.S"));

extern "C" {
    /// Switch to the context of `next_task_cx_ptr`, saving the current context
    /// in `current_task_cx_ptr`.
    pub fn __switch(current_task_cx_ptr: *mut TaskContext, next_task_cx_ptr: *const TaskContext);
    pub fn __schedule(current_task_cx_ptr: *mut TaskContext, next_task_cx_ptr: *const TaskContext);
}
