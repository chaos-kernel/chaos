use alloc::{string::String, sync::Arc, vec::Vec};
use spin::Mutex;

use crate::{block::{block_cache::get_block_cache, BLOCK_SZ}, fs::{efs::BlockDevice, fat32::dentry::{Fat32Dentry, Fat32DentryLayout, Fat32LDentryLayout}, inode::Inode}};

use super::file_system::Fat32FS;

pub struct Fat32Inode {
    pub start_cluster: u32,
    pub fs: Arc<Mutex<Fat32FS>>,
    pub bdev: Arc<dyn BlockDevice>,
}

impl Inode for Fat32Inode {
    fn fstat(&self) -> (usize, u32) {
        todo!()
    }
    
    fn find(&self, name: &str) -> Option<alloc::sync::Arc<dyn Inode>> {
        todo!()
    }
    
    fn create(&self, name: &str) -> Option<alloc::sync::Arc<dyn Inode>> {
        todo!()
    }
    
    fn link(&self, old_name: &str, new_name: &str) -> Option<alloc::sync::Arc<dyn Inode>> {
        todo!()
    }
    
    fn unlink(&self, name: &str) -> bool {
        todo!()
    }
    
    fn ls(&self) -> Vec<String> {
        debug!("ls");
        let fs = self.fs.lock();
        let mut v = Vec::new();
        let mut sector_id = fs.fat.cluster_id_to_sector_id(self.start_cluster).unwrap();
        let mut offset = 0;
        while let Some(dentry) = fs.get_dentry(&mut sector_id, &mut offset) {
            debug!("ls: {}", dentry.name());
            v.push(dentry.name());
        }
        v
    }
    
    fn read_at(&self, offset: usize, buf: &mut [u8]) -> usize {
        todo!()
    }
    
    fn write_at(&self, offset: usize, buf: &[u8]) -> usize {
        todo!()
    }
    
    fn clear(&self) {
        todo!()
    } 
}