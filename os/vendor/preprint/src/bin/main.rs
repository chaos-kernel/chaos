use core::fmt::Arguments;
use preprint::{init_print, pprintln, Print};


struct TestPrint;
impl Print for TestPrint{
    fn print(&self, args: Arguments) {
        print!("{}", args);
    }
}
fn test_print(){
    init_print(&TestPrint);
    pprintln!("test print");
}
fn main(){
    test_print();
}