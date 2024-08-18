use core::arch::asm;

use buddy_system_allocator::LockedHeap;

use crate::config::HEAP_SIZE;
use crate::config::{CORES, STACK_SIZE, VF2_FREQ};

#[global_allocator]
static mut HEAP_ALLOCATOR: LockedHeap<32> = LockedHeap::empty();

static mut HEAP: [u8; HEAP_SIZE] = [0; HEAP_SIZE];
pub fn init_heap() {
    unsafe {
        HEAP_ALLOCATOR
            .lock()
            .init(HEAP.as_ptr() as usize, HEAP_SIZE);
    }
}

#[naked]
#[no_mangle]
#[link_section = ".text.entry"]
unsafe extern "C" fn _start() -> ! {
    #[link_section = ".bss.uninit"]
    static mut STACK: [u8; STACK_SIZE * CORES] = [0u8; CORES * STACK_SIZE];
    core::arch::asm!(
    "   la sp, {stack}
        li t0, {stack_size}
        mv t1,a0
        mv tp,a0
        addi t1,t1,1
      1:mul t0,t0,t1
        add sp,sp,t0
        call {main}
      2:j 2b
        ",
    stack_size = const STACK_SIZE,
    stack      =   sym STACK,
    main       =   sym crate::main,
    options(noreturn),
    )
}
extern "C" {
    fn sbss();
    fn ebss();
}

pub fn clear_bss() {
    let len = ebss as usize - sbss as usize;
    unsafe {
        core::slice::from_raw_parts_mut(sbss as *mut u8, len).fill(0);
    }
}

pub fn hart_id() -> usize {
    let id: usize;
    unsafe {
        asm!("mv {},tp",out(reg) id);
    }
    id
}

pub fn read_time() -> usize {
    riscv::register::time::read()
}

pub fn read_time_ms() -> usize {
    read_time() / (VF2_FREQ / 1000)
}

pub fn read_time_us() -> usize {
    read_time() / (VF2_FREQ / 1000_000)
}

pub fn sleep_ms(ms: usize) {
    let start = read_time();
    while read_time() - start < ms * VF2_FREQ / 1000 {
        core::hint::spin_loop();
    }
}

pub fn sleep_ms_until(ms: usize, mut f: impl FnMut() -> bool) {
    let start = read_time();
    while read_time() - start < ms * VF2_FREQ / 1000 {
        if f() {
            return;
        }
        core::hint::spin_loop();
    }
}

pub fn sleep_us(us: usize) {
    let start = read_time();
    while read_time() - start < us * VF2_FREQ / 1000_000 {
        core::hint::spin_loop();
    }
}
