//! Types related to task management & Functions for completely changing TCB

use super::process::Flags;
use super::res::RecycleAllocator;
use super::{kstack_alloc, CloneFlags, KernelStack, PidHandle, SignalFlags, TaskContext};
use crate::config::{
    __breakpoint, BIG_STRIDE, MAX_SYSCALL_NUM, PAGE_SIZE, TRAP_CONTEXT_TRAMPOLINE, USER_STACK_SIZE,
};
use crate::fs::file::File;
use crate::fs::inode::{OSInode, ROOT_INODE};
use crate::fs::{Stdin, Stdout};
use crate::mm::{MapPermission, MemorySet, PTEFlags, VirtAddr, KERNEL_SPACE};
use crate::syscall::errno::EPERM;
use crate::task::manager::insert_into_pid2process;
use crate::task::res::{kernel_stack_position, trap_cx_bottom_from_tid};
use crate::task::{add_task, pid2process, pid_alloc};
use crate::timer::get_time;
use crate::trap::{trap_handler, TrapContext};
use crate::{mm::PhysPageNum, sync::UPSafeCell};
use alloc::string::String;
use alloc::sync::{Arc, Weak};
use alloc::vec;
use alloc::vec::Vec;
use core::cell::RefMut;
use core::{mem, slice, task};
use riscv::register::sstatus::set_mxr;
use riscv::register::{mstatus, sstatus};

/// Task control block structure
pub struct TaskControlBlock {
    /// immutable
    /// Kernel stack corresponding to PID
    pub kstack: KernelStack,
    /// thread id，作为进程时 pid == tid；作为线程时，tid 为其线程组 leader (父进程)的 pid 号。
    pub tid: usize,
    /// process id, the only identifier of the tasks
    pub pid: PidHandle,
    /// whether to send SIGCHLD when the task exits
    pub send_sigchld_when_exit: bool,
    /// mutable
    inner: UPSafeCell<TaskControlBlockInner>,
}
pub struct TaskControlBlockInner {
    /// memory set(address space)
    pub memory_set: MemorySet,
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
    /// thread group
    pub threads: Vec<Option<Arc<TaskControlBlock>>>,
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
    /// is zombie?
    pub is_zombie: bool,
    /// signal flags
    pub signals: SignalFlags,
}

