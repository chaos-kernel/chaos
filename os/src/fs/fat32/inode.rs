use core::cmp::min;

use alloc::{string::String, sync::Arc, vec::Vec};
use spin::Mutex;

use crate::fs::{efs::BlockDevice, fat32::CLUSTER_SIZE, inode::Inode};

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
        debug!("find: {}", name);
        let fs = self.fs.lock();
        let mut sector_id = fs.fat.cluster_id_to_sector_id(self.start_cluster).unwrap();
        let mut offset = 0;
        while let Some(dentry) = fs.get_dentry(&mut sector_id, &mut offset) {
            if dentry.name() == name {
                let inode = Fat32Inode {
                    start_cluster: dentry.start_cluster(),
                    fs: Arc::clone(&self.fs),
                    bdev: Arc::clone(&self.bdev),
                };
                return Some(Arc::new(inode));
            }
        }
        None
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
        let fs = self.fs.lock();
        let cluster_id = self.start_cluster;
        let cluster_chain = fs.cluster_chain(cluster_id);
        let mut read_size = 0;
        let mut pos = 0;
        let mut cluster_buf = [0u8; CLUSTER_SIZE];
        for cluster_id in cluster_chain {
            if pos < offset {
                pos += min(CLUSTER_SIZE, offset - pos);
                if pos < offset {
                    continue;
                }
            }
            fs.read_cluster(cluster_id, &mut cluster_buf);
            let copy_size = min(CLUSTER_SIZE - pos % CLUSTER_SIZE, buf.len() - read_size);
            buf[read_size..read_size + copy_size].copy_from_slice(&cluster_buf[pos % CLUSTER_SIZE..pos % CLUSTER_SIZE + copy_size]);
            read_size += copy_size;
            pos += copy_size;
            if read_size >= buf.len() {
                break;
            }
        }
        read_size
    }
    
    fn write_at(&self, offset: usize, buf: &[u8]) -> usize {
        todo!()
    }
    
    fn clear(&self) {
        todo!()
    } 
}