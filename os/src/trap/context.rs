//! Implementation of [`TrapContext`]
use riscv::register::sstatus::{self, Sstatus, SPP};

#[repr(C)]
#[derive(Debug, Clone, Copy)]
/// trap context structure containing sstatus, sepc and registers
pub struct TrapContext {
    /// General-Purpose Register x0-31
    pub x: [usize; 32],
    /// Supervisor Status Register
    pub sstatus: Sstatus,
    /// Supervisor Exception Program Counter
    pub sepc: usize,
    /// Token of kernel address space
    pub kernel_satp: usize,
    /// Kernel stack pointer of the current application
    pub kernel_sp: usize,
    /// Virtual address of trap handler entry point in kernel
    pub trap_handler: usize,
}

impl TrapContext {
    /// put the sp(stack pointer) into x\[2\] field of TrapContext
    pub fn set_sp(&mut self, sp: usize) {
        self.x[2] = sp;
    }
    /// init the trap context of an application
    pub fn app_init_context(
        entry: usize,
        sp: usize,
        kernel_satp: usize,
        kernel_sp: usize,
        trap_handler: usize,
    ) -> Self {
        let mut sstatus = sstatus::read();
        // set CPU privilege to User after trapping back
        sstatus.set_spp(SPP::User);
        let mut cx = Self {
            x: [0; 32],
            sstatus,
            sepc: entry,  // entry point of app
            kernel_satp,  // addr of page table
            kernel_sp,    // kernel stack
            trap_handler, // addr of trap_handler function
        };
        cx.set_sp(sp); // app's user stack pointer
        cx // return initial Trap Context of app
    }
}
