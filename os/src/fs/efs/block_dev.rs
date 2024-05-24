//! Block device interface.
//!
//! Define the block read-write interface [BlockDevice] that the device driver needs to implement

use core::any::Any;

pub trait BlockDevice: Send + Sync + Any {
    /// Read a block from the block device.
    fn read_block(&self, block_id: usize, buf: &mut [u8]);
    /// Write a block to the block device.
    fn write_block(&self, block_id: usize, buf: &[u8]);
}
