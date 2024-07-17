//! Trap handling functionality
//!
//! For rCore, we have a single trap entry point, namely `__alltraps`. At
//! initialization in [`init()`], we set the `stvec` CSR to point to it.
//!
//! All traps go through `__alltraps`, which is defined in `trap.S`. The
//! assembly language code does just enough work restore the kernel space
//! context, ensuring that Rust code safely runs, and transfers control to
//! [`trap_handler()`].
//!
//! It then calls different functionality based on what exactly the exception
//! was. For example, timer interrupts trigger task preemption, and syscalls go
//! to [`syscall()`].

mod context;

use crate::config::TRAP_CONTEXT_BASE;
use crate::syscall::syscall;
use crate::task::{
    check_signals_of_current, current_add_signal, current_process, current_trap_cx,
    current_trap_cx_user_va, current_user_token, exit_current_and_run_next,
    suspend_current_and_run_next, SignalFlags, INITPROC,
};
use crate::timer::{check_timer, set_next_trigger};
use core::arch::{asm, global_asm};
use riscv::register::sepc;
use riscv::register::{
    mtvec::TrapMode,
    scause::{self, Exception, Interrupt, Trap},
    sie, stval, stvec,
};

global_asm!(include_str!("trap.S"));
global_asm!(include_str!("init_entry.S"));

/// Initialize trap handling
pub fn init() {
    set_kernel_trap_entry();
}
/// set trap entry for traps happen in kernel(supervisor) mode
fn set_kernel_trap_entry() {
    extern "C" {
        fn __trap_from_kernel();
    }
    unsafe {
        stvec::write(__trap_from_kernel as usize, TrapMode::Direct);
    }
}
/// set trap entry for traps happen in user mode
fn set_user_trap_entry() {
    extern "C" {
        fn __alltraps();
    }
    unsafe {
        stvec::write(__alltraps as usize, TrapMode::Direct);
    }
}

/// enable timer interrupt in supervisor mode
pub fn enable_timer_interrupt() {
    unsafe {
        sie::set_stimer();
    }
}

/// trap handler
#[no_mangle]
pub fn trap_handler() -> ! {
    let mut sp: usize;
    unsafe {
        asm!("mv {}, sp", out(reg) sp);
    }
    set_kernel_trap_entry();
    let scause = scause::read();
    let stval = stval::read();
    // trace!("into {:?}", scause.cause());
    match scause.cause() {
        Trap::Exception(Exception::UserEnvCall) => {
            //进入内核态之前，计算用户态已运行的时间
            current_process()
                .inner_exclusive_access()
                .user_clock_time_end();

            // jump to next instruction anyway
            let mut cx = current_trap_cx();
            cx.sepc += 4;
            // get system call return value
            let result = syscall(
                cx.x[17],
                [cx.x[10], cx.x[11], cx.x[12], cx.x[13], cx.x[14], cx.x[15]],
            );
            // cx is changed during sys_exec, so we have to call it again
            cx = current_trap_cx();
            cx.x[10] = result as usize;
        }
        Trap::Exception(Exception::StoreFault)
        | Trap::Exception(Exception::StorePageFault)
        | Trap::Exception(Exception::InstructionFault)
        | Trap::Exception(Exception::InstructionPageFault)
        | Trap::Exception(Exception::LoadFault)
        | Trap::Exception(Exception::LoadPageFault) => {
            panic!(
                "[kernel] trap_handler: {:?} in application, bad addr = {:#x}, bad instruction = {:#x}, kernel killed it.",
                scause.cause(),
                stval,
                current_trap_cx().sepc,
            );
            current_add_signal(SignalFlags::SIGSEGV);
        }
        Trap::Exception(Exception::IllegalInstruction) => {
            current_add_signal(SignalFlags::SIGILL);
        }
        Trap::Interrupt(Interrupt::SupervisorTimer) => {
            set_next_trigger();
            check_timer();
            suspend_current_and_run_next();
        }
        _ => {
            panic!(
                "[kernel] trap_handler: unsupport trap {:?} , bad addr = {:#x}, bad instruction = {:#x}",
                scause.cause(),
                stval,
                current_trap_cx().sepc,
            );
        }
    }
    //check signals
    // if let Some((errno, msg)) = check_signals_of_current() {
    //     trace!("[kernel] trap_handler: .. check signals {}", msg);
    //     exit_current_and_run_next(errno);
    // }
    trap_return();
}

/// return to user space
#[no_mangle]
pub fn trap_return() -> ! {
    info!("trap_return");
    //disable_supervisor_interrupt();
    set_user_trap_entry();

    //从内核态返回后，计算内核态运行时间
    current_process()
        .inner_exclusive_access()
        .user_clock_time_start();

    let trap_cx_user_va: usize = current_trap_cx_user_va().into();
    let user_satp = current_user_token();
    extern "C" {
        fn __alltraps();
        fn __restore();
    }
    let restore_va = __restore as usize;
    // trace!("[kernel] trap_return: ..before return");
    unsafe {
        asm!(
            "fence.i",
            "jr {restore_va}",         // jump to new addr of __restore asm function
            restore_va = in(reg) restore_va,
            in("a0") trap_cx_user_va,      // a0 = virt addr of Trap Context
            in("a1") user_satp,        // a1 = phy addr of usr page table
            options(noreturn)
        );
    }
}

/// handle trap from kernel
#[no_mangle]
pub fn trap_from_kernel() -> ! {
    use riscv::register::sepc;
    error!("stval = {:#x}, sepc = {:#x}", stval::read(), sepc::read());
    panic!("a trap {:?} from kernel!", scause::read().cause());
}

#[no_mangle]
pub fn initproc_entry() -> ! {
    debug!("entering initproc");
    set_user_trap_entry();
    let trap_cx_user_va: usize = TRAP_CONTEXT_BASE;
    let user_satp = INITPROC.inner_exclusive_access().memory_set.token();
    debug!(
        "[kernel] initproc_entry, trap_cx_user_va = {:#x}, user_satp = {:#x}",
        trap_cx_user_va, user_satp
    );
    extern "C" {
        fn __init_entry();
    }
    let restore_va = __init_entry as usize;
    warn!("init satp to {:#x}", user_satp);
    unsafe {
        asm!(
            "fence.i",
            "jr {restore_va}",         // jump to new addr of __restore asm function
            restore_va = in(reg) restore_va,
            in("a0") trap_cx_user_va,      // a0 = virt addr of Trap Context
            in("a1") user_satp,        // a1 = phy addr of initproc page table
            options(noreturn)
        );
    }
}

#[no_mangle]
pub fn user_entry() -> ! {
    debug!("entering user app");
    set_user_trap_entry();
    let trap_cx_user_va: usize = current_trap_cx_user_va().into();
    let user_satp = current_user_token();
    debug!(
        "[kernel] user_entry, trap_cx_user_va = {:#x}, user_satp = {:#x}",
        trap_cx_user_va, user_satp
    );
    debug!(
        "[kernel] user_entry, sepc = {:#x}, sp = {:#x}",
        current_trap_cx().sepc,
        current_trap_cx().x[10]
    );
    extern "C" {
        fn __user_entry();
    }
    let entry_va = __user_entry as usize;
    warn!("reset satp to {:#x}", user_satp);
    unsafe {
        asm!(
            "fence.i",
            "jr {entry_va}",         // jump to new addr of __restore asm function
            entry_va = in(reg) entry_va,
            in("a0") trap_cx_user_va,      // a0 = virt addr of Trap Context
            in("a1") user_satp,        // a1 = phy addr of initproc page table
            options(noreturn)
        );
    }
}

pub use context::TrapContext;
