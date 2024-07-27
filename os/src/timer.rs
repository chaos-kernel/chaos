//! RISC-V timer-related functionality

use core::cmp::Ordering;

use crate::config::CLOCK_FREQ;
use crate::sbi::set_timer;
use crate::sync::UPSafeCell;
use crate::task::{current_task, wakeup_task, TaskControlBlock};
use alloc::collections::BinaryHeap;
use alloc::sync::Arc;
use lazy_static::*;
use riscv::register::time;
///纳秒转换关系
pub const NSEC_PER_SEC: usize = 1_000_000_000;
///纳秒转换关系
pub const NSEC_PER_MSEC: usize = 1_000_000;
///纳秒转换关系
pub const NSEC_PER_USEC: usize = 1_000;
/// The number of ticks per second
const TICKS_PER_SEC: usize = 10;
/// The number of milliseconds per second
const MSEC_PER_SEC: usize = 1000;
/// The number of microseconds per second
#[allow(dead_code)]
const MICRO_PER_SEC: usize = 1_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimeSpec {
    /// The tv_sec member represents the elapsed time, in whole seconds.
    pub tv_sec: usize,
    /// The tv_usec member captures rest of the elapsed time, represented as the number of microseconds.
    pub tv_nsec: usize,
}

/// Get the current time in ticks
pub fn get_time() -> usize {
    time::read()
}

/// Get the current time in milliseconds
pub fn get_time_ms() -> usize {
    time::read() * MSEC_PER_SEC / CLOCK_FREQ
}

/// get current time in microseconds
pub fn get_time_us() -> usize {
    time::read() * MICRO_PER_SEC / CLOCK_FREQ
}

/// Set the next timer interrupt
pub fn set_next_trigger() {
    set_timer(get_time() + CLOCK_FREQ / TICKS_PER_SEC);
}

/// condvar for timer
pub struct TimerCondVar {
    /// The time when the timer expires, in milliseconds
    pub expire_ms: usize,
    /// The task to be woken up when the timer expires
    pub task: Arc<TaskControlBlock>,
}

impl PartialEq for TimerCondVar {
    fn eq(&self, other: &Self) -> bool {
        self.expire_ms == other.expire_ms
    }
}
impl Eq for TimerCondVar {}
impl PartialOrd for TimerCondVar {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let a = -(self.expire_ms as isize);
        let b = -(other.expire_ms as isize);
        Some(a.cmp(&b))
    }
}

impl Ord for TimerCondVar {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

lazy_static! {
    /// TIMERS: global instance: set of timer condvars
    static ref TIMERS: UPSafeCell<BinaryHeap<TimerCondVar>> =
        unsafe { UPSafeCell::new(BinaryHeap::<TimerCondVar>::new()) };
}

/// Add a timer
pub fn add_timer(expire_ms: usize, task: Arc<TaskControlBlock>) {
    trace!("kernel:pid[{}] add_timer", current_task().unwrap().pid.0);
    let mut timers = TIMERS.exclusive_access(file!(), line!());
    timers.push(TimerCondVar { expire_ms, task });
}

/// Remove a timer
pub fn remove_timer(task: Arc<TaskControlBlock>) {
    //trace!("kernel:pid[{}] remove_timer", current_task().unwrap().process.upgrade().unwrap().getpid());
    trace!("kernel: remove_timer");
    let mut timers = TIMERS.exclusive_access(file!(), line!());
    let mut temp = BinaryHeap::<TimerCondVar>::new();
    for condvar in timers.drain() {
        if Arc::as_ptr(&task) != Arc::as_ptr(&condvar.task) {
            temp.push(condvar);
        }
    }
    timers.clear();
    timers.append(&mut temp);
    trace!("kernel: remove_timer END");
}

/// Check if the timer has expired
pub fn check_timer() {
    trace!("kernel:pid[{}] check_timer", current_task().unwrap().pid.0);
    let current_ms = get_time_ms();
    let mut timers = TIMERS.exclusive_access(file!(), line!());
    while let Some(timer) = timers.peek() {
        if timer.expire_ms <= current_ms {
            wakeup_task(Arc::clone(&timer.task));
            timers.pop();
        } else {
            break;
        }
    }
}
