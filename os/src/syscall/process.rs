use crate::{
    config::*,
    fs::{inode::ROOT_INODE, open_file, OpenFlags},
    mm::{translated_byte_buffer, translated_refmut},
    task::{
        current_task, current_user_token, exit_current_and_run_next, pid2process,
        suspend_current_and_run_next, CloneFlags, SignalFlags, TaskStatus, CSIGNAL,
    },
    timer::{get_time_ms, get_time_us},
    utils::string::c_ptr_to_string,
};
use core::{borrow::BorrowMut, mem::size_of, ptr};

#[allow(unused)]
use super::errno::{EINVAL, EPERM, SUCCESS};

use alloc::{string::String, sync::Arc, vec::Vec};
use riscv::register::sstatus;

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

#[repr(C)]
pub struct Tms {
    tms_utime: i64,
    tms_stime: i64,
    tms_cutime: i64,
    tms_cstime: i64,
}

#[allow(dead_code)]
pub struct Utsname {
    sysname: [u8; 65],
    nodename: [u8; 65],
    release: [u8; 65],
    version: [u8; 65],
    machine: [u8; 65],
    domainname: [u8; 65],
}
/// Task information
#[allow(dead_code)]
pub struct TaskInfo {
    /// Task status in it's life cycle
    status: TaskStatus,
    /// The numbers of syscall called by task
    syscall_times: [u32; MAX_SYSCALL_NUM],
    /// Total running time of task
    time: usize,
}

#[derive(Debug)]
#[repr(C)]
pub struct Dirent {
    ino: u64,
    off: i64,
    len: u16,
    type_: u8,
    name: [u8; 64],
}

impl Dirent {
    pub fn new(off: usize, len: u16, name: &String) -> Self {
        let mut dirent = Self {
            ino: 0,
            off: off as i64,
            len,
            type_: 0,
            name: [0; 64],
        };
        for (i, c) in name.chars().enumerate() {
            dirent.name[i] = c.as_ascii().unwrap() as u8;
        }
        dirent
    }
}

bitflags! {
    struct WaitOption: u32 {
        const WNOHANG    = 1;
        const WUNTRACED  = 2;
        const WEXITED    = 4;
        const WCONTINUED = 8;
        const WNOWAIT    = 0x1000000;
    }
}

/// exit syscall
///
/// exit the current task and run the next task in task list
pub fn sys_exit(exit_code: i32) -> ! {
    trace!("kernel:pid[{}] sys_exit", current_task().unwrap().pid.0);

    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}
