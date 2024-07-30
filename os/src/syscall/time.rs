use crate::{
    task::current_task,
    timer::{ClockId, TimeSpec},
};

pub fn sys_clock_gettime(clock_id: usize, timespec: *mut TimeSpec) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_clock_gettime",
        current_task().unwrap().pid.0,
        current_task().unwrap().tid
    );
    let task = current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access(file!(), line!());
    match ClockId::from(clock_id) {
        ClockId::Monotonic | ClockId::Realtime | ClockId::ProcessCputimeId => {
            let time = TimeSpec::now();
            unsafe { *timespec = time };
        }
        _ => {
            panic!("clock_get_time: clock_id {:?} not supported", clock_id);
        }
    }
    let time = TimeSpec::now();
    if timespec as usize != 0 {
        unsafe {
            *timespec = time;
        }
    }
    0
}
