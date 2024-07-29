use alloc::string::String;

use riscv::register::sstatus;

pub fn c_ptr_to_string(c_ptr: *const u8) -> String {
    let mut res = String::new();
    let mut i = 0;
    loop {
        let c = unsafe { *c_ptr.add(i) };
        if c == 0 {
            break;
        }
        res.push(c as char);
        i += 1;
    }
    res
}
