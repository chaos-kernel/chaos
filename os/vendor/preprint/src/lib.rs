#![no_std]
use core::fmt;
pub trait Print:Sync{
    fn print(&self,args: fmt::Arguments);
}

struct NonePrint;

impl Print for NonePrint{
    fn print(&self,_args: fmt::Arguments) {}
}


static mut PRINT: &dyn Print = &NonePrint;


pub fn init_print(print: &'static dyn Print){
    unsafe{
        PRINT = print;
    }
}

#[doc(hidden)]
/// this function is private
pub fn __private_print(args: fmt::Arguments){
    unsafe{
        PRINT.print(args);
    }
}

#[macro_export]
macro_rules! pprint {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::__private_print(format_args!($fmt $(, $($arg)+)?));
    }
}

#[macro_export]
macro_rules! pprintln {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::__private_print(format_args!(concat!($fmt, "\n") $(, $($arg)+)?));
    }
}

#[cfg(test)]
mod test{
    use core::fmt::Arguments;
    use crate::{init_print, Print};

    struct TestPrint;
    impl Print for TestPrint{
        fn print(&self, _args: Arguments) {
            // print!("{}", args);
        }
    }
    #[test]
    fn test_print(){
        init_print(&TestPrint);
        pprintln!("test print");
    }
}