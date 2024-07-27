//! Allocator for pid, task user resource, kernel stack using a simple recycle strategy.

use crate::config::{
    __breakpoint, KERNEL_STACK_SIZE, MEMORY_END, PAGE_SIZE, TRAP_CONTEXT_BASE, USER_STACK_SIZE,
};
use crate::mm::PTEFlags;
use crate::mm::{MapPermission, PageTable, PhysPageNum, VirtAddr, KERNEL_SPACE};
use crate::sync::UPSafeCell;
use crate::trap::TrapContext;
use alloc::{
    sync::{Arc, Weak},
    vec::Vec,
};
use lazy_static::*;
use riscv::register::satp;

/// Allocator with a simple recycle strategy
pub struct RecycleAllocator {
    current: usize,
    recycled: Vec<usize>,
}

impl RecycleAllocator {
    /// Create a new allocator
    pub fn new() -> Self {
        RecycleAllocator {
            current: 0,
            recycled: Vec::new(),
        }
    }
    /// allocate a new item
    pub fn alloc(&mut self) -> usize {
        if let Some(id) = self.recycled.pop() {
            id
        } else {
            self.current += 1;
            self.current - 1
        }
    }
    /// deallocate an item
    pub fn dealloc(&mut self, id: usize) {
        assert!(id < self.current);
        assert!(
            !self.recycled.iter().any(|i| *i == id),
            "id {} has been deallocated!",
            id
        );
        self.recycled.push(id);
    }
}

lazy_static! {
    /// Glocal allocator for pid
    static ref PID_ALLOCATOR: UPSafeCell<RecycleAllocator> =
        unsafe { UPSafeCell::new(RecycleAllocator::new()) };
    /// Global allocator for kernel stack
    static ref KSTACK_ALLOCATOR: UPSafeCell<RecycleAllocator> =
        unsafe { UPSafeCell::new(RecycleAllocator::new()) };

}

/// The idle task's pid is 0
pub const IDLE_PID: usize = 0;

/// A handle to a pid
pub struct PidHandle(pub usize);

/// Allocate a pid for a process
pub fn pid_alloc() -> PidHandle {
    PidHandle(PID_ALLOCATOR.exclusive_access(file!(), line!()).alloc())
}

impl Drop for PidHandle {
    fn drop(&mut self) {
        trace!("drop pid {}", self.0);
        PID_ALLOCATOR
            .exclusive_access(file!(), line!())
            .dealloc(self.0);
    }
}

/// Return (bottom, top) of a kernel stack in kernel space.
pub fn kernel_stack_position(kstack_id: usize) -> (usize, usize) {
    let top = MEMORY_END + (kstack_id + 1) * (KERNEL_STACK_SIZE + PAGE_SIZE);
    let bottom = top - KERNEL_STACK_SIZE;
    (bottom, top)
}

/// Kernel stack for a task
pub struct KernelStack(pub usize);

/// Allocate a kernel stack for a task
pub fn kstack_alloc() -> KernelStack {
    trace!("kstack_alloc");

    let kstack_id = KSTACK_ALLOCATOR.exclusive_access(file!(), line!()).alloc();
    let (kstack_bottom, kstack_top) = kernel_stack_position(kstack_id);

    KERNEL_SPACE
        .exclusive_access(file!(), line!())
        .insert_framed_area(
            kstack_bottom.into(),
            kstack_top.into(),
            MapPermission::R | MapPermission::W,
        );

    KernelStack(kstack_id)
}

impl Drop for KernelStack {
    fn drop(&mut self) {
        let (kernel_stack_bottom, _) = kernel_stack_position(self.0);
        let kernel_stack_bottom_va: VirtAddr = kernel_stack_bottom.into();
        KERNEL_SPACE
            .exclusive_access(file!(), line!())
            .remove_area_with_start_vpn(kernel_stack_bottom_va.into());
        KSTACK_ALLOCATOR
            .exclusive_access(file!(), line!())
            .dealloc(self.0);
    }
}

impl KernelStack {
    /// Push a variable of type T into the top of the KernelStack and return its raw pointer
    #[allow(unused)]
    pub fn push_on_top<T>(&self, value: T) -> *mut T
    where
        T: Sized,
    {
        let kernel_stack_top = self.get_top();
        let ptr_mut = (kernel_stack_top - core::mem::size_of::<T>()) as *mut T;
        unsafe {
            *ptr_mut = value;
        }
        ptr_mut
    }
    /// return the top of the kernel stack
    pub fn get_top(&self) -> usize {
        let (_, kernel_stack_top) = kernel_stack_position(self.0);
        kernel_stack_top
    }
}

// /// User Resource for a task
// pub struct TaskUserRes {
//     /// task id
//     pub tid: usize,
//     /// user stack base
//     pub ustack_top: usize,
//     /// process belongs to
//     pub process: Weak<ProcessControlBlock>,
// }
/// Return the bottom addr (low addr) of the trap context for a task
pub fn trap_cx_bottom_from_tid(tid: usize) -> usize {
    TRAP_CONTEXT_BASE - tid * PAGE_SIZE
}
/// Return the bottom addr (high addr) of the user stack for a task
pub fn ustack_bottom_from_tid(ustack_base: usize, tid: usize) -> usize {
    ustack_base + tid * (PAGE_SIZE + USER_STACK_SIZE)
}

