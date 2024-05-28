use core::{borrow::BorrowMut, mem::size_of, ptr};
use crate::{
    config::*, fs::{inode::ROOT_INODE, open_file, OpenFlags}, mm::{translated_byte_buffer, translated_ref, translated_refmut, translated_str, MapPermission, VirtAddr}, task::{
        current_process, current_task, current_user_token, exit_current_and_run_next, pid2process, suspend_current_and_run_next, CloneFlags, SignalFlags, TaskStatus, CSIGNAL
    }, timer::{get_time_ms, get_time_us}
};

#[allow(unused)]
use super::errno::{EINVAL, EPERM, SUCCESS};

use alloc::{string::String, sync::Arc, vec::Vec};


#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec:  usize,
    pub usec: usize,
}

#[repr(C)]
pub struct Tms {
    tms_utime:  i64,
    tms_stime:  i64,
    tms_cutime: i64,
    tms_cstime: i64,
}

#[allow(dead_code)]
pub struct Utsname {
    sysname:    [u8; 65],
    nodename:   [u8; 65],
    release:    [u8; 65],
    version:    [u8; 65],
    machine:    [u8; 65],
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

#[repr(C)]
pub struct Dirent {
    ino: u64,
    off: i64,
    len: u16,
    type_: char,
    name: String,
}

impl Dirent {
    pub fn new(off: usize, len: u16, name: String) -> Self {
        Self {
            ino: 0,
            off: off as i64,
            len,
            type_: 'f',
            name,
        }
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
    trace!(
        "kernel:pid[{}] sys_exit",
        current_task().unwrap().process.upgrade().unwrap().getpid()
    );

    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}
/// yield syscall
pub fn sys_yield() -> isize {
    //trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}
/// getpid syscall
pub fn sys_getpid() -> isize {
    trace!(
        "kernel: sys_getpid pid:{}",
        current_task().unwrap().process.upgrade().unwrap().getpid()
    );
    current_task().unwrap().process.upgrade().unwrap().getpid() as isize
}
/// getppid syscall
pub fn sys_getppid() -> isize {
    trace!(
        "kernel: sys_getppid pid:{}",
        current_task().unwrap().process.upgrade().unwrap().getpid()
    );
    if let Some(parent) = &current_task().unwrap()
    .process
    .upgrade().unwrap()
    .inner_exclusive_access().parent {
        parent.upgrade().unwrap().getpid()  as isize
    }
    else {
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
    trace!("[sys_clone] flags {:?} stack_ptr {:x?} ptid {:x?} tls {:x?} ctid {:x?}", flags , stack_ptr, ptid, tls, ctid);
    let current_process = current_process();

    let exit_signal = SignalFlags::from_bits(1 << ((flags & CSIGNAL) - 1)).unwrap();
    let clone_signals = CloneFlags::from_bits((flags & !CSIGNAL) as u32).unwrap();
    
    trace!(
        "[sys_clone] exit_signal = {:?}, clone_signals = {:?}, stack_ptr = {:#x}, ptid = {:#x}, tls = {:#x}, ctid = {:#x}",
         exit_signal, clone_signals, stack_ptr, ptid as usize, tls, ctid as usize
    );
    if !clone_signals.contains(CloneFlags::CLONE_THREAD) {
        // assert!(stack_ptr == 0);
        if stack_ptr == 0 {
            return current_process.fork() as isize;
        } else {
            return current_process.fork2(stack_ptr) as isize; //todo仅用于初赛
        }
    } else {
        println!("[sys_clone] create thread");
        let new_thread = current_process.clone2(exit_signal, clone_signals, stack_ptr, tls);

        // The thread ID of the main thread needs to be the same as the Process ID,
        // so we will exchange the thread whose thread ID is equal to Process ID with the thread whose thread ID is equal to 0,
        // but the system will not exchange it internally
        let process_pid = current_process.getpid();
        let mut new_thread_ttid = new_thread.inner_exclusive_access().gettid();
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
            let mut thread_inner = new_thread.inner_exclusive_access();
            thread_inner.clear_child_tid = ctid as usize;
        }

        return new_thread_ttid as isize;
    }
    
}
/// exec syscall
pub fn sys_execve(path: *const u8, mut args: *const usize) -> isize {
    trace!(
        "kernel:pid[{}] sys_execve",
        current_task().unwrap().process.upgrade().unwrap().getpid()
    );
    let token = current_user_token();
    let path = translated_str(token, path);
    let mut args_vec: Vec<String> = Vec::new();
    loop {
        let arg_str_ptr = *translated_ref(token, args);
        if arg_str_ptr == 0 {
            break;
        }
        args_vec.push(translated_str(token, arg_str_ptr as *const u8));
        unsafe {
            args = args.add(1);
        }
    }
    if let Some(app_inode) = open_file(ROOT_INODE.as_ref(), path.as_str(), OpenFlags::RDONLY) {
        let all_data = app_inode.read_all();
        let process = current_process();
        let argc = args_vec.len();
        process.exec(all_data.as_slice(), args_vec);
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
        let process = current_process();
        let mut inner = process.inner_exclusive_access();
        if !inner
            .children
            .iter()
            .any(|p| pid == -1 || pid as usize == p.getpid())
        {
            return -1;
            // ---- release current PCB
        }
        let pair = inner.children.iter().enumerate().find(|(_, p)| {
            // ++++ temporarily access child PCB exclusively
            p.inner_exclusive_access().is_zombie && (pid == -1 || pid as usize == p.getpid())
            // ++++ release child PCB
        });
        if let Some((idx, _)) = pair {
            let child = inner.children.remove(idx);
            // confirm that child will be deallocated after being removed from children list
            assert_eq!(Arc::strong_count(&child), 1);
            let found_pid = child.getpid();
            // ++++ temporarily access child PCB exclusively
            let exit_code = child.inner_exclusive_access().exit_code;
            // ++++ release child PCB
            if !exit_code_ptr.is_null() {
                *translated_refmut(inner.memory_set.token(), exit_code_ptr) = exit_code << 8;
            }
            return found_pid as isize;
        } else {
            // drop ProcessControlBlock and ProcessControlBlock to avoid mulit-use
            drop(inner);
            drop(process);
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
    trace!(
        "kernel:pid[{}] sys_kill",
        current_task().unwrap().process.upgrade().unwrap().getpid()
    );
    if let Some(process) = pid2process(pid) {
        if let Some(flag) = SignalFlags::from_bits(signal as usize) {
            process.inner_exclusive_access().signals |= flag;
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
    trace!(
        "kernel:pid[{}] sys_get_time",
        current_task().unwrap().process.upgrade().unwrap().getpid()
    );
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
///
/// YOUR JOB: Finish sys_task_info to pass testcases
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TaskInfo`] is splitted by two pages ?
pub fn sys_task_info(ti: *mut TaskInfo) -> isize {
    trace!(
        "kernel:pid[{}] sys_task_info",
        current_task().unwrap().process.upgrade().unwrap().getpid()
    );
    let mut v = translated_byte_buffer(current_user_token(), ti as *const u8, size_of::<TaskInfo>());
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
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
pub fn sys_mmap(start: usize, len: usize, port: usize) -> isize {
    trace!(
        "kernel:pid[{}] sys_mmap",
        current_task().unwrap().process.upgrade().unwrap().getpid()
    );
    if port & !0x7 != 0 {
        return -1;
    }
    if port & 0x7 == 0 {
        return -1;
    }
    let start_va: VirtAddr = start.into();
    if !start_va.aligned() {
        return -1;
    }
    let end_va: VirtAddr = (start + len).into();
    let port = (port << 1 | 0x10) as u8;
    let permission = MapPermission::from_bits(port).unwrap();
    let task = current_task().unwrap();
    let process = task.process.upgrade().unwrap();
    let mut inner = process.inner_exclusive_access();
    if inner.memory_set.is_conflict_with_va(start_va, end_va) {
        -1
    } else {
        inner.memory_set.insert_framed_area(start_va, end_va, permission);
        0
    }
}

/// munmap syscall
///
/// YOUR JOB: Implement munmap.
pub fn sys_munmap(start: usize, len: usize) -> isize {
    trace!(
        "kernel:pid[{}] sys_munmap",
        current_task().unwrap().process.upgrade().unwrap().getpid()
    );
    let start_va: VirtAddr = start.into();
    if !start_va.aligned() {
        return -1;
    }
    let end_va: VirtAddr = (start + len).into();
    let task = current_task().unwrap();
    let process = task.process.upgrade().unwrap();
    let mut inner = process.inner_exclusive_access();

    if inner.memory_set.remove_area_with_va(start_va, end_va) {
        0
    } else {
        -1
    }
}

/// change data segment size
pub fn sys_brk(addr: usize) -> isize {
    // println!("[sys_brk] addr = {:#x}", addr);
    let process = current_process();
    let mut inner = process.inner_exclusive_access();
    if addr == 0 {
        inner.heap_end.0 as isize
    } else if addr < inner.heap_base.0 {
        EINVAL
    } else {
        // We need to calculate to determine if we need a new page table
        // current end page address
        let align_addr = ((addr) + PAGE_SIZE - 1) & (!(PAGE_SIZE - 1));
        // the end of 'addr' value
        let align_end = ((inner.heap_end.0) + PAGE_SIZE - 1) & (!(PAGE_SIZE - 1));
        if align_end >= addr {
            inner.heap_end = addr.into();
            align_addr as isize
        } else {
            let heap_end = inner.heap_end;
            // map heap
            inner.memory_set.map_heap(heap_end, align_addr.into());
            inner.heap_end = align_addr.into();
            addr as isize
        }
    }
}

/// spawn syscall
/// YOUR JOB: Implement spawn.
/// HINT: fork + exec =/= spawn
pub fn sys_spawn(_path: *const u8) -> isize {
    trace!(
        "kernel:pid[{}] sys_spawn",
        current_task().unwrap().process.upgrade().unwrap().getpid()
    );
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
        current_task().unwrap().process.upgrade().unwrap().getpid()
    );
    if prio < 2 {
        return -1;
    }
    let prio = prio as usize;
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    inner.priority = prio;
    inner.pass = BIG_STRIDE / prio;
    prio as isize
}

/// get current process times
#[allow(unused)]
pub fn sys_times(tms: *mut Tms) -> isize {
    trace!(
        "kernel:pid[{}] sys_get_time",
        current_task().unwrap().process.upgrade().unwrap().getpid()
    );
    let mut tms_k = translated_byte_buffer(current_user_token(), tms as *const u8, size_of::<Tms>());
    let (tms_stime, tms_utime) = current_process()
    .inner_exclusive_access()
    .get_process_clock_time();
    let (tms_cstime, tms_cutime) = current_process()
    .inner_exclusive_access()
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
    let mut uts_k = translated_byte_buffer(current_user_token(), uts as *const u8, size_of::<Utsname>());
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