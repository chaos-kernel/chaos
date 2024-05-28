//! Signal flags and function for convert signal flag to integer & string

use bitflags::*;

bitflags! {
    /// Signal flags
    pub struct SignalFlags: usize {
        /// Hangup.
        const	SIGHUP		= 1 << ( 0);
        /// Interactive attention signal.
        const	SIGINT		= 1 << ( 1);
        /// Quit.
        const	SIGQUIT		= 1 << ( 2);
        /// Illegal instruction.
        const	SIGILL		= 1 << ( 3);
        /// Trace/breakpoint trap.
        const	SIGTRAP		= 1 << ( 4);
        /// IOT instruction, abort() on a PDP-11.
        const	SIGABRT		= 1 << ( 5);
        /// Bus error.
        const	SIGBUS		= 1 << ( 6);
        /// Erroneous arithmetic operation.
        const	SIGFPE		= 1 << ( 7);
        /// Killed.
        const	SIGKILL		= 1 << ( 8);
        /// User-defined signal 1.
        const	SIGUSR1		= 1 << ( 9);
        /// Invalid access to storage.
        const	SIGSEGV		= 1 << (10);
        /// User-defined signal 2.
        const	SIGUSR2		= 1 << (11);
        /// Broken pipe.
        const	SIGPIPE		= 1 << (12);
        /// Alarm clock.
        const	SIGALRM		= 1 << (13);
        /// Termination request.
        const	SIGTERM		= 1 << (14);
        const	SIGSTKFLT	= 1 << (15);
        /// Child terminated or stopped.
        const	SIGCHLD		= 1 << (16);
        /// Continue.
        const	SIGCONT		= 1 << (17);
        /// Stop, unblockable.
        const	SIGSTOP		= 1 << (18);
        /// Keyboard stop.
        const	SIGTSTP		= 1 << (19);
        /// Background read from control terminal.
        const	SIGTTIN		= 1 << (20);
        /// Background write to control terminal.
        const	SIGTTOU		= 1 << (21);
        /// Urgent data is available at a socket.
        const	SIGURG		= 1 << (22);
        /// CPU time limit exceeded.
        const	SIGXCPU		= 1 << (23);
        /// File size limit exceeded.
        const	SIGXFSZ		= 1 << (24);
        /// Virtual timer expired.
        const	SIGVTALRM	= 1 << (25);
        /// Profiling timer expired.
        const	SIGPROF		= 1 << (26);
        /// Window size change (4.3 BSD, Sun).
        const	SIGWINCH	= 1 << (27);
        /// I/O now possible (4.2 BSD).
        const	SIGIO		= 1 << (28);
        const   SIGPWR      = 1 << (29);
        /// Bad system call.
        const   SIGSYS      = 1 << (30);
        /* --- realtime signals for pthread --- */
        const   SIGTIMER    = 1 << (31);
        const   SIGCANCEL   = 1 << (32);
        const   SIGSYNCCALL = 1 << (33);
        /* --- other realtime signals --- */
        const   SIGRT_3     = 1 << (34);
        const   SIGRT_4     = 1 << (35);
        const   SIGRT_5     = 1 << (36);
        const   SIGRT_6     = 1 << (37);
        const   SIGRT_7     = 1 << (38);
        const   SIGRT_8     = 1 << (39);
        const   SIGRT_9     = 1 << (40);
        const   SIGRT_10    = 1 << (41);
        const   SIGRT_11    = 1 << (42);
        const   SIGRT_12    = 1 << (43);
        const   SIGRT_13    = 1 << (44);
        const   SIGRT_14    = 1 << (45);
        const   SIGRT_15    = 1 << (46);
        const   SIGRT_16    = 1 << (47);
        const   SIGRT_17    = 1 << (48);
        const   SIGRT_18    = 1 << (49);
        const   SIGRT_19    = 1 << (50);
        const   SIGRT_20    = 1 << (51);
        const   SIGRT_21    = 1 << (52);
        const   SIGRT_22    = 1 << (53);
        const   SIGRT_23    = 1 << (54);
        const   SIGRT_24    = 1 << (55);
        const   SIGRT_25    = 1 << (56);
        const   SIGRT_26    = 1 << (57);
        const   SIGRT_27    = 1 << (58);
        const   SIGRT_28    = 1 << (59);
        const   SIGRT_29    = 1 << (60);
        const   SIGRT_30    = 1 << (61);
        const   SIGRT_31    = 1 << (62);
        const   SIGRTMAX    = 1 << (63);
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
