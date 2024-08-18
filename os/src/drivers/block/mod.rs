//! virtio_blk device driver

mod vf2_sd;
mod virtio_blk;

use alloc::sync::Arc;

use lazy_static::*;
pub use virtio_blk::VirtIOBlock;

use crate::{block::block_dev::BlockDevice, board::BlockDeviceImpl};

lazy_static! {
    /// The global block device driver instance: BLOCK_DEVICE with BlockDevice trait
    pub static ref BLOCK_DEVICE: Arc<dyn ext4_rs::BlockDevice> = Arc::new(BlockDeviceImpl::new());
}

#[allow(unused)]
/// Test the block device
pub fn block_device_test() {
    let block_device = BLOCK_DEVICE.clone();
    let mut write_buffer = [0u8; 512];
    let mut read_buffer = [0u8; 512];
    for i in 0..512 {
        for byte in write_buffer.iter_mut() {
            *byte = i as u8;
        }
        // block_device.write_block(i as usize, &write_buffer);
        // block_device.read_block(i as usize, &mut read_buffer);
        assert_eq!(write_buffer, read_buffer);
    }
    println!("block device test passed!");
}
