//! Types related to task management & Functions for completely changing TCB

use super::res::TaskUserRes;
use super::{kstack_alloc, KernelStack, ProcessControlBlock, TaskContext};
use crate::config::{
    BIG_STRIDE, MAX_SYSCALL_NUM, PAGE_SIZE, TRAP_CONTEXT_TRAMPOLINE, USER_STACK_SIZE,
};
use crate::fs::file::File;
use crate::fs::inode::OSInode;
use crate::fs::{Stdin, Stdout};
use crate::mm::{MapPermission, PTEFlags, VirtAddr, KERNEL_SPACE};
use crate::task::res::trap_cx_bottom_from_tid;
use crate::trap::TrapContext;
use crate::{mm::PhysPageNum, sync::UPSafeCell};
use alloc::sync::{Arc, Weak};
use alloc::vec;
use alloc::vec::Vec;
use core::cell::RefMut;

/// Task control block structure
pub struct TaskControlBlock {
    /// immutable
    pub process: Weak<ProcessControlBlock>,
    /// Kernel stack corresponding to PID
    pub kstack: KernelStack,
    /// the only identifier of the task
    pub tid: usize,
    /// if as process pid == tid, else pid == tid of father process
    pub pid: usize,
    /// whether to send SIGCHLD when the task exits
    pub send_sigchld_when_exit: bool,
    /// mutable
    inner: UPSafeCell<TaskControlBlockInner>,
}
pub struct TaskControlBlockInner {
    /// The physical page number of the frame where the trap context is placed
    pub trap_cx_ppn: PhysPageNum,
    /// Save task context
    pub task_cx: TaskContext,
    /// Maintain the execution status of the current process
    pub task_status: TaskStatus,
    /// syscall times of tasks
    pub syscall_times: [u32; MAX_SYSCALL_NUM],
    /// the time task was first run
    pub first_time: Option<usize>, // todo: 封装为一个单独的TaskTimer结构体
    ///
    pub clear_child_tid: usize,
    /// working directory
    pub work_dir: Arc<OSInode>,
    /// father task control block
    pub parent: Option<Weak<TaskControlBlock>>,
    /// children task control block
    pub children: Vec<Arc<TaskControlBlock>>,
    /// user stack
    pub user_stack_top: usize,
    /// exit code
    pub exit_code: Option<i32>,
    /// file descriptor table
    pub fd_table: Vec<Option<Arc<dyn File + Send + Sync>>>,
    /// clock time stop watch
    pub clock_stop_watch: usize,
    /// user clock time
    pub user_clock: usize,
    /// kernel clock time
    pub kernel_clock: usize,
    /// Record the usage of heap_area in MemorySet
    pub heap_base: VirtAddr,
    ///
    pub heap_end: VirtAddr,
}

impl TaskControlBlock {
    /// Get the mutable reference of the inner TCB
    pub fn inner_exclusive_access(&self) -> RefMut<'_, TaskControlBlockInner> {
        self.inner.exclusive_access()
    }
    /// Get the address of app's page table
    pub fn get_user_token(&self) -> usize {
        let process = self.process.upgrade().unwrap();
        let inner = process.inner_exclusive_access();
        inner.memory_set.token()
    }
    /// 根据tid获取task的trap_cx位置
    pub fn get_trap_cx(&self) -> &'static mut TrapContext {
        self.trap_cx_user_va().get_mut()
    }

    pub fn gettid(&self) -> usize {
        self.tid
    }
}

impl TaskControlBlockInner {
    pub fn get_other_trap_cx(&self) -> &'static mut TrapContext {
        VirtAddr::from(TRAP_CONTEXT_TRAMPOLINE).floor().get_mut()
    }

    #[allow(unused)]
    fn get_status(&self) -> TaskStatus {
        self.task_status
    }
}

impl TaskControlBlock {
    /// Allocate user resource for a task
    fn alloc_initproc_res() {
        // todo: 封装new函数中的部分操作解耦合
    }

    /// The bottom usr vaddr (low addr) of the trap context for a task with tid
    pub fn trap_cx_user_va(&self) -> VirtAddr {
        trap_cx_bottom_from_tid(self.tid).into()
    }

    /// The physical page number(ppn) of the trap context for a task with tid
    pub fn trap_cx_ppn(&self) -> PhysPageNum {
        let process = self.process.upgrade().unwrap();
        let process_inner = process.inner_exclusive_access();
        let trap_cx_bottom_va: VirtAddr = trap_cx_bottom_from_tid(self.tid).into();
        debug!(
            "trap_cx_ppn = {:#x}",
            process_inner
                .memory_set
                .translate(trap_cx_bottom_va.into())
                .unwrap()
                .ppn()
                .0
        );
        process_inner
            .memory_set
            .translate(trap_cx_bottom_va.into())
            .unwrap()
            .ppn()
    }

