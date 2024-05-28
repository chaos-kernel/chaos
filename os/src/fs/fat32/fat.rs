use alloc::sync::Arc;

use crate::block::{block_cache::get_block_cache, block_dev::BlockDevice, BLOCK_SZ};

use super::super_block::Fat32SB;

pub struct FAT {
    pub start_sector: u32,
    pub sb: Arc<Fat32SB>,
}

impl FAT {
    pub fn from_sb(sb: Arc<Fat32SB>) -> Self {
        Self {
            start_sector: sb.reserved_sectors_cnt as u32,
            sb,
        }
    }

    /// allocate a new cluster
    pub fn alloc_new_cluster(&self, bdev: &Arc<dyn BlockDevice>) -> Option<u32> {
        let mut offset = self.start_sector * BLOCK_SZ as u32 + 3 * 4;
        let mut cluster_id = 0;
        loop {
            let fat_sector = offset / BLOCK_SZ as u32;
            let offset_in_sector = offset % BLOCK_SZ as u32;
            get_block_cache(fat_sector as usize, Arc::clone(bdev))
                .lock()
                .read(offset_in_sector as usize, |data: &[u8; 4]| {
                    let num = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                    if num == 0 {
                        cluster_id = (offset - self.start_sector * BLOCK_SZ as u32) / 4;
                    }
                });
            if cluster_id != 0 {
                break;
            }
            offset += 4;
        }
        Some(cluster_id)
    }
    
    /// get next cluster number
    pub fn next_cluster_id(&self, cluster: u32, bdev: &Arc<dyn BlockDevice>) -> Option<u32> {
        let fat_offset = self.start_sector * BLOCK_SZ as u32 + cluster * 4;
        let fat_sector = fat_offset / BLOCK_SZ as u32;
        let fat_offset_in_sector = fat_offset % BLOCK_SZ as u32;
        let mut next_cluster = 0;
        get_block_cache(fat_sector as usize, Arc::clone(bdev))
            .lock()
            .read(fat_offset_in_sector as usize, |data: &[u8; 4]| {
                next_cluster = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
            });
        if next_cluster >= 0x0FFFFFF8 {
            return None;
        } else {
            return Some(next_cluster);
        }
    }

    #[allow(unused)]
    /// cluster id to sector id
    pub fn cluster_id_to_sector_id(&self, cluster: u32) -> Option<u32> {
        if cluster < 2 {
            return None;
        }
        let res = self.sb.root_sector() + (cluster - 2) * self.sb.sectors_per_cluster as u32;
        Some(res)
    }

    #[allow(unused)]
    /// sector id to cluster id
    pub fn sector_id_to_cluster_id(&self, sector: u32) -> Option<u32> {
        if sector < self.sb.root_sector() {
            return None;
        }
        let res = (sector - self.sb.root_sector()) / self.sb.sectors_per_cluster as u32 + 2;
        Some(res)
    }
}

