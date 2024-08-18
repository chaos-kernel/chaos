use core::arch::asm;

const SBI_SET_TIMER: usize = 0;
const SBI_CONSOLE_PUT_CHAR: usize = 1;
const SBI_CONSOLE_GET_CHAR: usize = 2;
const SBI_SHUTDOWN: usize = 8;

// SBI 调用
fn sbi_call(which: usize, arg0: usize, arg1: usize, arg2: usize) -> i32 {
    let mut ret;
    unsafe {
        asm!("ecall",
        in("a7") which,
        inlateout("a0") arg0 as i32 => ret,
        in("a1") arg1,
        in("a2") arg2);
    }
    ret
}

pub fn set_timer(time: usize) {
    sbi_call(SBI_SET_TIMER, time, 0, 0);
}

pub fn console_putchar(ch: u8) {
    sbi_call(SBI_CONSOLE_PUT_CHAR, ch as usize, 0, 0);
}

pub fn console_getchar() -> char {
    sbi_call(SBI_CONSOLE_GET_CHAR, 0, 0, 0) as u8 as char
}

pub fn shutdown() -> ! {
    sbi_call(SBI_SHUTDOWN, 0, 0, 0);
    unreachable!()
}