    /// Create a new task
    pub fn new(
        process: Arc<ProcessControlBlock>,
        ustack_top: usize,
        kstack: KernelStack,
        alloc_user_res: bool,
    ) -> Self {
        let tid = process.inner_exclusive_access().alloc_tid();
        if alloc_user_res {
            // todo: 封装为alloc_initproc_res();
            let mut process_inner = process.inner_exclusive_access();
            // alloc user stack
            let ustack_bottom = ustack_top - USER_STACK_SIZE;
            debug!(
                "alloc_user_res: ustack_bottom={:#x} ustack_top={:#x}",
                ustack_bottom, ustack_top
            );
            process_inner.memory_set.insert_framed_area(
                ustack_bottom.into(),
                ustack_top.into(),
                MapPermission::R | MapPermission::W | MapPermission::U,
            );
            // alloc trap_cx
            let trap_cx_bottom = trap_cx_bottom_from_tid(tid);
            let trap_cx_top = trap_cx_bottom + PAGE_SIZE;
            debug!(
                "alloc_user_res: trap_cx_bottom={:#x} trap_cx_top={:#x}",
                trap_cx_bottom, trap_cx_top
            );
            process_inner.memory_set.insert_framed_area(
                trap_cx_bottom.into(),
                trap_cx_top.into(),
                MapPermission::R | MapPermission::W,
            );
            //将初始进程的trap_cx映射到当前初始化页表，确保可以在这个页表里写入，进入初始页表之后正常读取
            //后面其他进程之间的互相写入改用TRAP_CONTEXT_TRAMPOLINE
            //实现无栈协程之后就不用考虑进程之间互相映射了
            let trap_cx_bottom_va: VirtAddr = trap_cx_bottom.into();
            let trap_cx_bottom_ppn = process_inner
                .memory_set
                .translate(trap_cx_bottom_va.into())
                .unwrap()
                .ppn();
            let current_pagetable = &mut KERNEL_SPACE.exclusive_access().page_table;
            debug!(
                "map trap_cx in current pagetable trap_cx_bottom: {:#x}, trap_cx_bottom_ppn: {:#x}, page_table: {:#x}",
                trap_cx_bottom_va.0, trap_cx_bottom_ppn.0, current_pagetable.token()
            );
            current_pagetable.map(
                trap_cx_bottom_va.floor(),
                trap_cx_bottom_ppn,
                PTEFlags::from_bits((MapPermission::R | MapPermission::W).bits()).unwrap(),
            );
        }
        let trap_cx_ppn = {
            //todo: 封装为get_trap_cx_ppn()，解耦合
            let process_inner = process.inner_exclusive_access();
            let trap_cx_bottom_va: VirtAddr = trap_cx_bottom_from_tid(tid).into();
            debug!(
                "trap_cx_ppn = {:#x}",
                process_inner
                    .memory_set
                    .translate(trap_cx_bottom_va.into())
                    .unwrap()
                    .ppn()
                    .0
            );
            process_inner
                .memory_set
                .translate(trap_cx_bottom_va.into())
                .unwrap()
                .ppn()
        };
        // let kstack = kstack_alloc();
        let kstack_top = kstack.get_top();
        let process_inner = process.inner_exclusive_access();
        let work_dir = Arc::clone(&process_inner.work_dir);
        drop(process_inner);
        Self {
            process: Arc::downgrade(&process),
            kstack,
            tid: 0,                        //todo
            pid: 0,                        //todo
            send_sigchld_when_exit: false, //todo
            inner: unsafe {
                UPSafeCell::new(TaskControlBlockInner {
                    trap_cx_ppn,
                    task_cx: TaskContext::goto_user_entry(kstack_top),
                    task_status: TaskStatus::Ready,
                    exit_code: None,
                    syscall_times: [0; MAX_SYSCALL_NUM],
                    first_time: None,
                    work_dir,
                    clear_child_tid: 0,
                    parent: None,
                    children: Vec::new(),
                    user_stack_top: ustack_top, // todo
                    fd_table: vec![
                        // 0 -> stdin
                        Some(Arc::new(Stdin)),
                        // 1 -> stdout
                        Some(Arc::new(Stdout)),
                        // 2 -> stderr
                        Some(Arc::new(Stdout)),
                    ],
                    clock_stop_watch: 0,
                    user_clock: 0,
                    kernel_clock: 0,
                    heap_base: VirtAddr::from(0), // todo
                    heap_end: VirtAddr::from(0),  // todo
                })
            },
        }
    }
    /// Create a new init_task
    pub fn init_proc(
        process: Arc<ProcessControlBlock>,
        ustack_top: usize,
        kstack: KernelStack,
        alloc_user_res: bool,
    ) -> Self {
        info!("TaskControlBlock init_proc");
        let tid = process.inner_exclusive_access().alloc_tid();
        if alloc_user_res {
            // todo: 封装为alloc_initproc_res();
            let mut process_inner = process.inner_exclusive_access();
            // alloc user stack
            let ustack_bottom = ustack_top - USER_STACK_SIZE;
            debug!(
                "alloc_user_res: ustack_bottom={:#x} ustack_top={:#x}",
                ustack_bottom, ustack_top
            );
            process_inner.memory_set.insert_framed_area(
                ustack_bottom.into(),
                ustack_top.into(),
                MapPermission::R | MapPermission::W | MapPermission::U,
            );
            // alloc trap_cx
            let trap_cx_bottom = trap_cx_bottom_from_tid(tid);
            let trap_cx_top = trap_cx_bottom + PAGE_SIZE;
            debug!(
                "alloc_user_res: trap_cx_bottom={:#x} trap_cx_top={:#x}",
                trap_cx_bottom, trap_cx_top
            );
            process_inner.memory_set.insert_framed_area(
                trap_cx_bottom.into(),
                trap_cx_top.into(),
                MapPermission::R | MapPermission::W,
            );
            //将初始进程的trap_cx映射到当前初始化页表，确保可以在这个页表里写入，进入初始页表之后正常读取
            //后面其他进程之间的互相写入改用TRAP_CONTEXT_TRAMPOLINE
            //实现无栈协程之后就不用考虑进程之间互相映射了
            let trap_cx_bottom_va: VirtAddr = trap_cx_bottom.into();
            let trap_cx_bottom_ppn = process_inner
                .memory_set
                .translate(trap_cx_bottom_va.into())
                .unwrap()
                .ppn();
            let current_pagetable = &mut KERNEL_SPACE.exclusive_access().page_table;
            debug!(
                "map trap_cx in current pagetable trap_cx_bottom: {:#x}, trap_cx_bottom_ppn: {:#x}, page_table: {:#x}",
                trap_cx_bottom_va.0, trap_cx_bottom_ppn.0, current_pagetable.token()
            );
            current_pagetable.map(
                trap_cx_bottom_va.floor(),
                trap_cx_bottom_ppn,
                PTEFlags::from_bits((MapPermission::R | MapPermission::W).bits()).unwrap(),
            );
        }
        debug!("TaskUserRes allocated");
        let trap_cx_ppn = {
            //todo: 封装为get_trap_cx_ppn()，解耦合
            let process_inner = process.inner_exclusive_access();
            let trap_cx_bottom_va: VirtAddr = trap_cx_bottom_from_tid(tid).into();
            debug!(
                "trap_cx_ppn = {:#x}",
                process_inner
                    .memory_set
                    .translate(trap_cx_bottom_va.into())
                    .unwrap()
                    .ppn()
                    .0
            );
            process_inner
                .memory_set
                .translate(trap_cx_bottom_va.into())
                .unwrap()
                .ppn()
        };
        let kstack_top = kstack.get_top();
        let process_inner = process.inner_exclusive_access();
        let work_dir = Arc::clone(&process_inner.work_dir);
        drop(process_inner);
        Self {
            process: Arc::downgrade(&process),
            kstack,
            tid: 0,                        //todo
            pid: 0,                        //todo
            send_sigchld_when_exit: false, //todo
            inner: unsafe {
                UPSafeCell::new(TaskControlBlockInner {
                    trap_cx_ppn,
                    task_cx: TaskContext::goto_initproc_entry(kstack_top),
                    task_status: TaskStatus::Ready,
                    exit_code: None,
                    syscall_times: [0; MAX_SYSCALL_NUM],
                    first_time: None,
                    work_dir,
                    clear_child_tid: 0,
                    parent: None,
                    children: Vec::new(),
                    user_stack_top: ustack_top, // todo
                    fd_table: vec![
                        // 0 -> stdin
                        Some(Arc::new(Stdin)),
                        // 1 -> stdout
                        Some(Arc::new(Stdout)),
                        // 2 -> stderr
                        Some(Arc::new(Stdout)),
                    ],
                    clock_stop_watch: 0,
                    user_clock: 0,
                    kernel_clock: 0,
                    heap_base: VirtAddr::from(0), // todo
                    heap_end: VirtAddr::from(0),  // todo
                })
            },
        }
    }

