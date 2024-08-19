//! Constants in the kernel

#[allow(unused)]

/// user app's stack size
pub const USER_STACK_SIZE: usize = 4096 * 20;
/// kernel stack size
pub const KERNEL_STACK_SIZE: usize = 4096 * 8;
/// kernel heap size
pub const KERNEL_HEAP_SIZE: usize = PAGE_SIZE * 0x500;
/// physical memory end address
#[cfg(feature = "qemu")]
pub const MEMORY_END: usize = 0xffff_ffc0_88000000;

#[cfg(feature = "visionfive2")]
pub const MEMORY_END: usize = 0xffff_ffc0_88000000;

/// page size : 4KB
pub const PAGE_SIZE: usize = 0x1000;
/// page size bits: 12
pub const PAGE_SIZE_BITS: usize = 0xc;
/// the max number of syscall
pub const MAX_SYSCALL_NUM: usize = 500;
// /// the virtual addr of trapoline
// pub const TRAMPOLINE: usize = usize::MAX - PAGE_SIZE + 1;
/// user space end
pub const USER_SPACE_END: usize = 0x0000_003F_FFFF_FFFF;
/// kernel space end
pub const KERNEL_SPACE_END: usize = 0xFFFF_FFFF_FFFF_FFFF;
/// the virtual addr of trap context
pub const TRAP_CONTEXT_BASE: usize = USER_SPACE_END - PAGE_SIZE * 2 + 1;
/// qemu board info
pub use crate::boards::{CLOCK_FREQ, MMIO};
/// Big stride (lcm of 2..20)
pub const BIG_STRIDE: usize = 232792560;
/// system name
pub const SYS_NAME: &str = "Chaos";
/// system nodename
pub const SYS_NODENAME: &str = "None";
/// system release
pub const SYS_RELEASE: &str = "0.0.1";
/// system version
pub const SYS_VERSION: &str = "#1-Chaos RISC-V 64bit Version 0.0.1";
///
pub const STACK_TOP: usize = 0x1_0000_0000;
///
pub const MMAP_BASE: usize = 0x2000_0000;
/// SV39
pub const PAGE_TABLE_LEVEL: usize = 3;
/// kernel space offset
pub const KERNEL_SPACE_OFFSET: usize = 0xffff_ffc0_0000_0;

pub const TRAP_CONTEXT_TRAMPOLINE: usize = 0xFFFF_FFFF_FFFF_E000;

/// user trampoline
pub const USER_TRAMPOLINE: usize = 0x191_9810;

#[no_mangle]
#[inline(never)]
pub fn __breakpoint() {}