/// yield syscall
pub fn sys_yield() -> isize {
    trace!("kernel:pid[{}] sys_yield", current_task().unwrap().pid.0);
    suspend_current_and_run_next();
    0
}
/// getpid syscall
pub fn sys_getpid() -> isize {
    trace!("kernel: sys_getpid pid:{}", current_task().unwrap().pid.0);

    (current_task().unwrap().pid.0) as isize
}
/// getppid syscall
pub fn sys_getppid() -> isize {
    trace!("kernel: sys_getppid pid:{}", current_task().unwrap().pid.0);
    if let Some(parent) = &current_task()
        .unwrap()
        .inner_exclusive_access(file!(), line!())
        .parent
    {
        parent.upgrade().unwrap().pid.0 as isize
    } else {
        warn!("kwenel: getppid NOT IMPLEMENTED YET!!");
        1
    }
}
/// fork child process syscall
pub fn sys_clone(
    flags: usize,
    stack_ptr: usize,
    ptid: *mut usize,
    tls: usize,
    ctid: *mut usize,
) -> isize {
    trace!(
        "[sys_clone] flags {:?} stack_ptr {:x?} ptid {:x?} tls {:x?} ctid {:x?}",
        flags,
        stack_ptr,
        ptid,
        tls,
        ctid
    );
    let current_task = current_task().unwrap();

    let exit_signal = SignalFlags::from_bits(1 << ((flags & CSIGNAL) - 1)).unwrap();
    let clone_signals = CloneFlags::from_bits((flags & !CSIGNAL) as u32).unwrap();

    trace!(
        "[sys_clone] exit_signal = {:?}, clone_signals = {:?}, stack_ptr = {:#x}, ptid = {:#x}, tls = {:#x}, ctid = {:#x}",
         exit_signal, clone_signals, stack_ptr, ptid as usize, tls, ctid as usize
    );
    if !clone_signals.contains(CloneFlags::CLONE_THREAD) {
        // assert!(stack_ptr == 0);
        if stack_ptr == 0 {
            return current_task.fork() as isize;
        } else {
            // return current_task.fork2(stack_ptr) as isize; //todo仅用于初赛
            return current_task.fork() as isize; //todo
        }
    } else {
        println!("[sys_clone] create thread");
        let new_thread = current_task.clone2(exit_signal, clone_signals, stack_ptr, tls);

        // The thread ID of the main thread needs to be the same as the Process ID,
        // so we will exchange the thread whose thread ID is equal to Process ID with the thread whose thread ID is equal to 0,
        // but the system will not exchange it internally
        let process_pid = current_task.pid.0;
        let mut new_thread_ttid = new_thread.gettid();
        if new_thread_ttid == process_pid {
            new_thread_ttid = 0;
        }

        let token = current_user_token();
        if clone_signals.contains(CloneFlags::CLONE_PARENT_SETTID) {
            if !ptid.is_null() {
                *translated_refmut(token, ptid) = new_thread_ttid;
            }
        }
        if clone_signals.contains(CloneFlags::CLONE_CHILD_SETTID) {
            if !ctid.is_null() {
                *translated_refmut(token, ctid) = new_thread_ttid;
            }
        }
        if clone_signals.contains(CloneFlags::CLONE_CHILD_CLEARTID) {
            let mut thread_inner = new_thread.inner_exclusive_access(file!(), line!());
            thread_inner.clear_child_tid = ctid as usize;
        }

        return new_thread_ttid as isize;
    }
}
/// exec syscall
pub fn sys_execve(path: *const u8, mut args: *const usize) -> isize {
    trace!("kernel:pid[{}] sys_execve", current_task().unwrap().pid.0);
    unsafe {
        sstatus::set_sum();
    }
    let path = c_ptr_to_string(path);
    debug!("kernel: execve new app : {}", path);
    let mut args_vec: Vec<String> = Vec::new();
    loop {
        if unsafe { *args == 0 } {
            break;
        }
        args_vec.push(c_ptr_to_string(unsafe { (*args) as *const u8 }));
        debug!("exec get an arg {}", args_vec[args_vec.len() - 1]);
        unsafe {
            args = args.add(1);
        }
    }
    unsafe {
        sstatus::clear_sum();
    }
    if let Some(app_inode) = open_file(ROOT_INODE.as_ref(), path.as_str(), OpenFlags::RDONLY) {
        debug!("kernel: execve open app success : {}", path.as_str());
        let all_data = app_inode.read_all();
        debug!("kernel: execve read app success : {}", path.as_str());
        let task = current_task().unwrap();
        let argc = args_vec.len();
        task.exec(all_data.as_slice(), args_vec);
        // return argc because cx.x[10] will be covered with it later
        argc as isize
    } else {
        -1
    }
}

/// waitpid syscall
///
/// If there is not a child process whose pid is same as given, return -1.
/// Else if there is a child process but it is still running, return -2.
pub fn sys_wait4(pid: isize, exit_code_ptr: *mut i32, option: u32, _ru: usize) -> isize {
    trace!("kernel: sys_waitpid");
    let option = WaitOption::from_bits(option).unwrap();
    loop {
        let task = current_task().unwrap();
        let mut inner = task.inner_exclusive_access(file!(), line!());
        if !inner
            .children
            .iter()
            .any(|p| pid == -1 || pid as usize == p.pid.0)
        {
            return -1;
            // ---- release current PCB
        }
        let pair = inner.children.iter().enumerate().find(|(_, p)| {
            // ++++ temporarily access child PCB exclusively
            p.inner_exclusive_access(file!(), line!()).is_zombie
                && (pid == -1 || pid as usize == p.pid.0)
            // ++++ release child PCB
        });
        if let Some((idx, _)) = pair {
            let child = inner.children.remove(idx);
            // confirm that child will be deallocated after being removed from children list
            assert_eq!(Arc::strong_count(&child), 1);
            let found_pid = child.pid.0;
            // ++++ temporarily access child PCB exclusively
            let exit_code = child
                .inner_exclusive_access(file!(), line!())
                .exit_code
                .unwrap();
            // ++++ release child PCB
            if !exit_code_ptr.is_null() {
                *translated_refmut(inner.memory_set.token(), exit_code_ptr) = exit_code << 8;
            }
            return found_pid as isize;
        } else {
            // drop ProcessControlBlock and ProcessControlBlock to avoid mulit-use
            drop(inner);
            drop(task);
            if option.contains(WaitOption::WNOHANG) {
                return 0;
            } else {
                suspend_current_and_run_next();
                //block_current_and_run_next();
            }
        }
    }

    // ---- release current PCB automatically
}

