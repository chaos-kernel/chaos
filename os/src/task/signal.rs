//! Signal flags and function for convert signal flag to integer & string

use bitflags::*;

bitflags! {
    /// Signal flags
    pub struct SignalFlags: u32 {
        /// Interrupt
        const SIGINT    = 1 << 2;
        /// Illegal instruction
        const SIGILL    = 1 << 4;
        /// Abort
        const SIGABRT   = 1 << 6;
        /// Floating point exception
        const SIGFPE    = 1 << 8;
        /// Segmentation fault
        const SIGSEGV   = 1 << 11;
    }
}

impl SignalFlags {
    /// convert signal flag to integer & string
    pub fn check_error(&self) -> Option<(i32, &'static str)> {
        if self.contains(Self::SIGINT) {
            Some((-2, "Killed, SIGINT=2"))
        } else if self.contains(Self::SIGILL) {
            Some((-4, "Illegal Instruction, SIGILL=4"))
        } else if self.contains(Self::SIGABRT) {
            Some((-6, "Aborted, SIGABRT=6"))
        } else if self.contains(Self::SIGFPE) {
            Some((-8, "Erroneous Arithmetic Operation, SIGFPE=8"))
        } else if self.contains(Self::SIGSEGV) {
            Some((-11, "Segmentation Fault, SIGSEGV=11"))
        } else {
            // warn!("[kernel] signalflags check_error  {:?}", self);
            None
        }
    }
}
