use crate::boot::{hart_id, read_time, read_time_us};
use crate::println;
use core::arch::asm;
use core::sync::atomic;
use core::sync::atomic::{AtomicBool, AtomicUsize};
use jtable::*;
use paste::*;

const LOOP: usize = 1000;

static FLAG: AtomicBool = AtomicBool::new(true);

#[naked_function::naked]
pub unsafe extern "C" fn is_false() -> bool {
    asm!("li a0, 1", "ret",)
}

#[inline(never)]
#[no_mangle]
fn load_flag() -> bool {
    let inst_now = riscv::register::instret::read();
    let res = FLAG.load(core::sync::atomic::Ordering::SeqCst);
    let inst_end = riscv::register::instret::read();
    println!("load_flag: {}inst", inst_end - inst_now);
    res
}

#[inline(never)]
#[no_mangle]
unsafe fn load_fun() -> bool {
    let inst_now = riscv::register::instret::read();
    let res = is_false();
    let inst_end = riscv::register::instret::read();
    println!("load_fun: {}inst", inst_end - inst_now);
    res
}

#[inline(never)]
#[no_mangle]
fn load_nop() -> bool {
    let inst_now = riscv::register::instret::read();
    unsafe { asm!("nop") }
    let inst_end = riscv::register::instret::read();
    println!("load_nop: {}inst", inst_end - inst_now);
    true
}

unsafe fn test_static_keys() -> usize {
    let mut count = 0;
    let now = read_time_us();
    for _ in 0..LOOP {
        if is_false() {
            count += 1;
        }
        maybe_modify();
        if is_false() {
            count += 1;
        }
    }
    let end = read_time_us();
    println!("test_static_keys: {}us", end - now);
    println!("test_static_keys: {}", count);
    end - now
}

fn test_static_atomic() -> usize {
    let mut count = 0;
    let now = read_time_us();
    for _ in 0..LOOP {
        if FLAG.load(core::sync::atomic::Ordering::SeqCst) {
            count += 1;
        }
        maybe_modify();
        if FLAG.load(core::sync::atomic::Ordering::SeqCst) {
            count += 1;
        }
    }
    let end = read_time_us();
    println!("test_atomic: {}us", end - now);
    println!("test_atomic: {}", count);
    end - now
}

pub unsafe fn test() {
    // load_fun();
    // load_flag();
    // load_nop();

    // let end2 = test_static_keys();
    // let end1 = test_static_atomic();

    // if end1 > end2 {
    //     println!("static atomic :{} > static keys :{}", end1, end2);
    // } else {
    //     println!("static keys :{} > static atomic :{}", end2, end1);
    // }
    // let max = core::cmp::max(end1, end2);
    // let min = core::cmp::min(end1, end2);
    // let diff = (max - min) as f64 / max as f64;
    // println!("diff: {:.2}%", diff * 100.0);

    test_mass_static_atomic(0);
    test_mass_static_keys(0);

    let end1 = TIME_ATOMIC.load(atomic::Ordering::SeqCst);
    let end2 = TIME_KEYS.load(atomic::Ordering::SeqCst);

    if end1 > end2 {
        println!("static atomic :{} > static keys :{}", end1, end2);
    } else {
        println!("static keys :{} > static atomic :{}", end2, end1);
    }
    let max = core::cmp::max(end1, end2);
    let min = core::cmp::min(end1, end2);
    let diff = (max - min) as f64 / max as f64;
    println!("diff: {:.2}%", diff * 100.0);
}

#[repr(align(8))]
struct FlagCache {
    flag: AtomicUsize,
    _packed: [u64; 7],
}
impl FlagCache {
    const fn new() -> Self {
        FlagCache {
            flag: AtomicUsize::new(0),
            _packed: [0; 7],
        }
    }
    #[inline(always)]
    fn is_true(&self) -> bool {
        self.flag.load(atomic::Ordering::SeqCst) == 0
    }
    #[inline(always)]
    fn set_val(&self, val: usize) {
        self.flag.store(val, atomic::Ordering::SeqCst);
    }
}

const ARRAY_REPEAT_VALUE: FlagCache = FlagCache::new();
static FLAGS: [FlagCache; 10000] = [ARRAY_REPEAT_VALUE; 10000];

static TIME_ATOMIC: AtomicUsize = AtomicUsize::new(0);
static TIME_KEYS: AtomicUsize = AtomicUsize::new(0);

fn test_mass_static_atomic(cpu: usize) {
    let mut count = 0;
    let now = read_time_us();
    for i in 0..10 {
        for index in 0..100 {
            if FLAGS[index].is_true() {
                count += 1;
            }
            maybe_modify();
        }
    }
    let end1 = read_time_us() - now;
    println!("test_atomic: {}us", end1);
    println!("test_atomic: {}", count);
    TIME_ATOMIC.store(end1 as usize, atomic::Ordering::SeqCst);
}

fn test_mass_static_keys(cpu: usize) {
    let mut count = 0;
    let now = read_time_us();
    for i in 0..10 {
        for _ in 0..100 {
            unsafe {
                if is_false() {
                    count += 1;
                }
            }
            maybe_modify();
        }
    }
    let end2 = read_time_us() - now;
    println!("test_static_keys: {}us", end2);
    println!("test_static_keys: {}", count);
    TIME_KEYS.store(end2 as usize, atomic::Ordering::SeqCst);
}

#[repr(align(64))]
#[derive(Copy, Clone)]
pub struct DataCache {
    data: [u64; 2 * 1024 * 1024 / 8],
}

impl DataCache {
    pub const fn new() -> Self {
        DataCache {
            data: [0; 2 * 1024 * 1024 / 8],
        }
    }
    #[inline]
    pub fn fill(&mut self, val: u8) {
        let data = &mut self.data;
        for i in 0..data.len() {
            data[i] = (data[i] + val as u64);
        }
    }
}

pub static mut DATA_CACHE: [DataCache; 32] = [DataCache::new(); 32];

fn maybe_modify() {
    let time = read_time();
    if time < 1000 {
        FLAG.store(false, core::sync::atomic::Ordering::SeqCst);
        FLAGS[time].set_val(1);
    }
    unsafe {
        let cache = &mut DATA_CACHE[hart_id()];
        cache.fill((time % 255) as u8);
    }
}