/// kill syscall
pub fn sys_kill(pid: usize, signal: u32) -> isize {
    trace!("kernel:pid[{}] sys_kill", current_task().unwrap().pid.0);
    if let Some(process) = pid2process(pid) {
        if let Some(flag) = SignalFlags::from_bits(signal as usize) {
            process.inner_exclusive_access(file!(), line!()).signals |= flag;
            0
        } else {
            -1
        }
    } else {
        -1
    }
}

/// get_time syscall
///
/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_gettimeofday(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel:pid[{}] sys_get_time", current_task().unwrap().pid.0);
    let us = get_time_us();
    let mut v = translated_byte_buffer(current_user_token(), ts as *const u8, size_of::<TimeVal>());
    let mut ts = TimeVal {
        sec: us / 1_000_000,
        usec: us % 1_000_000,
    };
    unsafe {
        let mut p = ts.borrow_mut() as *mut TimeVal as *mut u8;
        for slice in v.iter_mut() {
            let len = slice.len();
            ptr::copy_nonoverlapping(p, slice.as_mut_ptr(), len);
            p = p.add(len);
        }
    }
    0
}

/// task_info syscall
pub fn sys_task_info(ti: *mut TaskInfo) -> isize {
    trace!(
        "kernel:pid[{}] sys_task_info",
        current_task().unwrap().pid.0
    );
    let mut v =
        translated_byte_buffer(current_user_token(), ti as *const u8, size_of::<TaskInfo>());
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access(file!(), line!());
    let mut ti = TaskInfo {
        status: TaskStatus::Running,
        syscall_times: inner.syscall_times,
        time: get_time_ms() - inner.first_time.unwrap(),
    };
    unsafe {
        let mut p = ti.borrow_mut() as *mut TaskInfo as *mut u8;
        for slice in v.iter_mut() {
            let len = slice.len();
            ptr::copy_nonoverlapping(p, slice.as_mut_ptr(), len);
            p = p.add(len);
        }
    }
    0
}

/// mmap syscall
///
/// YOUR JOB: Implement mmap.
pub fn sys_mmap(
    start: usize,
    len: usize,
    prot: usize,
    flags: usize,
    fd: usize,
    off: usize,
) -> isize {
    trace!(
        "kernel:pid[{}] sys_mmap start:{:#x} len:{} prot:{} flags:{} fd:{} off:{}",
        current_task().unwrap().pid.0,
        start,
        len,
        prot,
        flags,
        fd,
        off
    );
    if start as isize == -1 || len == 0 {
        debug!("mmap: invalid arguments");
        return EPERM;
    }
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access(file!(), line!());
    inner.mmap(start, len, prot, flags, fd, off)
}

/// munmap syscall
pub fn sys_munmap(start: usize, len: usize) -> isize {
    trace!("kernel:pid[{}] sys_munmap", current_task().unwrap().pid.0);
    current_task()
        .unwrap()
        .inner_exclusive_access(file!(), line!())
        .munmap(start, len)
}

/// change data segment size
pub fn sys_brk(addr: usize) -> isize {
    // println!("[sys_brk] addr = {:#x}", addr);
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access(file!(), line!());
    if addr == 0 {
        inner.heap_end.0 as isize
    } else if addr < inner.heap_base.0 {
        EINVAL
    } else {
        // We need to calculate to determine if we need a new page table
        // current end page address
        let _align_addr = ((addr) + PAGE_SIZE - 1) & (!(PAGE_SIZE - 1));
        // the end of 'addr' value
        let align_end = ((inner.heap_end.0) + PAGE_SIZE - 1) & (!(PAGE_SIZE - 1));
        if align_end >= addr {
            inner.heap_end = addr.into();
            //todo: should return aligned adreess
            addr as isize
        } else {
            let heap_end = inner.heap_end;
            // map heap
            //todo: aim_addr should map aligned adreess
            inner.memory_set.map_heap(heap_end, addr.into());
            inner.heap_end = addr.into();
            addr as isize
        }
    }
}

