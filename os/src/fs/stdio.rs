use riscv::register::sstatus;

use super::{file::File, inode::Stat};
use crate::{mm::UserBuffer, sbi::console_getchar, task::suspend_current_and_run_next};

/// stdin file for getting chars from console
pub struct Stdin;

/// stdout file for putting chars to console
pub struct Stdout;

impl File for Stdin {
    fn readable(&self) -> bool {
        true
    }
    fn writable(&self) -> bool {
        false
    }
    fn read(&self, user_buf: &mut [u8]) -> usize {
        // assert_eq!(user_buf.len(), 1);
        // busy loop
        unsafe {
            sstatus::set_sum();
        }
        let mut c: usize;
        loop {
            c = console_getchar();
            if c == 0 {
                debug!("stdin: no char, suspend and run next");
                suspend_current_and_run_next();
                continue;
            } else {
                break;
            }
        }
        let ch = c as u8;
        user_buf[0] = ch;
        unsafe {
            sstatus::clear_sum();
        }
        1
    }
    fn read_all(&self) -> alloc::vec::Vec<u8> {
        panic!("Stdin::read_all not implemented");
    }
    fn write(&self, _user_buf: &[u8]) -> usize {
        panic!("Cannot write to stdin!");
    }
    fn fstat(&self) -> Option<Stat> {
        None
    }
    fn hang_up(&self) -> bool {
        todo!()
    }
}

impl File for Stdout {
    fn readable(&self) -> bool {
        false
    }
    fn writable(&self) -> bool {
        true
    }
    fn read(&self, _user_buf: &mut [u8]) -> usize {
        panic!("Cannot read from stdout!");
    }
    fn read_all(&self) -> alloc::vec::Vec<u8> {
        panic!("Stdout::read_all not allowed");
    }
    fn write(&self, user_buf: &[u8]) -> usize {
        unsafe {
            sstatus::set_sum();
            print!("{}", core::str::from_utf8(user_buf).unwrap());
            sstatus::clear_sum();
        }
        user_buf.len()
    }
    fn fstat(&self) -> Option<Stat> {
        None
    }
    fn hang_up(&self) -> bool {
        todo!()
    }
}
