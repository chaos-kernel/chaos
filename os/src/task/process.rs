//! Implementation of  [`ProcessControlBlock`]

use super::id::RecycleAllocator;
use super::manager::insert_into_pid2process;
use super::{add_task, SignalFlags};
use super::{current_task, TaskControlBlock};
use super::{pid_alloc, PidHandle};
use crate::fs::file::File;
use crate::fs::inode::{OSInode, ROOT_INODE};
use crate::fs::{Stdin, Stdout};
use crate::mm::{translated_refmut, MemorySet, VirtAddr, KERNEL_SPACE};
use crate::sync::UPSafeCell;
use crate::syscall::errno::EPERM;
use crate::timer::get_time;
use crate::trap::{trap_handler, TrapContext};
use alloc::string::String;
use alloc::sync::{Arc, Weak};
use alloc::vec;
use alloc::vec::Vec;
use core::cell::RefMut;

#[allow(unused)]
#[allow(missing_docs)]
pub const CSIGNAL: usize = 0x000000ff; /* signal mask to be sent at exit */
bitflags! {
    ///
    pub struct CloneFlags: u32 {
        ///set if VM shared between processes
        const CLONE_VM	            = 0x00000100;
        ///set if fs info shared between processes
        const CLONE_FS	            = 0x00000200;
        ///set if open files shared between processes
        const CLONE_FILES	        = 0x00000400;
        ///set if signal handlers and blocked signals shared
        const CLONE_SIGHAND	        = 0x00000800;
        ///set if a pidfd should be placed in parent
        const CLONE_PIDFD	        = 0x00001000;
        ///set if we want to let tracing continue on the child too
        const CLONE_PTRACE	        = 0x00002000;
        ///set if the parent wants the child to wake it up on mm_release
        const CLONE_VFORK	        = 0x00004000;
        ///set if we want to have the same parent as the cloner
        const CLONE_PARENT	        = 0x00008000;
        ///Same thread group?
        const CLONE_THREAD	        = 0x00010000;
        ///New mount namespace group
        const CLONE_NEWNS	        = 0x00020000;
        ///share system V SEM_UNDO semantics
        const CLONE_SYSVSEM	        = 0x00040000;
        ///create a new TLS for the child
        const CLONE_SETTLS	        = 0x00080000;
        ///set the TID in the parent
        const CLONE_PARENT_SETTID	= 0x00100000;
        ///clear the TID in the child
        const CLONE_CHILD_CLEARTID	= 0x00200000;
        ///Unused, ignored
        const CLONE_DETACHED		= 0x00400000;
        ///set if the tracing process can't force CLONE_PTRACE on this clone
        const CLONE_UNTRACED		= 0x00800000;
        ///set the TID in the child
        const CLONE_CHILD_SETTID	= 0x01000000;
        ///New cgroup namespace
        const CLONE_NEWCGROUP		= 0x02000000;
        ///New utsname namespace
        const CLONE_NEWUTS		    = 0x04000000;
        ///New ipc namespace
        const CLONE_NEWIPC		    = 0x08000000;
        ///New user namespace
        const CLONE_NEWUSER		    = 0x10000000;
        ///New pid namespace
        const CLONE_NEWPID		    = 0x20000000;
        ///New network namespace
        const CLONE_NEWNET		    = 0x40000000;
        ///Clone io context
        const CLONE_IO		        = 0x80000000;
    }
}

bitflags! {
    pub struct Flags: u32 {
        const MAP_SHARED = 0x01;
        const MAP_PRIVATE = 0x02;
        const MAP_FIXED = 0x10;
        const MAP_ANONYMOUS = 0x20;
        const MAP_GROWSDOWN = 0x0100;
        const MAP_DENYWRITE = 0x0800;
        const MAP_EXECUTABLE = 0x1000;
        const MAP_LOCKED = 0x2000;
        const MAP_NORESERVE = 0x4000;
        const MAP_POPULATE = 0x8000;
        const MAP_NONBLOCK = 0x10000;
        const MAP_STACK = 0x20000;
        const MAP_HUGETLB = 0x40000;
        const MAP_SYNC = 0x80000;
        const MAP_FIXED_NOREPLACE = 0x100000;
        const MAP_UNINITIALIZED = 0x4000000;
    }
}

