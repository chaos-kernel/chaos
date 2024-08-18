/// Infinity for RLimit
pub const RLIM_INFINITY: usize = usize::MAX;

#[allow(unused)]
const RLIMIT_CPU: u32 = 0;
#[allow(unused)]
const RLIMIT_FSIZE: u32 = 1;
#[allow(unused)]
const RLIMIT_DATA: u32 = 2;
#[allow(unused)]
const RLIMIT_STACK: u32 = 3;
#[allow(unused)]
const RLIMIT_CORE: u32 = 4;
#[allow(unused)]
const RLIMIT_RSS: u32 = 5;
#[allow(unused)]
const RLIMIT_NPROC: u32 = 6;
#[allow(unused)]
const RLIMIT_NOFILE: u32 = 7;
#[allow(unused)]
const RLIMIT_MEMLOCK: u32 = 8;
#[allow(unused)]
const RLIMIT_AS: u32 = 9;
#[allow(unused)]
const RLIMIT_LOCKS: u32 = 10;
#[allow(unused)]
const RLIMIT_SIGPENDING: u32 = 11;
#[allow(unused)]
const RLIMIT_MSGQUEUE: u32 = 12;
#[allow(unused)]
const RLIMIT_NICE: u32 = 13;
#[allow(unused)]
const RLIMIT_RTPRIO: u32 = 14;
#[allow(unused)]
const RLIMIT_RTTIME: u32 = 15;

/// Resource Limit
#[derive(Debug, Clone, Copy)]
pub struct RLimit {
    /// Soft limit
    pub rlim_cur: usize,
    /// Hard limit (ceiling for rlim_cur)
    pub rlim_max: usize,
}

impl RLimit {
    /// New a RLimit
    pub fn new(cur: usize, max: usize) -> Self {
        Self {
            rlim_cur: cur,
            rlim_max: max,
        }
    }
    /// Set RLimit
    pub fn set_rlimit(resource: u32, rlimit: &RLimit) -> isize {
        log::info!("[set_rlimit] try to set limit: {:?}", resource);
        match resource {
            RLIMIT_NOFILE => {
                current_process().inner_handler(|proc| proc.fd_table.set_rlimit(*rlimit))
            }
            _ => {}
        }
        0
    }
    /// Get RLimit
    pub fn get_rlimit(resource: u32) -> Self {
        match resource {
            RLIMIT_STACK => Self::new(USER_STACK_SIZE, RLIM_INFINITY),
            RLIMIT_NOFILE => current_process().inner_handler(|proc| proc.fd_table.rlimit()),
            _ => Self {
                rlim_cur: 0,
                rlim_max: 0,
            },
        }
    }
}