    /// Deallocate user resource for a task
    fn dealloc_user_res(&self) {
        // dealloc tid
        let process = self.process.upgrade().unwrap();
        let mut process_inner = process.inner_exclusive_access();
        // dealloc ustack manually
        let ustack_top = self.inner_exclusive_access().user_stack_top;
        let ustack_bottom_va: VirtAddr = (ustack_top - USER_STACK_SIZE).into();
        process_inner
            .memory_set
            .remove_area_with_start_vpn(ustack_bottom_va.into());
        // dealloc trap_cx manually
        let trap_cx_bottom_va: VirtAddr = trap_cx_bottom_from_tid(self.tid).into();
        process_inner
            .memory_set
            .remove_area_with_start_vpn(trap_cx_bottom_va.into());
    }

    /// dealloc task id
    pub fn dealloc_tid(&self) {
        let process = self.process.upgrade().unwrap();
        let mut process_inner = process.inner_exclusive_access();
        process_inner.dealloc_tid(self.tid);
    }
}

#[derive(Copy, Clone, PartialEq)]
/// The execution status of the current process
pub enum TaskStatus {
    /// ready to run
    Ready,
    /// running
    Running,
    /// blocked, waiting
    Blocked,
    /// wait father process to release resources
    Zombie,
    /// exit
    Exit,
}

impl Drop for TaskControlBlock {
    fn drop(&mut self) {
        self.dealloc_user_res();
        self.dealloc_tid();
    }
}
