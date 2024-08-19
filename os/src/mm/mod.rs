//! Memory management implementation
//!
//! SV39 page-based virtual-memory architecture for RV64 systems, and
//! everything about memory management, like frame allocator, page table,
//! map area and memory set, is implemented here.
//!
//! Every task or process has a memory_set to control its virtual memory.

mod address;
mod config;
mod frame_allocator;
mod heap_allocator;
mod memory_set;
mod page_table;

use address::VPNRange;
pub use address::{KernelAddr, PhysAddr, PhysPageNum, StepByOne, VirtAddr, VirtPageNum};
pub use frame_allocator::{frame_alloc, frame_alloc_contiguous, frame_dealloc, FrameTracker};
pub use heap_allocator::init_heap;
pub use memory_set::{kernel_token, remap_test, MapPermission, MemorySet, KERNEL_SPACE};
pub use page_table::{
    translated_byte_buffer,
    translated_ref,
    translated_refmut,
    translated_str,
    PTEFlags,
    PageTable,
    PageTableEntry,
    UserBuffer,
    UserBufferIterator,
};

/// initiate heap allocator, frame allocator and kernel space
pub fn init(memory_end: usize) {
    debug!("heap allocator initialize");
    heap_allocator::init_heap();
    debug!("frame allocator initialize");
    frame_allocator::init_frame_allocator(memory_end);
    debug!("kernel space initialize");
    KERNEL_SPACE.exclusive_access(file!(), line!()).activate();
}