/// Process Control Block
pub struct ProcessControlBlock {
    /// immutable
    pub pid: PidHandle,
    /// mutable
    inner: UPSafeCell<ProcessControlBlockInner>,
}

/// Inner of Process Control Block
pub struct ProcessControlBlockInner {
    /// is zombie?
    pub is_zombie: bool,
    /// memory set(address space)
    pub memory_set: MemorySet,
    /// parent process
    pub parent: Option<Weak<ProcessControlBlock>>,
    /// children process
    pub children: Vec<Arc<ProcessControlBlock>>,
    /// exit code
    pub exit_code: i32,
    /// file descriptor table
    pub fd_table: Vec<Option<Arc<dyn File + Send + Sync>>>,
    /// signal flags
    pub signals: SignalFlags,
    /// tasks(also known as threads)
    pub tasks: Vec<Option<Arc<TaskControlBlock>>>,
    /// task resource allocator
    pub task_res_allocator: RecycleAllocator,
    /// mutex list
    // pub mutex_list: Vec<Option<Arc<dyn MutexSupport>>>,
    /// semaphore list
    // pub semaphore_list: Vec<Option<Arc<Semaphore>>>,
    // /// condvar list
    // pub condvar_list: Vec<Option<Arc<Condvar>>>,
    /// enable deadlock detect
    pub deadlock_detect: bool,
    /// available list
    pub available: Vec<u32>,
    /// allocation matrix
    pub allocation: Vec<Vec<u32>>,
    /// nedd matrix
    pub need: Vec<Vec<u32>>,
    /// finish list
    pub finish: Vec<bool>,
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
    /// working directory
    pub work_dir: Arc<OSInode>,
}

impl ProcessControlBlockInner {
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
    /// allocate a new task id
    pub fn alloc_tid(&mut self) -> usize {
        self.task_res_allocator.alloc()
    }
    /// deallocate a task id
    pub fn dealloc_tid(&mut self, tid: usize) {
        self.task_res_allocator.dealloc(tid)
    }
    /// the count of tasks(threads) in this process
    pub fn thread_count(&self) -> usize {
        self.tasks.len()
    }
    /// get a task with tid in this process
    pub fn get_task(&self, tid: usize) -> Arc<TaskControlBlock> {
        self.tasks[tid].as_ref().unwrap().clone()
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
            children_kernel_clock += c.inner_exclusive_access().kernel_clock;
            children_user_clock += c.inner_exclusive_access().user_clock;
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

impl ProcessControlBlock {
    /// inner_exclusive_access
    pub fn inner_exclusive_access(&self) -> RefMut<'_, ProcessControlBlockInner> {
        self.inner.exclusive_access()
    }
    /// new process from elf file
    pub fn new(elf_data: &[u8]) -> Arc<Self> {
        trace!("kernel: ProcessControlBlock::new");
        // memory_set with elf program headers/trampoline/trap context/user stack
        let (memory_set, user_heap_base, ustack_top, entry_point) = MemorySet::from_elf(elf_data);
        // allocate a pid
        let pid_handle = pid_alloc();
        let process = Arc::new(Self {
            pid: pid_handle,
            inner: unsafe {
                UPSafeCell::new(ProcessControlBlockInner {
                    is_zombie: false,
                    memory_set,
                    parent: None,
                    children: Vec::new(),
                    exit_code: 0,
                    fd_table: vec![
                        // 0 -> stdin
                        Some(Arc::new(Stdin)),
                        // 1 -> stdout
                        Some(Arc::new(Stdout)),
                        // 2 -> stderr
                        Some(Arc::new(Stdout)),
                    ],
                    signals: SignalFlags::empty(),
                    tasks: Vec::new(),
                    task_res_allocator: RecycleAllocator::new(),
                    // mutex_list: Vec::new(),
                    // semaphore_list: Vec::new(),
                    // condvar_list: Vec::new(),
                    deadlock_detect: false,
                    available: Vec::new(),
                    allocation: Vec::new(),
                    need: Vec::new(),
                    finish: Vec::new(),
                    clock_stop_watch: 0,
                    user_clock: 0,
                    kernel_clock: 0,
                    heap_base: user_heap_base.into(),
                    heap_end: user_heap_base.into(),
                    work_dir: ROOT_INODE.clone(),
                })
            },
        });
        info!("create new TaskControlBlock, ustack_top ={:#x}", ustack_top);
        // create a main thread, we should allocate ustack and trap_cx here
        let task = Arc::new(TaskControlBlock::new(
            Arc::clone(&process),
            ustack_top,
            true,
        ));
        info!("TaskControlBlock create completed");
        // prepare trap_cx of main thread
        let task_inner = task.inner_exclusive_access();
        let trap_cx = task_inner.get_trap_cx();
        let ustack_top = task_inner.res.as_ref().unwrap().ustack_top();
        let kstack_top = task.kstack.get_top();
        drop(task_inner);
        debug!("TrapContext::app_init_context");
        // debug!("*trap_cx = {:?}", *trap_cx);
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            ustack_top - 4,
            KERNEL_SPACE.exclusive_access().token(),
            kstack_top,
            trap_handler as usize,
        );
        debug!("TrapContext completed");
        // add main thread to the process
        let mut process_inner = process.inner_exclusive_access();
        process_inner.tasks.push(Some(Arc::clone(&task)));
        let mut res = process_inner.available.clone();
        res.fill(0);
        process_inner.allocation.push(res.clone());
        process_inner.need.push(res.clone());
        process_inner.finish.push(false);
        drop(process_inner);
        debug!("insert_into_pid2process");
        insert_into_pid2process(process.getpid(), Arc::clone(&process));
        // add main thread to scheduler
        add_task(task);
        process
    }

