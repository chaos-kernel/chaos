use alloc::vec::Vec;

use ext4_rs::{BlockDevice, BLOCK_SIZE};
use spin::Mutex;
use visionfive2_sd::*;

use crate::{
    block::BLOCK_SZ,
    timer::{sleep_ms, sleep_ms_until},
};

pub struct SdIoImpl;
pub const SDIO_BASE: usize = 0xffffffc016020000;

impl SDIo for SdIoImpl {
    fn read_data_at(&self, offset: usize) -> u64 {
        let addr = (SDIO_BASE + offset) as *mut u64;
        unsafe { addr.read_volatile() }
    }
    fn read_reg_at(&self, offset: usize) -> u32 {
        let addr = (SDIO_BASE + offset) as *mut u32;
        unsafe { addr.read_volatile() }
    }
    fn write_data_at(&mut self, offset: usize, val: u64) {
        let addr = (SDIO_BASE + offset) as *mut u64;
        unsafe { addr.write_volatile(val) }
    }
    fn write_reg_at(&mut self, offset: usize, val: u32) {
        let addr = (SDIO_BASE + offset) as *mut u32;
        unsafe { addr.write_volatile(val) }
    }
}

pub struct SleepOpsImpl;

impl SleepOps for SleepOpsImpl {
    fn sleep_ms(ms: usize) {
        sleep_ms(ms);
    }
    fn sleep_ms_until(ms: usize, f: impl FnMut() -> bool) {
        sleep_ms_until(ms, f);
    }
}

pub struct SDCard(Mutex<Vf2SdDriver<SdIoImpl, SleepOpsImpl>>);

impl SDCard {
    pub fn new() -> Self {
        debug!("SDCard::new()");
        let mut sd = Vf2SdDriver::<_, SleepOpsImpl>::new(SdIoImpl);
        sd.init();
        Self(Mutex::new(sd))
    }
}

impl BlockDevice for SDCard {
    fn read_offset(&self, offset: usize) -> Vec<u8> {
        let mut buf = [0u8; BLOCK_SIZE];
        let mut block_id = offset / BLOCK_SZ;
        let mut buf_offset = 0;
        for _ in 0..8 {
            self.0
                .lock()
                .read_block(block_id, buf[buf_offset..buf_offset + BLOCK_SZ].as_mut());
            block_id += 1;
            buf_offset += BLOCK_SZ;
        }
        buf[offset % BLOCK_SIZE..].to_vec()
    }
    fn write_offset(&self, offset: usize, data: &[u8]) {
        let mut block_id = offset / BLOCK_SZ;
        let mut data_offset = 0;
        for _ in 0..8 {
            self.0
                .lock()
                .write_block(block_id, data[data_offset..data_offset + BLOCK_SZ].as_ref());
            block_id += 1;
            data_offset += BLOCK_SZ;
        }
    }
}
