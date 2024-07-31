//! RISC-V timer-related functionality

use alloc::{collections::BinaryHeap, sync::Arc};
use core::{
    arch,
    cmp::Ordering,
    ops::{Add, AddAssign, Sub},
};

use lazy_static::*;
use riscv::register::time;

use crate::{
    config::CLOCK_FREQ,
    sbi::set_timer,
    sync::UPSafeCell,
    task::{current_task, wakeup_task, TaskControlBlock},
};
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

pub const USEC_PER_SEC: usize = 1_000_000;
pub const USEC_PER_MSEC: usize = 1_000;

/// The number of microseconds per second
#[allow(dead_code)]
const MICRO_PER_SEC: usize = 1_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Traditional UNIX timespec structures represent elapsed time, measured by the system clock
/// # *CAUTION*
/// tv_sec & tv_usec should be usize.
pub struct TimeSpec {
    /// The tv_sec member represents the elapsed time, in whole seconds.
    pub tv_sec:  usize,
    /// The tv_usec member captures rest of the elapsed time, represented as the number of microseconds.
    pub tv_nsec: usize,
}
impl AddAssign for TimeSpec {
    fn add_assign(&mut self, rhs: Self) {
        self.tv_sec += rhs.tv_sec;
        self.tv_nsec += rhs.tv_nsec;
    }
}
impl Add for TimeSpec {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        let mut sec = self.tv_sec + other.tv_sec;
        let mut nsec = self.tv_nsec + other.tv_nsec;
        sec += nsec / NSEC_PER_SEC;
        nsec %= NSEC_PER_SEC;
        Self {
            tv_sec:  sec,
            tv_nsec: nsec,
        }
    }
}

impl Sub for TimeSpec {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        let self_ns = self.to_ns();
        let other_ns = other.to_ns();
        if self_ns <= other_ns {
            TimeSpec::new()
        } else {
            TimeSpec::from_ns(self_ns - other_ns)
        }
    }
}

impl Ord for TimeSpec {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.tv_sec.cmp(&other.tv_sec) {
            Ordering::Less => Ordering::Less,
            Ordering::Equal => self.tv_nsec.cmp(&other.tv_nsec),
            Ordering::Greater => Ordering::Greater,
        }
    }
}

