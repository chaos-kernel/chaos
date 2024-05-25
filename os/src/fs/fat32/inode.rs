use alloc::{string::String, sync::Arc, vec::Vec};
use spin::Mutex;

use crate::{block::{block_cache::get_block_cache, BLOCK_SZ}, fs::{efs::BlockDevice, fat32::dentry::{self, Fat32Dentry, Fat32DentryLayout}, inode::Inode}};

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
        let fs = self.fs.lock();
        let cluster_chain = fs.cluster_chain(self.start_cluster);
        let mut v = Vec::new();
        let mut finished = false;
        for cluster in cluster_chain {
            let start_sector_id = (fs.sb.root_sector() + (cluster - 2) * fs.sb.sectors_per_cluster as u32) as usize;
            for i in 0..fs.sb.sectors_per_cluster as usize {
                let sector_id = start_sector_id + i;
                let dentrys_per_sector = BLOCK_SZ / 32;
                for j in 0..dentrys_per_sector {
                    let offset = (j * 32) as usize;
                    debug!("sector_id: {}, offset: {}", sector_id, offset);
                    get_block_cache(sector_id, Arc::clone(&fs.bdev))
                        .lock()
                        .read(offset, | layout: &Fat32DentryLayout | {
                            if let Some(dentry) = Fat32Dentry::from_layout(layout) {
                                if dentry.is_file() {
                                    v.push(dentry.name());
                                }
                            } else {
                                finished = true;
                            }
                        });
                    if finished {
                        return v;
                    }
                }
            }
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