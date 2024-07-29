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

use core::arch::{asm, global_asm};

use riscv::register::{
    mtvec::TrapMode,
    scause::{self, Exception, Interrupt, Trap},
    sepc,
    sie,
    stval,
    stvec,
};

use crate::{
    config::TRAP_CONTEXT_BASE,
    syscall::{self, syscall},
    task::{
        check_signals_of_current,
        current_add_signal,
        current_task,
        current_trap_cx,
        current_trap_cx_user_va,
        current_user_token,
        exit_current_and_run_next,
        suspend_current_and_run_next,
        SignalFlags,
        INITPROC,
    },
    timer::{check_timer, set_next_trigger},
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
    set_kernel_trap_entry();
    let scause = scause::read();
    let stval = stval::read();
    let mut syscall_num = -1;
    // trace!("into {:?}", scause.cause());
    match scause.cause() {
        Trap::Exception(Exception::UserEnvCall) => {
            //进入内核态之前，计算用户态已运行的时间
            current_task()
                .unwrap()
                .inner_exclusive_access(file!(), line!())
                .user_clock_time_end();

            // jump to next instruction anyway
            let mut cx = current_trap_cx();
            cx.sepc += 4;
            syscall_num = cx.x[17] as i32;
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
            error!(
                "[kernel] trap_handler: {:?} in application, bad addr = {:#x}, bad instruction = \
                 {:#x}, kernel killed it.",
                scause.cause(),
                stval,
                current_trap_cx().sepc,
            );
            current_add_signal(SignalFlags::SIGSEGV);
        }
        Trap::Exception(Exception::IllegalInstruction) => {
            exit_current_and_run_next(-1);
            current_add_signal(SignalFlags::SIGILL);
        }
        Trap::Interrupt(Interrupt::SupervisorTimer) => {
            set_next_trigger();
            check_timer();
            suspend_current_and_run_next();
        }
        _ => {
            panic!(
                "[kernel] trap_handler: unsupport trap {:?} , bad addr = {:#x}, bad instruction = \
                 {:#x}",
                scause.cause(),
                stval,
                current_trap_cx().sepc,
            );
        }
    }
    //check signals
    if let Some((errno, msg)) = check_signals_of_current() {
        trace!("[kernel] trap_handler: .. check signals {}", msg);
        exit_current_and_run_next(errno);
    }

    match scause.cause() {
        Trap::Exception(Exception::UserEnvCall) => {
            if syscall_num == syscall::SYSCALL_EXECVE as i32 {
                match current_task().unwrap().pid.0 {
                    0 => initproc_entry(),
                    _ => user_entry(),
                }
            } else {
                trap_return();
            }
        }
        _ => {
            trap_return();
        }
    }
    panic!("[kernel] trap_handler: unreachable code");
}

/// return to user space
#[no_mangle]
pub fn trap_return() -> ! {
    info!("trap_return");
    //disable_supervisor_interrupt();
    set_user_trap_entry();

    //从内核态返回后，计算内核态运行时间
    current_task()
        .unwrap()
        .inner_exclusive_access(file!(), line!())
        .user_clock_time_start();

    let trap_cx_user_va: usize = current_trap_cx_user_va().into();
    let user_satp = current_user_token();
    // warn!(
    //     "[kernel] user_entry, trap_cx_user_va = {:#x}, user_satp = {:#x}",
    //     trap_cx_user_va, user_satp
    // );
    // warn!(
    //     "[kernel] user_entry, sepc = {:#x}, sp = {:#x}",
    //     current_trap_cx().sepc,
    //     current_trap_cx().x[10]
    // );
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
    error!("stval = {:#x}, sepc = {:#x}", stval::read(), sepc::read());
    panic!("a trap {:?} from kernel!", scause::read().cause());
}

#[no_mangle]
pub fn initproc_entry() -> ! {
    debug!("entering initproc");
    set_user_trap_entry();
    let trap_cx_user_va: usize = current_trap_cx_user_va().into();
    let user_satp = INITPROC
        .inner_exclusive_access(file!(), line!())
        .memory_set
        .token();
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
    info!("entering user app");
    set_user_trap_entry();
    let trap_cx_user_va: usize = current_trap_cx_user_va().into();
    let user_satp = current_user_token();
    debug!(
        "[kernel] user_entry, trap_cx_user_va = {:#x}, user_satp = {:#x}",
        trap_cx_user_va, user_satp
    );
    // debug!(
    //     "[kernel] user_entry, at: {:#x}, sepc = {:#x}, sp = {:#x}",
    //     current_task().unwrap().trap_cx_user_va().0,
    //     current_trap_cx().sepc,
    //     current_trap_cx().x[10]
    // );
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