    /// Only support processes with a single thread.
    pub fn exec(self: &Arc<Self>, elf_data: &[u8], args: Vec<String>) {
        trace!("kernel: exec");
        assert_eq!(self.inner_exclusive_access().thread_count(), 1);
        // memory_set with elf program headers/trampoline/trap context/user stack
        trace!("kernel: exec .. MemorySet::from_elf");
        let (memory_set, user_heap_base, ustack_top, entry_point) = MemorySet::from_elf(elf_data);
        let new_token = memory_set.token();
        // substitute memory_set
        trace!("kernel: exec .. substitute memory_set");
        self.inner_exclusive_access().memory_set = memory_set;
        // set heap position
        self.inner_exclusive_access().heap_base = user_heap_base.into();
        self.inner_exclusive_access().heap_end = user_heap_base.into();
        // then we alloc user resource for main thread again
        // since memory_set has been changed
        trace!("kernel: exec .. alloc user resource for main thread again");
        let task = self.inner_exclusive_access().get_task(0);
        let mut task_inner = task.inner_exclusive_access();
        task_inner.res.as_mut().unwrap().ustack_top = ustack_top;
        task_inner.res.as_mut().unwrap().alloc_user_res();
        task_inner.trap_cx_ppn = task_inner.res.as_mut().unwrap().trap_cx_ppn();
        // push arguments on user stack
        trace!("kernel: exec .. push arguments on user stack");
        let mut user_sp = task_inner.res.as_mut().unwrap().ustack_top();
        user_sp -= (args.len() + 1) * core::mem::size_of::<usize>();
        let argv_base = user_sp;
        let mut argv: Vec<_> = (0..=args.len())
            .map(|arg| {
                translated_refmut(
                    new_token,
                    (argv_base + arg * core::mem::size_of::<usize>()) as *mut usize,
                )
            })
            .collect();
        *argv[args.len()] = 0;
        for i in 0..args.len() {
            user_sp -= args[i].len() + 1;
            *argv[i] = user_sp;
            let mut p = user_sp;
            for c in args[i].as_bytes() {
                *translated_refmut(new_token, p as *mut u8) = *c;
                p += 1;
            }
            *translated_refmut(new_token, p as *mut u8) = 0;
        }
        // make the user_sp aligned to 8B for k210 platform
        user_sp -= user_sp % core::mem::size_of::<usize>();
        // initialize trap_cx
        trace!("kernel: exec .. initialize trap_cx");
        let mut trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            task.kstack.get_top(),
            trap_handler as usize,
        );
        trap_cx.x[10] = args.len();
        trap_cx.x[11] = argv_base;
        *task_inner.get_trap_cx() = trap_cx;
    }

    /// Only support processes with a single thread.
    pub fn fork(self: &Arc<Self>) -> usize {
        trace!("kernel: sys_fork");
        let mut parent = self.inner_exclusive_access();
        assert_eq!(parent.thread_count(), 1);
        // clone parent's memory_set completely including trampoline/ustacks/trap_cxs
        let memory_set = MemorySet::from_existed_user(&parent.memory_set);
        // alloc a pid
        let pid = pid_alloc();
        // copy fd table
        let mut new_fd_table: Vec<Option<Arc<dyn File + Send + Sync>>> = Vec::new();
        for fd in parent.fd_table.iter() {
            if let Some(file) = fd {
                new_fd_table.push(Some(file.clone()));
            } else {
                new_fd_table.push(None);
            }
        }
        // create child process pcb
        let child = Arc::new(Self {
            pid,
            inner: unsafe {
                UPSafeCell::new(ProcessControlBlockInner {
                    is_zombie: false,
                    memory_set,
                    parent: Some(Arc::downgrade(self)),
                    children: Vec::new(),
                    exit_code: 0,
                    fd_table: new_fd_table,
                    signals: SignalFlags::empty(),
                    tasks: Vec::new(),
                    task_res_allocator: RecycleAllocator::new(),
                    // mutex_list: Vec::new(),
                    // semaphore_list: Vec::new(),
                    // condvar_list: Vec::new(),
                    deadlock_detect: false,
                    available: Vec::new(),
                    allocation: Vec::new(),
                    need: Vec::new(),
                    finish: Vec::new(),
                    clock_stop_watch: 0,
                    user_clock: 0,
                    kernel_clock: 0,
                    heap_base: parent.heap_base,
                    heap_end: parent.heap_base,
                    work_dir: parent.work_dir.clone(),
                })
            },
        });
        // add child
        parent.children.push(Arc::clone(&child));
        // create main thread of child process
        let task = Arc::new(TaskControlBlock::new(
            Arc::clone(&child),
            parent
                .get_task(0)
                .inner_exclusive_access()
                .res
                .as_ref()
                .unwrap()
                .ustack_top(),
            // here we do not allocate trap_cx or ustack again
            // but mention that we allocate a new kstack here
            false,
        ));
        // attach task to child process
        let mut child_inner = child.inner_exclusive_access();
        child_inner.tasks.push(Some(Arc::clone(&task)));
        let mut res = child_inner.available.clone();
        res.fill(0);
        // 防死锁算法
        child_inner.allocation.push(res.clone());
        child_inner.need.push(res.clone());
        child_inner.finish.push(false);
        drop(child_inner);

        // modify kstack_top in trap_cx of this thread
        let task_inner = task.inner_exclusive_access();
        let trap_cx = task_inner.get_trap_cx();
        trap_cx.kernel_sp = task.kstack.get_top();
        trap_cx.x[10] = 0;
        drop(task_inner);
        let pid = child.getpid();
        insert_into_pid2process(child.getpid(), Arc::clone(&child));
        // add this thread to scheduler
        add_task(task);
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
        trace!("kernel: clone");
        let task = current_task().unwrap();
        let process = task.process.upgrade().unwrap();
        // create a new thread.
        // We did not alloc for stack space here
        let thread_stack_top = if stack_ptr != 0 {
            stack_ptr
        } else {
            task.inner_exclusive_access()
                .res
                .as_ref()
                .unwrap()
                .ustack_top
        };
        let new_task = Arc::new(TaskControlBlock::new(
            Arc::clone(&process),
            thread_stack_top,
            true,
        ));
        let new_task_inner = new_task.inner_exclusive_access();
        let new_task_res = new_task_inner.res.as_ref().unwrap();
        let new_task_tid = new_task_res.tid;
        let mut process_inner = process.inner_exclusive_access();
        // add new thread to current process
        let tasks = &mut process_inner.tasks;
        while tasks.len() < new_task_tid + 1 {
            tasks.push(None);
        }
        tasks[new_task_tid] = Some(Arc::clone(&new_task));
        let new_task_trap_cx = new_task_inner.get_trap_cx();

        // I don't know if this is correct
        *new_task_trap_cx = *task.inner_exclusive_access().get_trap_cx();

        // for child process, fork returns 0
        new_task_trap_cx.x[10] = 0;
        // set tp reg
        new_task_trap_cx.x[4] = tls;
        // set sp reg
        new_task_trap_cx.set_sp(new_task_res.ustack_top());
        // modify kernel_sp in trap_cx
        new_task_trap_cx.kernel_sp = new_task.kstack.get_top();

        // add new task to scheduler
        add_task(Arc::clone(&new_task));

        drop(new_task_inner);
        new_task
    }
    /// get pid
    pub fn getpid(&self) -> usize {
        self.pid.0
    }

    pub fn fork2(self: &Arc<Self>, stack_ptr: usize) -> usize {
        trace!("kernel: sys_fork2");
        let mut parent = self.inner_exclusive_access();
        assert_eq!(parent.thread_count(), 1);
        // clone parent's memory_set completely including trampoline/ustacks/trap_cxs
        let memory_set = MemorySet::from_existed_user(&parent.memory_set);
        // alloc a pid
        let pid = pid_alloc();
        // copy fd table
        let mut new_fd_table: Vec<Option<Arc<dyn File + Send + Sync>>> = Vec::new();
        for fd in parent.fd_table.iter() {
            if let Some(file) = fd {
                new_fd_table.push(Some(file.clone()));
            } else {
                new_fd_table.push(None);
            }
        }
        // create child process pcb
        let child = Arc::new(Self {
            pid,
            inner: unsafe {
                UPSafeCell::new(ProcessControlBlockInner {
                    is_zombie: false,
                    memory_set,
                    parent: Some(Arc::downgrade(self)),
                    children: Vec::new(),
                    exit_code: 0,
                    fd_table: new_fd_table,
                    signals: SignalFlags::empty(),
                    tasks: Vec::new(),
                    task_res_allocator: RecycleAllocator::new(),
                    // mutex_list: Vec::new(),
                    // semaphore_list: Vec::new(),
                    // condvar_list: Vec::new(),
                    deadlock_detect: false,
                    available: Vec::new(),
                    allocation: Vec::new(),
                    need: Vec::new(),
                    finish: Vec::new(),
                    clock_stop_watch: 0,
                    user_clock: 0,
                    kernel_clock: 0,
                    heap_base: parent.heap_base,
                    heap_end: parent.heap_base,
                    work_dir: parent.work_dir.clone(),
                })
            },
        });
        // add child
        parent.children.push(Arc::clone(&child));
        // create main thread of child process
        let task = Arc::new(TaskControlBlock::new(
            Arc::clone(&child),
            parent
                .get_task(0)
                .inner_exclusive_access()
                .res
                .as_ref()
                .unwrap()
                .ustack_top(),
            // here we do not allocate trap_cx or ustack again
            // but mention that we allocate a new kstack here
            false,
        ));
        // attach task to child process
        let mut child_inner = child.inner_exclusive_access();
        child_inner.tasks.push(Some(Arc::clone(&task)));
        let mut res = child_inner.available.clone();
        res.fill(0);
        // 防死锁算法
        child_inner.allocation.push(res.clone());
        child_inner.need.push(res.clone());
        child_inner.finish.push(false);
        drop(child_inner);

        // modify kstack_top in trap_cx of this thread
        let task_inner = task.inner_exclusive_access();
        let trap_cx = task_inner.get_trap_cx();
        trap_cx.kernel_sp = task.kstack.get_top();
        trap_cx.x[10] = 0;
        trap_cx.x[2] = stack_ptr;
        drop(task_inner);
        let pid = child.getpid();
        insert_into_pid2process(child.getpid(), Arc::clone(&child));
        // add this thread to scheduler
        add_task(task);
        pid
    }
}
