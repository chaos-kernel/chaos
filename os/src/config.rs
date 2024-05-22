//! Constants in the kernel

#[allow(unused)]

/// user app's stack size
pub const USER_STACK_SIZE: usize = 4096 * 2;
/// kernel stack size
pub const KERNEL_STACK_SIZE: usize = 4096 * 2;
/// kernel heap size
pub const KERNEL_HEAP_SIZE: usize = 0x200_0000;
/// physical memory end address
pub const MEMORY_END: usize = 0x88000000;
/// page size : 4KB
pub const PAGE_SIZE: usize = 0x1000;
/// page size bits: 12
pub const PAGE_SIZE_BITS: usize = 0xc;
/// the max number of syscall
pub const MAX_SYSCALL_NUM: usize = 500;
/// the virtual addr of trapoline
pub const TRAMPOLINE: usize = usize::MAX - PAGE_SIZE + 1;
/// the virtual addr of trap context
pub const TRAP_CONTEXT_BASE: usize = TRAMPOLINE - PAGE_SIZE;
/// qemu board info
pub use crate::board::{CLOCK_FREQ, MMIO};
/// Big stride (lcm of 2..20)
pub const BIG_STRIDE: usize = 232792560;