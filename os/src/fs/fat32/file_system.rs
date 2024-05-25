use alloc::{sync::Arc, vec::Vec};
use spin::Mutex;

use crate::{block::block_cache::get_block_cache, fs::efs::BlockDevice};

use super::{fat::FAT, inode::Fat32Inode, super_block::{Fat32SB, Fat32SBLayout}};

pub struct Fat32FS {
    pub sb: Fat32SB,
    pub fat: FAT,
    pub bdev: Arc<dyn BlockDevice>,
}

impl Fat32FS {
    /// load a exist fat32 file system from block device
    pub fn load(bdev: Arc<dyn BlockDevice>) -> Arc<Mutex<Self>> {
        get_block_cache(0, Arc::clone(&bdev))
            .lock()
            .read(0, |sb_layout: &Fat32SBLayout| {
                assert!(sb_layout.is_valid(), "Error loading FAT32!");
                let fat32fs = Self {
                    sb: Fat32SB::from_layout(sb_layout),
                    fat: FAT::from_sb(&Fat32SB::from_layout(sb_layout)),
                    bdev,
                };
                Arc::new(Mutex::new(fat32fs))
            })
    }

    /// get root inode
    pub fn root_inode(fs: &Arc<Mutex<Fat32FS>>) -> Fat32Inode {
        let fs_ = fs.lock();
        let start_cluster = fs_.sb.root_cluster;
        let bdev = Arc::clone(&fs_.bdev);
        drop(fs_);
        Fat32Inode {
            start_cluster,
            fs: Arc::clone(fs),
            bdev,
        }
    }

    /// get cluster chain
    pub fn cluster_chain(&self, start_cluster: u32) -> Vec<u32> {
        let mut cluster_chain = Vec::new();
        let mut cluster = start_cluster;
        while cluster < 0x0FFFFFF8 {
            cluster_chain.push(cluster);
            cluster = self.fat.next_cluster_id(cluster, &self.bdev);
        }
        cluster_chain
    }

    /// read a cluster
    pub fn read_cluster(&self, cluster: u32, buf: &mut [u8; 4096]) {
        let cluster_offset = self.sb.root_sector() + (cluster - 2) * self.sb.sectors_per_cluster as u32;
        let cluster_size = self.sb.bytes_per_sector as usize * self.sb.sectors_per_cluster as usize;
        let mut read_size = 0;
        for i in 0..self.sb.sectors_per_cluster {
            get_block_cache(cluster_offset as usize + i as usize, Arc::clone(&self.bdev))
                .lock()
                .read(0, |data: &[u8; 4096]| {
                    let copy_size = core::cmp::min(cluster_size - read_size, data.len());
                    buf[read_size..read_size + copy_size].copy_from_slice(&data[..copy_size]);
                    read_size += copy_size;
                });
        }
    }

    /// write a cluster
    pub fn write_cluster(&self, cluster: u32, buf: &[u8; 4096]) {
        let cluster_offset = self.sb.root_sector() + (cluster - 2) * self.sb.sectors_per_cluster as u32;
        let cluster_size = self.sb.bytes_per_sector as usize * self.sb.sectors_per_cluster as usize;
        let mut write_size = 0;
        for i in 0..self.sb.sectors_per_cluster {
            get_block_cache(cluster_offset as usize + i as usize, Arc::clone(&self.bdev))
                .lock()
                .modify(0, |data: &mut [u8; 4096]| {
                    let copy_size = core::cmp::min(cluster_size - write_size, data.len());
                    data[..copy_size].copy_from_slice(&buf[write_size..write_size + copy_size]);
                    write_size += copy_size;
                });
        }
    }


    
}