impl TaskControlBlock {
    /// Get the mutable reference of the inner TCB
    pub fn inner_exclusive_access(
        &self,
        file: &'static str,
        line: u32,
    ) -> RefMut<'_, TaskControlBlockInner> {
        self.inner.exclusive_access(file, line)
    }
    /// Get the address of app's page table
    pub fn get_user_token(&self) -> usize {
        let inner = self.inner_exclusive_access(file!(), line!());
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

impl TaskControlBlock {
    /// Allocate user resource for a task
    fn alloc_initproc_res() {
        // todo: 封装new函数中的部分操作解耦合
    }

    /// The bottom usr vaddr (low addr) of the trap context for a task with tid
    pub fn trap_cx_user_va(&self) -> VirtAddr {
        trap_cx_bottom_from_tid(self.pid.0).into()
    }

    /// The physical page number(ppn) of the trap context for a task with tid
    pub fn trap_cx_ppn(&self) -> PhysPageNum {
        let task_inner = self.inner_exclusive_access(file!(), line!());
        let trap_cx_bottom_va: VirtAddr = trap_cx_bottom_from_tid(self.tid).into();
        debug!(
            "trap_cx_ppn = {:#x}",
            task_inner
                .memory_set
                .translate(trap_cx_bottom_va.into())
                .unwrap()
                .ppn()
                .0
        );
        task_inner
            .memory_set
            .translate(trap_cx_bottom_va.into())
            .unwrap()
            .ppn()
    }

    /// 从零开始创建一个新进程，只会在创建初始进程的时候使用一次
    pub fn init_task(elf_data: &[u8]) -> Arc<Self> {
        trace!("TaskControlBlock new");
        let kstack = kstack_alloc();
        let (mut memory_set, user_heap_base, ustack_top, entry_point) =
            MemorySet::from_elf(elf_data);
        let pid_handle = pid_alloc();
        let tid = pid_handle.0;

        // todo: 封装为alloc_initproc_res();
        // alloc user stack
        let ustack_bottom = ustack_top - USER_STACK_SIZE;
        debug!(
            "alloc_user_res: ustack_bottom={:#x} ustack_top={:#x}",
            ustack_bottom, ustack_top
        );
        memory_set.insert_framed_area(
            ustack_bottom.into(),
            ustack_top.into(),
            MapPermission::R | MapPermission::W | MapPermission::U,
        );
        // alloc trap_cx
        let trap_cx_bottom = trap_cx_bottom_from_tid(pid_handle.0);
        let trap_cx_top = trap_cx_bottom + PAGE_SIZE;
        debug!(
            "alloc_user_res: trap_cx_bottom={:#x} trap_cx_top={:#x}",
            trap_cx_bottom, trap_cx_top
        );
        memory_set.insert_framed_area(
            trap_cx_bottom.into(),
            trap_cx_top.into(),
            MapPermission::R | MapPermission::W,
        );
        //将初始进程的trap_cx映射到当前初始化页表，确保可以在这个页表里写入，进入初始页表之后正常读取
        //后面其他进程之间的互相写入改用TRAP_CONTEXT_TRAMPOLINE
        //实现无栈协程之后就不用考虑进程之间互相映射了
        let trap_cx_bottom_va: VirtAddr = trap_cx_bottom.into();
        let trap_cx_bottom_ppn = memory_set
            .translate(trap_cx_bottom_va.into())
            .unwrap()
            .ppn();

        {
            // 在一定区域中获取可变引用，保证离开时自动释放
            let current_pagetable = &mut KERNEL_SPACE.exclusive_access(file!(), line!()).page_table;
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
            debug!(
                "trap_cx_ppn = {:#x}",
                memory_set
                    .translate(trap_cx_bottom_va.into())
                    .unwrap()
                    .ppn()
                    .0
            );
            memory_set
                .translate(trap_cx_bottom_va.into())
                .unwrap()
                .ppn()
        };
        // let kstack = kstack_alloc();
        let kstack_top = kstack.get_top();
        let work_dir = ROOT_INODE.clone();
        let task = Arc::new(Self {
            kstack,
            tid: tid,
            pid: pid_handle,
            send_sigchld_when_exit: false, //todo
            inner: unsafe {
                UPSafeCell::new(TaskControlBlockInner {
                    is_zombie: false,
                    memory_set,
                    trap_cx_ppn,
                    task_cx: TaskContext::goto_initproc_entry(kstack_top),
                    task_status: TaskStatus::Ready,
                    exit_code: None,
                    syscall_times: [0; MAX_SYSCALL_NUM],
                    first_time: None,
                    clear_child_tid: 0,
                    parent: None,
                    children: Vec::new(),
                    threads: Vec::new(),
                    user_stack_top: ustack_top, // todo
                    fd_table: vec![
                        // 0 -> stdin
                        Some(Arc::new(Stdin)),
                        // 1 -> stdout
                        Some(Arc::new(Stdout)),
                        // 2 -> stderr
                        Some(Arc::new(Stdout)),
                    ],
                    signals: SignalFlags::empty(),
                    clock_stop_watch: 0,
                    user_clock: 0,
                    kernel_clock: 0,
                    heap_base: user_heap_base.into(),
                    heap_end: user_heap_base.into(),
                    work_dir,
                })
            },
        });
        let task_inner = task.inner_exclusive_access(file!(), line!());
        let trap_cx = task.get_trap_cx();
        let ustack_top = task_inner.user_stack_top;
        let kstack_top = task.kstack.get_top();
        drop(task_inner);
        debug!("TrapContext::app_init_context");
        // debug!("*trap_cx = {:?}", *trap_cx);
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            ustack_top,
            KERNEL_SPACE.exclusive_access(file!(), line!()).token(),
            kstack_top,
            trap_handler as usize,
        );
        // add initproc
        add_task(task.clone());

        let test_va: &mut usize =
            VirtAddr::from(kernel_stack_position(task.pid.0).0 as usize).get_mut();
        warn!("test_va: {:#x}", kernel_stack_position(task.pid.0).0);
        warn!("*test_va: {:#x}", *test_va);

        task
    }

    ///
    pub fn clone_t(
        self: &Arc<Self>,
        flag: CloneFlags,
        stack: usize,
        sig: SignalFlags,
        ptid: usize,
        tls: usize,
        ctid: usize,
    ) -> Option<Arc<TaskControlBlock>> {
        warn!(
            "clone: flag:{:?}, sig:{:?}, stack:{:#x}, ptid:{:#x}, tls:{:#x}, ctid:{:#x}",
            flag, sig, stack, ptid, tls, ctid
        );
        let pid = pid_alloc();
        let task_inner = self.inner_exclusive_access(file!(), line!());
        let memory_set = if flag.contains(CloneFlags::CLONE_VM) {
            MemorySet::from_existed_user(&task_inner.memory_set)
        } else {
            MemorySet::from_existed_user(&task_inner.memory_set) //todo: 改为Flag对应要求
        };

        // copy fd table
        let fd_table = if flag.contains(CloneFlags::CLONE_FILES) {
            // todo: 实现clone trait，这样就可以直接clone父进程的，解耦合
            let mut new_fd_table: Vec<Option<Arc<dyn File + Send + Sync>>> = Vec::new();
            for fd in task_inner.fd_table.iter() {
                if let Some(file) = fd {
                    new_fd_table.push(Some(file.clone()));
                } else {
                    new_fd_table.push(None);
                }
            }
            new_fd_table
        } else {
            let new_fd_table: Vec<Option<Arc<dyn File + Send + Sync>>> = vec![
                // 0 -> stdin
                Some(Arc::new(Stdin)),
                // 1 -> stdout
                Some(Arc::new(Stdout)),
                // 2 -> stderr
                Some(Arc::new(Stdout)),
            ];
            new_fd_table
        };

        let tid = if flag.contains(CloneFlags::CLONE_THREAD) {
            self.tid
        } else {
            pid.0
        };

        let parent = if flag.contains(CloneFlags::CLONE_PARENT) {
            task_inner.parent.clone()
        } else {
            Some(Arc::downgrade(self))
        };

        let kstask = kstack_alloc();
        let kstack_top = kstask.get_top();

        // map the thread trap_context if clone_vm
        // let trap_context = if flag.contains(CloneFlags::CLONE_VM) {
        //     todo!("should alloc a new trap_context for the new thread according to thread id");
        // } else {
        //     child_task.get_trap_cx()
        // };

        // insert_into_pid2process(pid, Arc::clone(child_task));

        todo!("unfinished");
    }

    pub fn fork(self: &Arc<Self>) -> usize {
        trace!("[kernel]: sys_fork");
        let pid = pid_alloc();
        warn!("fork: pid[{}]", pid.0);
        let trap_cx_ppn = self.trap_cx_ppn();

        let mut task_inner = self.inner_exclusive_access(file!(), line!());
        let kstack = kstack_alloc();
        let kstack_top = kstack.get_top();
        let mut memory_set = MemorySet::from_existed_user(&task_inner.memory_set);

        let tid = pid.0;
        let parent = Some(Arc::downgrade(self));
        // copy fd table
        let mut new_fd_table: Vec<Option<Arc<dyn File + Send + Sync>>> = Vec::new();
        for fd in task_inner.fd_table.iter() {
            if let Some(file) = fd {
                new_fd_table.push(Some(file.clone()));
            } else {
                new_fd_table.push(None);
            }
        }

        // 为新进程分配中断上下文
        // 现在获取中断上下文靠pid的划分，这其实不太合适，应该在线程组内部按照线程id区分
        // 但是由于pid是全局唯一的，所以进程之间互相映射或者访问中断上下文的时候就可以不用 TRAP_CONTEXT_TRAMPOLINE了
        // alloc trap_cx
        let trap_cx_bottom = trap_cx_bottom_from_tid(pid.0);
        let trap_cx_top = trap_cx_bottom + PAGE_SIZE;
        debug!(
            "alloc_user_res: trap_cx_bottom={:#x} trap_cx_top={:#x}",
            trap_cx_bottom, trap_cx_top
        );
        memory_set.insert_framed_area(
            trap_cx_bottom.into(),
            trap_cx_top.into(),
            MapPermission::R | MapPermission::W,
        );

        //将初始进程的trap_cx映射到当前页表，确保可以在这个页表里写入
        //实现无栈协程之后就不用考虑进程之间互相映射了
        // 注意这里只复制了pte，没有复制物理页帧
        let trap_cx_bottom_va: VirtAddr = trap_cx_bottom.into();
        let trap_cx_bottom_ppn = memory_set
            .translate(trap_cx_bottom_va.into())
            .unwrap()
            .ppn();

        {
            // 在一定区域中获取可变引用，保证离开时自动释放
            let current_pagetable = &mut task_inner.memory_set.page_table;
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

        let child_task = Arc::new(TaskControlBlock {
            kstack,
            tid,
            pid,
            send_sigchld_when_exit: false,
            inner: unsafe {
                UPSafeCell::new(TaskControlBlockInner {
                    is_zombie: false,
                    memory_set,
                    trap_cx_ppn,
                    task_cx: TaskContext::goto_user_entry(kstack_top),
                    task_status: TaskStatus::Ready,
                    exit_code: None,
                    syscall_times: [0; MAX_SYSCALL_NUM],
                    first_time: None,
                    clear_child_tid: 0,
                    parent,
                    children: Vec::new(),
                    threads: Vec::new(),
                    user_stack_top: task_inner.user_stack_top,
                    fd_table: new_fd_table,
                    signals: SignalFlags::empty(),
                    clock_stop_watch: 0,
                    user_clock: 0,
                    kernel_clock: 0,
                    heap_base: task_inner.heap_base.clone(),
                    heap_end: task_inner.heap_end.clone(),
                    work_dir: task_inner.work_dir.clone(),
                })
            },
        });

        task_inner.children.push(Arc::clone(&child_task));
        // 这里复制父进程中断上下文，确保接下来能正确切换到子进程
        let father_trap_cx = self.get_trap_cx();
        let trap_cx = child_task.get_trap_cx();
        let src_ptr = father_trap_cx as *const TrapContext;
        let dst_ptr = trap_cx as *mut TrapContext;
        debug!(
            "copy trap_cx from {:#x} to {:#x}",
            src_ptr as usize, dst_ptr as usize
        );

        unsafe {
            core::ptr::copy(
                src_ptr,
                dst_ptr,
                PAGE_SIZE / core::mem::size_of::<TrapContext>(),
            );
        }

        // fork出的子进程应该返回0
        trap_cx.x[10] = 0;
        let pid = child_task.pid.0.clone();
        insert_into_pid2process(pid, Arc::clone(&child_task));
        // add this thread to scheduler
        add_task(child_task);
        info!("fork: child pid[{}] add to scheduler", pid);

        pid
    }

    /// clone2
    pub fn clone2(
        self: &Arc<Self>,
        _exit_signals: SignalFlags,
        _clone_signals: CloneFlags,
        stack_ptr: usize,
        tls: usize,
    ) -> Arc<TaskControlBlock> {
        trace!("kernel: clone thread");
        let pid = pid_alloc();
        let mut father_inner = self.inner_exclusive_access(file!(), line!());

        // create a new thread.
        // We did not alloc for stack space here
        // 不需要分配用户栈，只分配内核栈和中断上下文
        let thread_stack_top = if stack_ptr != 0 {
            stack_ptr
        } else {
            self.inner_exclusive_access(file!(), line!()).user_stack_top
        };
        //这里是线程，所以tid = 父进程pid
        let tid = self.pid.0;
        let kstack = kstack_alloc();
        let kstack_top = kstack.get_top();
        let trap_cx_bottom = trap_cx_bottom_from_tid(pid.0);
        let trap_cx_top = trap_cx_bottom + PAGE_SIZE;
        let trap_cx_ppn = father_inner
            .memory_set
            .page_table
            .translate(trap_cx_bottom.into())
            .unwrap()
            .ppn();

        // 把子线程的中断上下文映射到当前地址空间
        father_inner.memory_set.insert_framed_area(
            trap_cx_bottom.into(),
            trap_cx_top.into(),
            MapPermission::R | MapPermission::W,
        );

        let memory_set = MemorySet::from_existed_user(&father_inner.memory_set);
        let new_task = Arc::new(Self {
            kstack,
            tid: tid,
            pid: pid,
            send_sigchld_when_exit: false, //todo
            inner: unsafe {
                UPSafeCell::new(TaskControlBlockInner {
                    is_zombie: false,
                    memory_set,
                    trap_cx_ppn,
                    task_cx: TaskContext::goto_user_entry(kstack_top),
                    task_status: TaskStatus::Ready,
                    exit_code: None,
                    syscall_times: [0; MAX_SYSCALL_NUM],
                    first_time: None,
                    clear_child_tid: 0,
                    parent: None,
                    children: Vec::new(),
                    threads: Vec::new(),
                    user_stack_top: thread_stack_top, // todo
                    fd_table: vec![
                        // 0 -> stdin
                        Some(Arc::new(Stdin)),
                        // 1 -> stdout
                        Some(Arc::new(Stdout)),
                        // 2 -> stderr
                        Some(Arc::new(Stdout)),
                    ],
                    signals: SignalFlags::empty(),
                    clock_stop_watch: 0,
                    user_clock: 0,
                    kernel_clock: 0,
                    heap_base: father_inner.heap_base.clone(), //todo 这里存在一个疑问，即共享堆空间，子线程修改堆空间后如何及时更新线程组下其他
                    heap_end: father_inner.heap_end.clone(), //todo  的线程包括主线程，以及地址空间的修改也需要同步，后续需要修改为线程组使用同一个对象，暂时先别用线程
                    work_dir: father_inner.work_dir.clone(),
                })
            },
        });

        father_inner.threads.push(Some(Arc::clone(&new_task)));

        let trap_cx_ptr = new_task.get_trap_cx();

        // I don't know if this is correct, too
        *trap_cx_ptr = *self.get_trap_cx();

        // for child process, fork returns 0
        trap_cx_ptr.x[10] = 0;
        // set tp reg
        trap_cx_ptr.x[4] = tls;
        // set sp reg
        trap_cx_ptr.set_sp(
            new_task
                .inner_exclusive_access(file!(), line!())
                .user_stack_top,
        );
        // modify kernel_sp in trap_cx
        trap_cx_ptr.kernel_sp = new_task.kstack.get_top();

        // add new task to scheduler
        add_task(Arc::clone(&new_task));

        new_task
    }

    /// Only support processes with a single thread or self as the main thread
    pub fn exec(self: &Arc<Self>, elf_data: &[u8], args: Vec<String>) {
        trace!("[kernel: exec]");
        assert_eq!(self.pid.0, self.tid);
        // memory_set with elf program headers/trampoline/trap context/user stack
        trace!("[kernel: exec] .. MemorySet::from_elf");
        let (mut memory_set, user_heap_base, ustack_top, entry_point) =
            MemorySet::from_elf(elf_data);
        // substitute memory_set
        // set heap position
        self.inner_exclusive_access(file!(), line!()).heap_base = user_heap_base.into();
        self.inner_exclusive_access(file!(), line!()).heap_end = user_heap_base.into();
        // then we alloc user resource for main thread again
        // since memory_set has been changed
        trace!("[kernel: exec] .. alloc user resource for main thread again");
        let mut task_inner = self.inner_exclusive_access(file!(), line!());
        task_inner.user_stack_top = ustack_top;

        // 为新地址空间分配用户栈和trap_cx
        //trap_cx由于虚拟地址按照pid划分，所以要把映射复制过来
        let ustack_top = task_inner.user_stack_top;
        let ustack_bottom = ustack_top - USER_STACK_SIZE;
        debug!(
            "[kernel: exec] alloc user stack ustack_bottom={:#x} ustack_top={:#x}",
            ustack_bottom, ustack_top
        );
        memory_set.insert_framed_area(
            ustack_bottom.into(),
            ustack_top.into(),
            MapPermission::R | MapPermission::W | MapPermission::U,
        );

        // let user_trap_va: VirtAddr = trap_cx_bottom_from_tid(self.pid.0).into();
        // let user_trap_ppn = task_inner
        //     .memory_set
        //     .translate(user_trap_va.floor())
        //     .unwrap()
        //     .ppn();
        // info!(
        //     "[kernel: exec] map trap_cx in new memory_set trap_cx_bottom: {:#x}, trap_cx_bottom_ppn: {:#x}, page_table: {:#x}",
        //     user_trap_va.0, user_trap_ppn.0, memory_set.page_table.token()
        // );
        // memory_set.page_table.map(
        //     user_trap_va.floor(),
        //     user_trap_ppn,
        //     PTEFlags::from_bits((MapPermission::R | MapPermission::W).bits()).unwrap(),
        // );

        // 替换为新的地址空间
        debug!(
            "[kernel: exec] replace memory_set with new one, old: {:#x}, new: {:#x} 
            will dealloc old memory_set here",
            task_inner.memory_set.token(),
            memory_set.token()
        );
        task_inner.memory_set = memory_set; // todo dealloc page here

        // push arguments on user stack
        trace!("[kernel: exec] .. push arguments on user stack");
        let mut user_sp = task_inner.user_stack_top;

        // Enable kernel to visit user space
        unsafe {
            sstatus::set_sum(); //todo Use RAII
        }

        // argv is a vector of each arg's addr
        let mut argv = vec![0; args.len()];

        user_sp -= (args.len() + 1) * core::mem::size_of::<usize>();
        let argv_base = user_sp;
        for i in 0..args.len() {
            unsafe {
                *((argv_base + i * core::mem::size_of::<usize>()) as *mut usize) = argv[i];
            }
        }
        unsafe {
            *((argv_base + args.len() * core::mem::size_of::<usize>()) as *mut usize) = 0;
        }

        // Copy each arg to the newly allocated stack
        for i in 0..args.len() {
            // Here we leave one byte to store a '\0' as a terminator
            user_sp -= args[i].len() + 1;
            let p = user_sp as *mut u8;
            unsafe {
                argv[i] = user_sp;
                p.copy_from(args[i].as_ptr(), args[i].len());
                *((p as usize + args[i].len()) as *mut u8) = 0;
            }
        }
        user_sp -= user_sp % core::mem::size_of::<usize>();

        // // Disable kernel to visit user space
        // unsafe {
        //     sstatus::clear_sum(); //todo Use RAII
        // }

        unsafe {
            warn!("entry: {:#x}", entry_point);
        }
        debug!("sstatus before: {:#x?}", sstatus::read().spp());
        // initialize trap_cx
        trace!("[kernel: exec] .. initialize trap_cx for new process");
        let mut trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access(file!(), line!()).token(),
            self.kstack.get_top(),
            trap_handler as usize,
        );
        trap_cx.x[10] = args.len();
        trap_cx.x[11] = argv_base;

        // 获取 *mut TrapContext 的指针
        let trap_cx_ptr: *mut TrapContext = &mut trap_cx;

        // 计算 TrapContext 的大小
        let size_of_trap_context = core::mem::size_of::<TrapContext>();

        // 将 *mut TrapContext 转换为 &[u8]
        let trap_cx_bytes: &[u8] =
            unsafe { slice::from_raw_parts(trap_cx_ptr as *const u8, size_of_trap_context) };

        // alloc trap_cx
        // 为新进程重新分配中断上下文，原来的地址空间被覆盖掉之后，所有页都会被回收
        let trap_cx_bottom = trap_cx_bottom_from_tid(self.pid.0);
        let trap_cx_top = trap_cx_bottom + PAGE_SIZE;
        debug!(
            "alloc trap_cx again: trap_cx_bottom={:#x} trap_cx_top={:#x}",
            trap_cx_bottom, trap_cx_top
        );
        task_inner.memory_set.insert_framed_area_with_data(
            trap_cx_bottom.into(),
            trap_cx_top.into(),
            MapPermission::R | MapPermission::W,
            trap_cx_bytes,
        );

        // 重新设置被调度后的跳转地址以切换地址空间
        task_inner.task_cx = TaskContext::goto_user_entry(self.kstack.get_top());

        // let entry = VirtAddr::from(0x1000 as usize).get_mut() as *mut i32;
        // unsafe {
        //     warn!("entry inst:{:#x?}", *entry);
        // }

        *self.get_trap_cx() = trap_cx;

        __breakpoint();
    }

    // /// Create a new init_task
    // pub fn init_proc(
    //     process: Arc<ProcessControlBlock>,
    //     ustack_top: usize,
    //     kstack: KernelStack,
    //     alloc_user_res: bool,
    // ) -> Self {
    //     info!("TaskControlBlock init_proc");
    //     let tid = process.inner_exclusive_access(file!(), line!()).alloc_tid();
    //     if alloc_user_res {
    //         // todo: 封装为alloc_initproc_res();
    //         let mut process_inner = process.inner_exclusive_access(file!(), line!());
    //         // alloc user stack
    //         let ustack_bottom = ustack_top - USER_STACK_SIZE;
    //         debug!(
    //             "alloc_user_res: ustack_bottom={:#x} ustack_top={:#x}",
    //             ustack_bottom, ustack_top
    //         );
    //         process_inner.memory_set.insert_framed_area(
    //             ustack_bottom.into(),
    //             ustack_top.into(),
    //             MapPermission::R | MapPermission::W | MapPermission::U,
    //         );
    //         // alloc trap_cx
    //         let trap_cx_bottom = trap_cx_bottom_from_tid(tid);
    //         let trap_cx_top = trap_cx_bottom + PAGE_SIZE;
    //         debug!(
    //             "alloc_user_res: trap_cx_bottom={:#x} trap_cx_top={:#x}",
    //             trap_cx_bottom, trap_cx_top
    //         );
    //         process_inner.memory_set.insert_framed_area(
    //             trap_cx_bottom.into(),
    //             trap_cx_top.into(),
    //             MapPermission::R | MapPermission::W,
    //         );
    //         //将初始进程的trap_cx映射到当前初始化页表，确保可以在这个页表里写入，进入初始页表之后正常读取
    //         //后面其他进程之间的互相写入改用TRAP_CONTEXT_TRAMPOLINE
    //         //实现无栈协程之后就不用考虑进程之间互相映射了
    //         let trap_cx_bottom_va: VirtAddr = trap_cx_bottom.into();
    //         let trap_cx_bottom_ppn = process_inner
    //             .memory_set
    //             .translate(trap_cx_bottom_va.into())
    //             .unwrap()
    //             .ppn();
    //         let current_pagetable = &mut KERNEL_SPACE.exclusive_access(file!(), line!()).page_table;
    //         debug!(
    //             "map trap_cx in current pagetable trap_cx_bottom: {:#x}, trap_cx_bottom_ppn: {:#x}, page_table: {:#x}",
    //             trap_cx_bottom_va.0, trap_cx_bottom_ppn.0, current_pagetable.token()
    //         );
    //         current_pagetable.map(
    //             trap_cx_bottom_va.floor(),
    //             trap_cx_bottom_ppn,
    //             PTEFlags::from_bits((MapPermission::R | MapPermission::W).bits()).unwrap(),
    //         );
    //     }
    //     debug!("TaskUserRes allocated");
    //     let trap_cx_ppn = {
    //         //todo: 封装为get_trap_cx_ppn()，解耦合
    //         let process_inner = process.inner_exclusive_access(file!(), line!());
    //         let trap_cx_bottom_va: VirtAddr = trap_cx_bottom_from_tid(tid).into();
    //         debug!(
    //             "trap_cx_ppn = {:#x}",
    //             process_inner
    //                 .memory_set
    //                 .translate(trap_cx_bottom_va.into())
    //                 .unwrap()
    //                 .ppn()
    //                 .0
    //         );
    //         process_inner
    //             .memory_set
    //             .translate(trap_cx_bottom_va.into())
    //             .unwrap()
    //             .ppn()
    //     };
    //     let kstack_top = kstack.get_top();
    //     let process_inner = process.inner_exclusive_access(file!(), line!());
    //     let work_dir = Arc::clone(&process_inner.work_dir);
    //     drop(process_inner);
    //     Self {
    //         process: Arc::downgrade(&process),
    //         kstack,
    //         tid: 0,                        //todo
    //         pid: 0,                        //todo
    //         send_sigchld_when_exit: false, //todo
    //         inner: unsafe {
    //             UPSafeCell::new(TaskControlBlockInner {
    //                 trap_cx_ppn,
    //                 task_cx: TaskContext::goto_initproc_entry(kstack_top),
    //                 task_status: TaskStatus::Ready,
    //                 exit_code: None,
    //                 syscall_times: [0; MAX_SYSCALL_NUM],
    //                 first_time: None,
    //                 work_dir,
    //                 clear_child_tid: 0,
    //                 parent: None,
    //                 children: Vec::new(),
    //                 user_stack_top: ustack_top, // todo
    //                 fd_table: vec![
    //                     // 0 -> stdin
    //                     Some(Arc::new(Stdin)),
    //                     // 1 -> stdout
    //                     Some(Arc::new(Stdout)),
    //                     // 2 -> stderr
    //                     Some(Arc::new(Stdout)),
    //                 ],
    //                 clock_stop_watch: 0,
    //                 user_clock: 0,
    //                 kernel_clock: 0,
    //                 heap_base: VirtAddr::from(0), // todo
    //                 heap_end: VirtAddr::from(0),  // todo
    //             })
    //         },
    //     }
    // }

    /// Deallocate user resource for a task
    fn dealloc_user_res(&self) {
        // dealloc tid
        let mut task_inner = self.inner_exclusive_access(file!(), line!());
        // dealloc ustack manually
        let ustack_top = self.inner_exclusive_access(file!(), line!()).user_stack_top;
        let ustack_bottom_va: VirtAddr = (ustack_top - USER_STACK_SIZE).into();
        task_inner
            .memory_set
            .remove_area_with_start_vpn(ustack_bottom_va.into());
        // dealloc trap_cx manually
        let trap_cx_bottom_va: VirtAddr = trap_cx_bottom_from_tid(self.tid).into();
        task_inner
            .memory_set
            .remove_area_with_start_vpn(trap_cx_bottom_va.into());
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

    #[allow(unused)]
    /// get the address of app's page table
    pub fn get_user_token(&self) -> usize {
        self.memory_set.token()
    }
    /// allocate a new file descriptor
    pub fn alloc_fd(&mut self) -> usize {
        if let Some(fd) = (0..self.fd_table.len()).find(|fd| self.fd_table[*fd].is_none()) {
            fd
        } else {
            self.fd_table.push(None);
            self.fd_table.len() - 1
        }
    }

    /// the count of tasks(threads) in this process
    pub fn thread_count(&self) -> usize {
        self.children.len() //todo 需要完善，目前只是计数子进程，没有维护线程计数，函数本身暂时没什么用
    }

    /// count clock time
    pub fn clock_time_refresh(&mut self) {
        self.clock_stop_watch = get_time();
    }
    /// count user clock time and start to count kernel clock time
    pub fn user_clock_time_end(&mut self) -> usize {
        let last_stop = self.clock_stop_watch;
        self.clock_stop_watch = get_time();
        self.user_clock += self.clock_stop_watch - last_stop;
        self.user_clock
    }
    /// count kernel clock time and start to count user clock time
    pub fn user_clock_time_start(&mut self) -> usize {
        let last_stop = self.clock_stop_watch;
        self.clock_stop_watch = get_time();
        self.kernel_clock += self.clock_stop_watch - last_stop;
        self.kernel_clock
    }
    /// get clock time
    pub fn get_process_clock_time(&mut self) -> (i64, i64) {
        let last_stop = self.clock_stop_watch;
        self.clock_stop_watch = get_time();
        self.kernel_clock += self.clock_stop_watch - last_stop;
        (self.kernel_clock as i64, self.user_clock as i64)
    }
    /// get children's clock time
    pub fn get_children_process_clock_time(&self) -> (i64, i64) {
        let mut children_kernel_clock: usize = 0;
        let mut children_user_clock: usize = 0;
        for c in &self.children {
            children_kernel_clock += c.inner_exclusive_access(file!(), line!()).kernel_clock;
            children_user_clock += c.inner_exclusive_access(file!(), line!()).user_clock;
        }
        (children_kernel_clock as i64, children_user_clock as i64)
    }

    /// mmap
    pub fn mmap(
        &mut self,
        start_addr: usize,
        len: usize,
        _prot: usize,
        flags: usize,
        fd: usize,
        offset: usize,
    ) -> isize {
        let flags = Flags::from_bits(flags as u32).unwrap();
        let file = self.fd_table[fd].clone().unwrap();
        let file = unsafe { &*(file.as_ref() as *const dyn File as *const OSInode) };
        let (context, length) = if flags.contains(Flags::MAP_ANONYMOUS) {
            (Vec::new(), len)
        } else {
            debug!("mmap: file name: {}", file.name().unwrap());
            let context = file.read_all();

            let file_len = context.len();
            let length = len.min(file_len - offset);
            if file_len <= offset {
                debug!(
                    "mmap ERROR: offset exceeds file length context.len(): {}, offset: {}",
                    file_len, offset
                );
                return EPERM;
            };
            (context, length)
        };

        self.memory_set
            .mmap(start_addr, length, offset, context, flags)
    }

    ///munmap
    pub fn munmap(&mut self, start_addr: usize, len: usize) -> isize {
        self.memory_set.munmap(start_addr, len)
    }
}