impl PartialOrd for TimeSpec {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl TimeSpec {
    pub fn new() -> Self {
        Self {
            tv_sec:  0,
            tv_nsec: 0,
        }
    }
    pub fn from_tick(tick: usize) -> Self {
        Self {
            tv_sec:  tick / CLOCK_FREQ,
            tv_nsec: (tick % CLOCK_FREQ) * NSEC_PER_SEC / CLOCK_FREQ,
        }
    }
    pub fn from_s(s: usize) -> Self {
        Self {
            tv_sec:  s,
            tv_nsec: 0,
        }
    }
    pub fn from_ms(ms: usize) -> Self {
        Self {
            tv_sec:  ms / MSEC_PER_SEC,
            tv_nsec: (ms % MSEC_PER_SEC) * NSEC_PER_MSEC,
        }
    }
    pub fn from_us(us: usize) -> Self {
        Self {
            tv_sec:  us / USEC_PER_SEC,
            tv_nsec: (us % USEC_PER_SEC) * NSEC_PER_USEC,
        }
    }
    pub fn from_ns(ns: usize) -> Self {
        Self {
            tv_sec:  ns / NSEC_PER_SEC,
            tv_nsec: ns % NSEC_PER_SEC,
        }
    }
    pub fn to_ns(&self) -> usize {
        self.tv_sec * NSEC_PER_SEC + self.tv_nsec
    }
    pub fn is_zero(&self) -> bool {
        self.tv_sec == 0 && self.tv_nsec == 0
    }
    pub fn now() -> Self {
        TimeSpec::from_tick(get_time())
    }
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
    pub task:      Arc<TaskControlBlock>,
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

// /* Identifier for system-wide realtime clock.  */
// # define CLOCK_REALTIME			0
// /* Monotonic system-wide clock.  */
// # define CLOCK_MONOTONIC		1
// /* High-resolution timer from the CPU.  */
// # define CLOCK_PROCESS_CPUTIME_ID	2
// /* Thread-specific CPU-time clock.  */
// # define CLOCK_THREAD_CPUTIME_ID	3
// /* Monotonic system-wide clock, not adjusted for frequency scaling.  */
// # define CLOCK_MONOTONIC_RAW		4
// /* Identifier for system-wide realtime clock, updated only on ticks.  */
// # define CLOCK_REALTIME_COARSE		5
// /* Monotonic system-wide clock, updated only on ticks.  */
// # define CLOCK_MONOTONIC_COARSE		6
// /* Monotonic system-wide clock that includes time spent in suspension.  */
// # define CLOCK_BOOTTIME			7
// /* Like CLOCK_REALTIME but also wakes suspended system.  */
// # define CLOCK_REALTIME_ALARM		8
// /* Like CLOCK_BOOTTIME but also wakes suspended system.  */
// # define CLOCK_BOOTTIME_ALARM		9
// /* Like CLOCK_REALTIME but in International Atomic Time.  */
// # define CLOCK_TAI			11

const CLOCK_REALTIME: usize = 0;
const CLOCK_MONOTONIC: usize = 1;
const CLOCK_PROCESS_CPUTIME_ID: usize = 2;
const CLOCK_THREAD_CPUTIME_ID: usize = 3;
const CLOCK_MONOTONIC_RAW: usize = 4;
const CLOCK_REALTIME_COARSE: usize = 5;
const CLOCK_MONOTONIC_COARSE: usize = 6;
const CLOCK_BOOTTIME: usize = 7;
const CLOCK_REALTIME_ALARM: usize = 8;
const CLOCK_BOOTTIME_ALARM: usize = 9;
const CLOCK_TAI: usize = 11;

#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockId {
    Realtime = CLOCK_REALTIME,
    Monotonic = CLOCK_MONOTONIC,
    ProcessCputimeId = CLOCK_PROCESS_CPUTIME_ID,
    ThreadCputimeId = CLOCK_THREAD_CPUTIME_ID,
    MonotonicRaw = CLOCK_MONOTONIC_RAW,
    RealtimeCoarse = CLOCK_REALTIME_COARSE,
    MonotonicCoarse = CLOCK_MONOTONIC_COARSE,
    Boottime = CLOCK_BOOTTIME,
    RealtimeAlarm = CLOCK_REALTIME_ALARM,
    BoottimeAlarm = CLOCK_BOOTTIME_ALARM,
    Tai = CLOCK_TAI,
}

impl ClockId {
    pub fn from(clock_id: usize) -> Self {
        match clock_id {
            CLOCK_REALTIME => ClockId::Realtime,
            CLOCK_MONOTONIC => ClockId::Monotonic,
            CLOCK_PROCESS_CPUTIME_ID => ClockId::ProcessCputimeId,
            CLOCK_THREAD_CPUTIME_ID => ClockId::ThreadCputimeId,
            CLOCK_MONOTONIC_RAW => ClockId::MonotonicRaw,
            CLOCK_REALTIME_COARSE => ClockId::RealtimeCoarse,
            CLOCK_MONOTONIC_COARSE => ClockId::MonotonicCoarse,
            CLOCK_BOOTTIME => ClockId::Boottime,
            CLOCK_REALTIME_ALARM => ClockId::RealtimeAlarm,
            CLOCK_BOOTTIME_ALARM => ClockId::BoottimeAlarm,
            CLOCK_TAI => ClockId::Tai,
            _ => panic!("clock_id {:?} not supported", clock_id),
        }
    }
}

#[repr(usize)]
#[allow(non_camel_case_types)]
#[derive(Eq, PartialEq, Debug, Copy, Clone)]
/// sys_settimer / sys_gettimer 中设定的 which，即计时器类型
pub enum TimerType {
    /// 表示目前没有任何计时器(不在linux规范中，自定义标准)
    NONE = 114514,
    /// 统计系统实际运行时间
    REAL = 0,
    /// 统计用户态运行时间
    VIRTUAL = 1,
    /// 统计进程的所有用户态/内核态运行时间
    PROF = 2,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct Times {
    /// the ticks of user mode
    pub tms_utime:  usize,
    /// the ticks of kernel mode
    pub tms_stime:  usize,
    /// the ticks of user mode of child process
    pub tms_cutime: usize,
    /// the ticks of kernel mode of child process
    pub tms_cstime: usize,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Ord, PartialOrd, Eq, PartialEq)]
pub struct TimeVal {
    /// seconds
    pub tv_sec:  usize,
    /// microseconds
    pub tv_usec: usize,
}

/// [`getitimer`] / [`setitimer`] 指定的类型，用户执行系统调用时获取和输入的计时器
// todo 还未投入使用
#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct ITimerVal {
    /// 计时器超时间隔
    pub it_interval: TimeVal,
    /// 计时器当前所剩时间
    pub it_value:    TimeVal,
}
