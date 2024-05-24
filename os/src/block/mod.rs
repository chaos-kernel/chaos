//! Block device and block cache module
pub mod block_dev;
pub mod block_cache;

/// Block size in bytes
pub const BLOCK_SZ: usize = 512;