/// spawn syscall
/// YOUR JOB: Implement spawn.
/// HINT: fork + exec =/= spawn
pub fn sys_spawn(_path: *const u8) -> isize {
    trace!("kernel:pid[{}] sys_spawn", current_task().unwrap().pid.0);
    -1
    // let token = current_user_token();
    // let path = translated_str(token, path);
    // if let Some(app_inode) = open_file(path.as_str(), OpenFlags::RDONLY) {
    //     let task = current_task().unwrap();
    //     let all_data = app_inode.read_all();
    //     let new_task = task.spawn(all_data.as_slice());
    //     let new_pid = new_task.pid.0;
    //     add_task(new_task);
    //     new_pid as isize
    // } else {
    //     -1
    // }
}

/// set priority syscall
///
/// YOUR JOB: Set task priority
pub fn sys_set_priority(prio: isize) -> isize {
    trace!(
        "kernel:pid[{}] sys_set_priority",
        current_task().unwrap().pid.0
    );
    0
}

/// get current process times
#[allow(unused)]
pub fn sys_times(tms: *mut Tms) -> isize {
    trace!("kernel:pid[{}] sys_get_time", current_task().unwrap().pid.0);
    let mut tms_k =
        translated_byte_buffer(current_user_token(), tms as *const u8, size_of::<Tms>());
    let (tms_stime, tms_utime) = current_task()
        .unwrap()
        .inner_exclusive_access(file!(), line!())
        .get_process_clock_time();
    let (tms_cstime, tms_cutime) = current_task()
        .unwrap()
        .inner_exclusive_access(file!(), line!())
        .get_children_process_clock_time();
    let mut sys_tms = Tms {
        tms_utime,
        tms_stime,
        tms_cutime,
        tms_cstime,
    };
    unsafe {
        let mut p = sys_tms.borrow_mut() as *mut Tms as *mut u8;
        for slice in tms_k.iter_mut() {
            let len = slice.len();
            ptr::copy_nonoverlapping(p, slice.as_mut_ptr(), len);
            p = p.add(len);
        }
    }
    (tms_stime + tms_utime) as isize
}

///get OS informations
pub fn sys_uname(uts: *mut Utsname) -> isize {
    let mut uts_k =
        translated_byte_buffer(current_user_token(), uts as *const u8, size_of::<Utsname>());
    let mut sys_uts = Utsname {
        sysname: [0; 65],
        nodename: [0; 65],
        release: [0; 65],
        version: [0; 65],
        machine: [0; 65],
        domainname: [0; 65],
    };

    let sysname_bytes = SYS_NAME.as_bytes();
    let nodename_bytes = SYS_NODENAME.as_bytes();
    let release_bytes = SYS_RELEASE.as_bytes();
    let version_bytes = SYS_VERSION.as_bytes();
    let machine_bytes = "Machine: riscv64".as_bytes();
    let domainname_bytes = "None".as_bytes();

    sys_uts.sysname[..sysname_bytes.len()].copy_from_slice(sysname_bytes);
    sys_uts.nodename[..nodename_bytes.len()].copy_from_slice(nodename_bytes);
    sys_uts.release[..release_bytes.len()].copy_from_slice(release_bytes);
    sys_uts.version[..version_bytes.len()].copy_from_slice(version_bytes);
    sys_uts.machine[..machine_bytes.len()].copy_from_slice(machine_bytes);
    sys_uts.domainname[..domainname_bytes.len()].copy_from_slice(domainname_bytes);
    unsafe {
        let mut p = sys_uts.borrow_mut() as *mut Utsname as *mut u8;
        for slice in uts_k.iter_mut() {
            let len = slice.len();
            ptr::copy_nonoverlapping(p, slice.as_mut_ptr(), len);
            p = p.add(len);
        }
    }
    0
}
