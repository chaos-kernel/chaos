//! Physical page frame allocator

use alloc::vec::Vec;
use core::fmt::{self, Debug, Formatter};

use lazy_static::*;

use super::{PhysAddr, PhysPageNum};
use crate::{config::MEMORY_END, mm::address::KernelAddr, sync::UPSafeCell};

/// tracker for physical page frame allocation and deallocation
pub struct FrameTracker {
    /// physical page number
    pub ppn: PhysPageNum,
}

impl FrameTracker {
    /// Create a new FrameTracker
    pub fn new(ppn: PhysPageNum) -> Self {
        // page cleaning
        // debug!("set FrameTracker::new: ppn={:?}", ppn);
        let bytes_array = ppn.get_bytes_array();
        for i in bytes_array {
            *i = 0;
        }
        // debug!("new FrameTracker::new: ppn={:?}", ppn);
        Self { ppn }
    }
}

impl Debug for FrameTracker {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("FrameTracker:PPN={:#x}", self.ppn.0))
    }
}

impl Drop for FrameTracker {
    fn drop(&mut self) {
        frame_dealloc(self.ppn);
    }
}

trait FrameAllocator {
    fn new() -> Self;
    fn alloc(&mut self) -> Option<PhysPageNum>;
    fn alloc_contiguous(&mut self, num: usize) -> (Vec<PhysPageNum>, PhysPageNum);
    fn dealloc(&mut self, ppn: PhysPageNum);
}

pub struct StackFrameAllocator {
    current:  usize,
    end:      usize,
    recycled: Vec<usize>,
}

impl StackFrameAllocator {
    pub fn init(&mut self, l: PhysPageNum, r: PhysPageNum) {
        self.current = l.0;
        self.end = r.0;
        // trace!("last {} Physical Frames.", self.end - self.current);
    }
}
impl FrameAllocator for StackFrameAllocator {
    fn new() -> Self {
        Self {
            current:  0,
            end:      0,
            recycled: Vec::new(),
        }
    }
    fn alloc(&mut self) -> Option<PhysPageNum> {
        // debug!(
        //     "alloc a new page: current={:#x} end={:#x}",
        //     self.current, self.end
        // );
        // if let Some(ppn) = self.recycled.pop() {
        //     debug!(" alloc a new page: recycled ppn={:#x}", ppn);
        //     Some(ppn.into())
        // } else
        if self.current == self.end {
            error!("FrameAllocator out of memory!");
            None
        } else {
            // debug!("alloc a new page: new ppn={:#x}", self.current);
            self.current += 1;
            Some((self.current - 1).into())
        }
    }
    fn alloc_contiguous(&mut self, num: usize) -> (Vec<PhysPageNum>, PhysPageNum) {
        let mut ret = Vec::with_capacity(num);
        let root_ppn = self.current;
        for _ in 0..num {
            if self.current == self.end {
                error!("FrameAllocator out of memory!");
                panic!("FrameAllocator out of memory!");
            } else {
                // debug!("alloc a new page contiguous: new ppn={:#x}", self.current);
                self.current += 1;
                ret.push((self.current - 1).into());
            }
        }
        (ret, root_ppn.into())
    }
    fn dealloc(&mut self, ppn: PhysPageNum) {
        // debug!("dealloc a page: ppn={:#x}", ppn.0);
        let ppn = ppn.0;
        // validity check
        if ppn >= self.current || self.recycled.iter().any(|&v| v == ppn) {
            panic!("Frame ppn={:#x} has not been allocated!", ppn);
        }
        // recycle
        self.recycled.push(ppn);
    }
}

type FrameAllocatorImpl = StackFrameAllocator;

lazy_static! {
    pub static ref FRAME_ALLOCATOR: UPSafeCell<FrameAllocatorImpl> =
        unsafe { UPSafeCell::new(FrameAllocatorImpl::new()) };
}

pub fn init_frame_allocator(memory_end: usize) {
    extern "C" {
        fn ekernel();
    }
    debug!(
        "init_frame_allocator: ekernel={:#x} memory_end={:#x}",
        ekernel as usize, memory_end
    );
    debug!(
        "PhysAddr::from(ekernel as usize)={:?}",
        PhysAddr::from(ekernel as usize)
    );
    debug!(
        "PhysAddr::from(MEMORY_END)={:?}",
        PhysAddr::from(memory_end)
    );
    FRAME_ALLOCATOR.exclusive_access(file!(), line!()).init(
        PhysAddr::from(KernelAddr::from(ekernel as usize)).ceil(),
        PhysAddr::from(KernelAddr::from(memory_end)).floor(),
    );
}

/// Allocate a physical page frame in FrameTracker style
pub fn frame_alloc() -> Option<FrameTracker> {
    FRAME_ALLOCATOR
        .exclusive_access(file!(), line!())
        .alloc()
        .map(FrameTracker::new)
}

/// Allocate n contiguous physical page frames in FrameTracker style
pub fn frame_alloc_contiguous(num: usize) -> (Vec<FrameTracker>, PhysPageNum) {
    let (frames, root_ppn) = FRAME_ALLOCATOR
        .exclusive_access(file!(), line!())
        .alloc_contiguous(num);
    let frame_trackers: Vec<FrameTracker> = frames.iter().map(|&p| FrameTracker::new(p)).collect();
    (frame_trackers, root_ppn)
}

/// Deallocate a physical page frame with a given ppn
pub fn frame_dealloc(ppn: PhysPageNum) {
    // debug!("dealloc a page: ppn={:#x}", ppn.0);
    FRAME_ALLOCATOR
        .exclusive_access(file!(), line!())
        .dealloc(ppn);
}

#[allow(unused)]
pub fn frame_allocator_test() {
    let mut v: Vec<FrameTracker> = Vec::new();
    for i in 0..5 {
        let frame = frame_alloc().unwrap();
        println!("{:?}", frame);
        v.push(frame);
    }
    v.clear();
    for i in 0..5 {
        let frame = frame_alloc().unwrap();
        println!("{:?}", frame);
        v.push(frame);
    }
    drop(v);
    println!("frame_allocator_test passed!");
}
