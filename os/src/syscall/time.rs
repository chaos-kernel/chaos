use riscv::register::sstatus;

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
            sstatus::set_sum();
        }
        debug!("timespec: {:#x?}", timespec);
        unsafe {
            *timespec = time;
        }
        unsafe {
            sstatus::clear_sum();
        }
    }
    0
}
