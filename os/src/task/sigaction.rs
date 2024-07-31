use super::signal::{SaFlags, MAX_SIG, SIG_DFL};
use crate::task::SignalFlags;

/// Action for a signal
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SignalAction {
    pub sa_handler:  usize,
    pub sa_flags:    SaFlags,
    pub sa_restorer: usize,
    pub mask:        SignalFlags,
}

impl Default for SignalAction {
    fn default() -> Self {
        Self {
            sa_handler:  SIG_DFL,
            sa_flags:    SaFlags::empty(),
            sa_restorer: 0,
            mask:        SignalFlags::from_bits(40).unwrap(),
        }
    }
}

#[derive(Clone)]
pub struct SignalActions {
    pub table: [SignalAction; MAX_SIG + 1],
}

impl Default for SignalActions {
    fn default() -> Self {
        Self {
            table: [SignalAction::default(); MAX_SIG + 1],
        }
    }
}