#[allow(unused)]
fn ustack_top_from_id(ustack_top: usize, id: usize) -> usize {
    //todo 暂时弃用，意义不明
    ustack_top - id * (PAGE_SIZE + USER_STACK_SIZE)
}

// impl TaskUserRes {
//     /// Create a new TaskUserRes (Task User Resource)
//     pub fn new(process: Arc<ProcessControlBlock>, ustack_top: usize, alloc_user_res: bool) -> Self {
//         let tid = process.inner_exclusive_access(file!(), line!()).alloc_tid();
//         let task_user_res = Self {
//             tid,
//             ustack_top,
//             process: Arc::downgrade(&process),
//         };
//         if alloc_user_res {
//             //todo 因为只有初始进程创建即分配，所以这里只有初始进程会调用
//             //todo 这里是临时措施，接下来马上删了这个b user_res
//             task_user_res.alloc_initproc_res();
//         }
//         task_user_res
//     }
//     /// Allocate user resource for a task
//     pub fn alloc_user_res(&self) -> PhysPageNum {
//         let process = self.process.upgrade().unwrap();
//         let mut process_inner = process.inner_exclusive_access(file!(), line!());
//         // alloc user stack
//         let ustack_top = self.ustack_top;
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
//         let trap_cx_bottom = trap_cx_bottom_from_tid(self.tid);
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
//         //想要为其他进程分配trap_cx，需要在这里映射到当前页表，否则无法写入
//         //实现无栈协程之后就不用考虑进程之间互相映射了
//         let trap_cx_bottom_va: VirtAddr = trap_cx_bottom.into();
//         let trap_cx_bottom_ppn = process_inner
//             .memory_set
//             .translate(trap_cx_bottom_va.into())
//             .unwrap()
//             .ppn();

//         trap_cx_bottom_ppn

//         // debug!(
//         //     "trap_cx_bottom ppn = {:x}",
//         //     process_inner
//         //         .memory_set
//         //         .translate(crate::mm::VirtPageNum::from(VirtAddr::from(trap_cx_bottom)))
//         //         .unwrap()
//         //         .ppn()
//         //         .0
//         // );
//     }

//     /// Allocate user resource for a task
//     pub fn alloc_initproc_res(&self) {
//         let process = self.process.upgrade().unwrap();
//         let mut process_inner = process.inner_exclusive_access(file!(), line!());
//         // alloc user stack
//         let ustack_top = self.ustack_top;
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
//         let trap_cx_bottom = trap_cx_bottom_from_tid(self.tid);
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
//         // debug!(
//         //     "trap_cx_bottom ppn = {:x}",
//         //     process_inner
//         //         .memory_set
//         //         .translate(crate::mm::VirtPageNum::from(VirtAddr::from(trap_cx_bottom)))
//         //         .unwrap()
//         //         .ppn()
//         //         .0
//         // );
//     }
//     /// Deallocate user resource for a task
//     fn dealloc_user_res(&self) {
//         // dealloc tid
//         let process = self.process.upgrade().unwrap();
//         let mut process_inner = process.inner_exclusive_access(file!(), line!());
//         // dealloc ustack manually
//         let ustack_top = ustack_top_from_id(self.ustack_top, self.tid);
//         let ustack_bottom_va: VirtAddr = (ustack_top - USER_STACK_SIZE).into();
//         process_inner
//             .memory_set
//             .remove_area_with_start_vpn(ustack_bottom_va.into());
//         // dealloc trap_cx manually
//         let trap_cx_bottom_va: VirtAddr = trap_cx_bottom_from_tid(self.tid).into();
//         process_inner
//             .memory_set
//             .remove_area_with_start_vpn(trap_cx_bottom_va.into());
//     }

//     #[allow(unused)]
//     /// alloc task id
//     pub fn alloc_tid(&mut self) {
//         self.tid = self
//             .process
//             .upgrade()
//             .unwrap()
//             .inner_exclusive_access(file!(), line!())
//             .alloc_tid();
//     }
//     /// dealloc task id
//     pub fn dealloc_tid(&self) {
//         let process = self.process.upgrade().unwrap();
//         let mut process_inner = process.inner_exclusive_access(file!(), line!());
//         process_inner.dealloc_tid(self.tid);
//     }
//     /// The bottom usr vaddr (low addr) of the trap context for a task with tid
//     pub fn trap_cx_user_va(&self) -> VirtAddr {
//         trap_cx_bottom_from_tid(self.tid).into()
//     }
//     /// The physical page number(ppn) of the trap context for a task with tid
//     pub fn trap_cx_ppn(&self) -> PhysPageNum {
//         let process = self.process.upgrade().unwrap();
//         let process_inner = process.inner_exclusive_access(file!(), line!());
//         let trap_cx_bottom_va: VirtAddr = trap_cx_bottom_from_tid(self.tid).into();
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
//     }
//     /// the bottom addr (low addr) of the user stack for a task
//     pub fn ustack_top(&self) -> usize {
//         self.ustack_top
//     }
//     /// the top addr (high addr) of the user stack for a task
//     pub fn ustack_base(&self) -> usize {
//         ustack_bottom_from_tid(self.ustack_top, self.tid) - USER_STACK_SIZE
//     }
// }

// impl Drop for TaskUserRes {
//     fn drop(&mut self) {
//         self.dealloc_tid();
//         self.dealloc_user_res();
//     }
// }
