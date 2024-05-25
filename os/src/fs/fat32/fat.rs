use alloc::sync::Arc;

use crate::{block::{block_cache::get_block_cache, BLOCK_SZ}, fs::efs::BlockDevice};

use super::super_block::Fat32SB;

pub struct FAT {
    pub start_sector: u32,
}

impl FAT {
    pub fn from_sb(sb: &Fat32SB) -> Self {
        Self {
            start_sector: sb.reserved_sectors_cnt as u32,
        }
    }
    
    /// get next cluster number
    pub fn next_cluster_id(&self, cluster: u32, bdev: &Arc<dyn BlockDevice>) -> u32 {
        let fat_offset = self.start_sector * BLOCK_SZ as u32 + cluster * 4;
        let fat_sector = fat_offset / BLOCK_SZ as u32;
        let fat_offset_in_sector = fat_offset % BLOCK_SZ as u32;
        let mut next_cluster = 0;
        get_block_cache(fat_sector as usize, Arc::clone(bdev))
            .lock()
            .read(fat_offset_in_sector as usize, |data: &[u8; 4]| {
                next_cluster = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
            });
        next_cluster
    }

    #[allow(unused)]
    /// cluster id to sector id
    pub fn cluster_id_to_sector_id(&self, cluster: u32, sb: &Fat32SB) -> Option<u32> {
        if cluster < 2 {
            return None;
        }
        let res = sb.root_sector() + (cluster - 2) * sb.sectors_per_cluster as u32;
        Some(res)
    }
}

