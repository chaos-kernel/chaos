use core::fmt::Debug;

/// the super block of a fat32 file system
pub struct Fat32SB {
    pub bytes_per_sector: u16,
    pub sectors_per_cluster: u8,
    pub reserved_sectors_cnt: u16,
    pub fat_cnt: u8,
    pub total_sectors_32: u32,
    pub fat_size_32: u32,
    pub root_cluster: u32,
}

impl Fat32SB {
    pub fn from_layout(layout: &Fat32SBLayout) -> Self {
        Self {
            bytes_per_sector: layout.bytes_per_sector[0] as u16 | (layout.bytes_per_sector[1] as u16) << 8,
            sectors_per_cluster: layout.sectors_per_cluster,
            reserved_sectors_cnt: layout.reserved_sectors_cnt,
            fat_cnt: layout.fat_cnt,
            total_sectors_32: layout.total_sectors_32,
            fat_size_32: layout.fat_size_32,
            root_cluster: layout.root_cluster,
        }
    }
}

impl Fat32SB {
    pub fn root_sector(&self) -> usize {
        let res = self.reserved_sectors_cnt as usize + self.fat_cnt as usize * self.fat_size_32 as usize;
        res
    }
}

impl From<Fat32SBLayout> for Fat32SB {
    fn from(sb_layout: Fat32SBLayout) -> Self {
        Self {
            bytes_per_sector: sb_layout.bytes_per_sector[0] as u16 | (sb_layout.bytes_per_sector[1] as u16) << 8,
            sectors_per_cluster: sb_layout.sectors_per_cluster,
            reserved_sectors_cnt: sb_layout.reserved_sectors_cnt,
            fat_cnt: sb_layout.fat_cnt,
            total_sectors_32: sb_layout.total_sectors_32,
            fat_size_32: sb_layout.fat_size_32,
            root_cluster: sb_layout.root_cluster,
        }
    }
}

/// the super block layout of a fat32 file system
#[repr(C)]
#[derive(Debug)]
pub struct Fat32SBLayout {
    pub jump_code: [u8; 3],
    pub oem_name: [u8; 8],
    pub bytes_per_sector: [u8; 2],
    pub sectors_per_cluster: u8,
    pub reserved_sectors_cnt: u16,
    pub fat_cnt: u8,
    pub root_entry_cnt: [u8; 2],
    pub total_sectors_16: [u8; 2],
    pub media_type: u8,
    pub fat_size_16: u16,
    pub sectors_per_track: u16,
    pub head_cnt: u16,
    pub hidden_sectors: u32,
    pub total_sectors_32: u32,
    pub fat_size_32: u32,
    pub ext_flags: u16,
    pub fs_version: u16,
    pub root_cluster: u32,
    pub fs_info: u16,
    pub backup_boot_sector: u16,
    pub reserved_0: [u8; 12],
    pub drive_number: u8,
    pub reserved_1: u8,
    pub boot_signature: u8,
    pub volume_id: [u8; 4],
    pub volume_label: [u8; 11],
    pub fs_type: [u8; 8],
}

impl Fat32SBLayout {
    pub fn is_valid(&self) -> bool {
        self.fs_type == *b"FAT32   "
    }
}