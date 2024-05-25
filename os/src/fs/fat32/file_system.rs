use alloc::{string::String, sync::Arc, vec::Vec};
use spin::Mutex;

use crate::{block::{block_cache::get_block_cache, BLOCK_SZ}, fs::efs::BlockDevice};

use super::{dentry::{Fat32Dentry, Fat32DentryLayout, Fat32LDentryLayout}, fat::FAT, inode::Fat32Inode, super_block::{Fat32SB, Fat32SBLayout}};

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
                    fat: FAT::from_sb(Arc::new(Fat32SB::from_layout(sb_layout))),
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
        loop {
            cluster_chain.push(cluster);
            if let Some(next_cluster) = self.fat.next_cluster_id(cluster, &self.bdev) {
                cluster = next_cluster;
            } else {
                break;
            }
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
                .read(0, |data: &[u8; BLOCK_SZ]| {
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
                .modify(0, |data: &mut [u8; BLOCK_SZ]| {
                    let copy_size = core::cmp::min(cluster_size - write_size, data.len());
                    data[..copy_size].copy_from_slice(&buf[write_size..write_size + copy_size]);
                    write_size += copy_size;
                });
        }
    }

    /// get next dentry sector id and offset
    fn next_dentry_id(&self, sector_id: u32, offset: usize) -> Option<(u32, usize)> {
        if offset >= 512 || offset % 32 != 0 {
            return None;
        }
        let next_offset = offset + 32;
        if next_offset >= 512 {
            let next_sector_id = sector_id + 1;
            if next_sector_id % self.sb.sectors_per_cluster as u32 == 0 {
                if let Some(next_sector_id) = self.fat.next_cluster_id(sector_id, &self.bdev) {
                    Some((next_sector_id, 0))
                } else {
                    None
                }
            } else {
                Some((next_sector_id, 0))
            }
        } else {
            Some((sector_id, next_offset))
        }
    }

    /// get a dentry with sector id and offset
    pub fn get_dentry(&self, sector_id: &mut u32, offset: &mut usize) -> Option<Fat32Dentry> {
        if *offset >= 512 || *offset % 32 != 0 {
            return None;
        }
        let mut is_long_entry = false;
        let dentry = get_block_cache(*sector_id as usize, Arc::clone(&self.bdev))
            .lock()
            .read(*offset, |layout: &Fat32DentryLayout| {
                if layout.is_long() {
                    is_long_entry = true;
                    return None;
                }
                Fat32Dentry::from_layout(layout)
            });
        if is_long_entry {
            let mut name = String::new();
            let mut is_end = false;
            loop {
                get_block_cache(*sector_id as usize, Arc::clone(&self.bdev))
                    .lock()
                    .read(*offset, |layout: &Fat32LDentryLayout| {
                        name.push_str(&layout.name());
                        if layout.is_end() {
                            is_end = true;
                        }
                    });
                (*sector_id, *offset) = self.next_dentry_id(*sector_id, *offset).unwrap();
                if is_end {
                    break;
                }
            }
            if let Some(mut dentry) = get_block_cache(*sector_id as usize, Arc::clone(&self.bdev))
                .lock()
                .read(*offset, |layout: &Fat32DentryLayout| {
                    Fat32Dentry::from_layout(layout)
                })
            {
                dentry.set_name(name);
                (*sector_id, *offset) = self.next_dentry_id(*sector_id, *offset).unwrap();
                return Some(dentry);
            } else {
                None
            }
        } else {
            (*sector_id, *offset) = self.next_dentry_id(*sector_id, *offset).unwrap();
            dentry
        }
    }
    
}