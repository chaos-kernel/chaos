use alloc::vec::Vec;

use crate::block::{block_dev::BlockDevice, BLOCK_SZ};

mod defs;
pub mod fs;
pub mod inode;

impl ext4_rs::BlockDevice for dyn BlockDevice {
    fn read_offset(&self, offset: usize) -> Vec<u8> {
        let mut buf = [0u8; BLOCK_SZ];
        self.read_block(offset / BLOCK_SZ, &mut buf);
        buf.to_vec()
    }
    fn write_offset(&self, offset: usize, data: &[u8]) {
        self.write_block(offset / BLOCK_SZ, data);
    }
}
