use alloc::{string::String, sync::Arc};

use super::fat::FAT;
use crate::block::{block_cache::get_block_cache, block_dev::BlockDevice};

pub struct Fat32Dentry {
    pub sector_id:     usize,
    pub sector_offset: usize,
    pub deleted:       bool,
    pub bdev:          Arc<dyn BlockDevice>,
    pub fat:           Arc<FAT>,
}

bitflags! {
    pub struct FileAttributes: u8 {
        const READ_ONLY  = 0b00000001;
        const HIDDEN     = 0b00000010;
        const SYSTEM     = 0b00000100;
        const VOLUME_ID  = 0b00001000;
        const DIRECTORY  = 0b00010000;
        const ARCHIVE    = 0b00100000;
    }
}

impl Fat32Dentry {
    pub fn new(
        sector_id: usize, sector_offset: usize, bdev: &Arc<dyn BlockDevice>, fat: &Arc<FAT>,
    ) -> Self {
        Self {
            sector_id,
            sector_offset,
            deleted: false,
            bdev: Arc::clone(bdev),
            fat: fat.clone(),
        }
    }

    pub fn new_deleted(bdev: &Arc<dyn BlockDevice>, fat: &Arc<FAT>) -> Self {
        Self {
            sector_id:     0,
            sector_offset: 0,
            deleted:       true,
            bdev:          Arc::clone(bdev),
            fat:           fat.clone(),
        }
    }

    pub fn is_system(&self) -> bool {
        self.read_dentry().attr() == FileAttributes::SYSTEM
    }

    pub fn is_dir(&self) -> bool {
        self.read_dentry().attr() == FileAttributes::DIRECTORY
    }

    pub fn is_volume_id(&self) -> bool {
        self.read_dentry().attr() == FileAttributes::VOLUME_ID
    }

    pub fn is_file(&self) -> bool {
        !self.is_dir() && !self.is_volume_id() && !self.is_system()
    }

    pub fn is_deleted(&self) -> bool {
        self.deleted
    }

    pub fn file_size(&self) -> usize {
        let (sector_id, offset) = self.to_end();
        get_block_cache(sector_id, self.bdev.clone())
            .lock()
            .read(offset, |layout: &Fat32DentryLayout| {
                layout.file_size() as usize
            })
    }

    pub fn set_file_size(&self, size: usize) {
        let (sector_id, offset) = self.to_end();
        get_block_cache(sector_id, self.bdev.clone()).lock().modify(
            offset,
            |layout: &mut Fat32DentryLayout| {
                layout.file_size = size as u32;
            },
        );
    }

    pub fn is_long(&self) -> bool {
        self.read_dentry().is_long()
    }

    pub fn name(&self) -> String {
        if self.is_long() {
            let mut name = String::new();
            let mut sector_id = self.sector_id;
            let mut offset = self.sector_offset;
            loop {
                let layout = get_block_cache(sector_id, self.bdev.clone())
                    .lock()
                    .read(offset, |layout: &Fat32LDentryLayout| *layout);
                name.insert_str(0, &layout.name());
                if layout.is_end() {
                    break;
                }
                (sector_id, offset) = self.fat.next_dentry_id(sector_id, offset).unwrap();
            }
            name
        } else {
            get_block_cache(self.sector_id, self.bdev.clone())
                .lock()
                .read(self.sector_offset, |layout: &Fat32DentryLayout| {
                    layout.name()
                })
        }
    }

    pub fn start_cluster_id(&self) -> usize {
        let (sector_id, offset) = self.to_end();
        get_block_cache(sector_id, self.bdev.clone())
            .lock()
            .read(offset, |layout: &Fat32DentryLayout| {
                layout.start_cluster_id() as usize
            })
    }

    fn to_end(&self) -> (usize, usize) {
        if !self.is_long() {
            return (self.sector_id, self.sector_offset);
        }
        let mut sector_id = self.sector_id;
        let mut offset = self.sector_offset;
        loop {
            let layout = get_block_cache(sector_id, self.bdev.clone())
                .lock()
                .read(offset, |layout: &Fat32LDentryLayout| *layout);
            (sector_id, offset) = self.fat.next_dentry_id(sector_id, offset).unwrap();
            if layout.is_end() {
                break;
            }
        }
        (sector_id, offset)
    }

    fn read_dentry(&self) -> Fat32DentryLayout {
        get_block_cache(self.sector_id, self.bdev.clone())
            .lock()
            .read(self.sector_offset, |layout: &Fat32DentryLayout| *layout)
    }

    fn write_dentry(&self, layout: &Fat32DentryLayout) {
        get_block_cache(self.sector_id, self.bdev.clone())
            .lock()
            .modify(self.sector_offset, |l: &mut Fat32DentryLayout| {
                *l = *layout;
            });
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Fat32DentryLayout {
    name:               [u8; 8],
    ext:                [u8; 3],
    attr:               u8,
    reserved:           u8,
    create_time_ms:     u8,
    create_time:        u16,
    create_date:        u16,
    last_access_date:   u16,
    start_cluster_high: u16,
    last_modify_time:   u16,
    last_modify_date:   u16,
    start_cluster_low:  u16,
    file_size:          u32,
}

impl Fat32DentryLayout {
    pub fn new(
        file_name: &str, attr: FileAttributes, start_cluster: usize, file_size: u32,
    ) -> Self {
        let mut name = [0u8; 8];
        let mut ext = [0u8; 3];
        let mut name_capital = false;
        let mut ext_capital = false;
        let mut i = 0;
        for c in file_name.chars() {
            if c == '.' {
                i = 8;
                continue;
            }
            if i < 8 {
                name[i] = c as u8;
                if c.is_ascii_uppercase() {
                    name_capital = true;
                }
            } else if i < 11 {
                ext[i - 8] = c as u8;
                if c.is_ascii_uppercase() {
                    ext_capital = true;
                }
            } else {
                break;
            }
            i += 1;
        }
        Self {
            name,
            ext,
            attr: attr.bits(),
            reserved: if name_capital { 0x08 } else { 0x00 }
                | if ext_capital { 0x10 } else { 0x00 },
            create_time_ms: 0,
            create_time: 0,
            create_date: 0,
            last_access_date: 0,
            start_cluster_high: (start_cluster >> 16) as u16,
            last_modify_time: 0,
            last_modify_date: 0,
            start_cluster_low: start_cluster as u16,
            file_size,
        }
    }

    pub fn is_long(&self) -> bool {
        self.attr & 0x0F == 0x0F
    }

    pub fn is_deleted(&self) -> bool {
        self.name[0] == 0xE5
    }

    pub fn is_empty(&self) -> bool {
        self.name[0] == 0x00
    }

    pub fn attr(&self) -> FileAttributes {
        FileAttributes::from_bits_truncate(self.attr)
    }

    pub fn file_size(&self) -> u32 {
        self.file_size
    }

    pub fn start_cluster_id(&self) -> u32 {
        (self.start_cluster_high as u32) << 16 | self.start_cluster_low as u32
    }

    pub fn set_deleted(&mut self) {
        self.name[0] = 0xE5;
    }

    pub fn name(&self) -> String {
        let mut name = String::new();
        for i in self.name.iter() {
            if *i == 0x20 {
                break;
            }
            name.push(char::from_u32(*i as u32).unwrap());
        }
        name.push('.');
        for i in self.ext.iter() {
            if *i == 0x20 {
                break;
            }
            name.push(char::from_u32(*i as u32).unwrap());
        }
        name
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
/// the layout of a fat32 long dentry
pub struct Fat32LDentryLayout {
    pub order:         u8,
    pub name1:         [u16; 5],
    pub attr:          u8,
    pub reserved:      u8,
    pub checksum:      u8,
    pub name2:         [u16; 6],
    pub start_cluster: u16,
    pub name3:         [u16; 2],
}

impl Fat32LDentryLayout {
    pub fn new(mut order: u8, name: &str, is_end: bool) -> Self {
        let mut name1 = [0u16; 5];
        let mut name2 = [0u16; 6];
        let mut name3 = [0u16; 2];
        let mut i = 0;
        for c in name.chars() {
            if i < 5 {
                name1[i] = c as u16;
            } else if i < 11 {
                name2[i - 5] = c as u16;
            } else {
                name3[i - 11] = c as u16;
            }
            i += 1;
        }
        if is_end {
            order |= 0x40;
        }
        Self {
            order,
            name1,
            attr: 0x0F,
            reserved: 0,
            checksum: 0,
            name2,
            start_cluster: 0,
            name3,
        }
    }
    pub fn from_short_layout(layout: &Fat32DentryLayout) -> Option<Self> {
        if layout.attr & 0x0F != 0x0F {
            return None;
        }
        Some(Self {
            order:         layout.name[0],
            name1:         [
                layout.name[1] as u16 | ((layout.name[2] as u16) << 8),
                layout.name[3] as u16 | ((layout.name[4] as u16) << 8),
                layout.name[5] as u16 | ((layout.name[6] as u16) << 8),
                layout.name[7] as u16 | ((layout.ext[0] as u16) << 8),
                layout.ext[1] as u16 | ((layout.ext[2] as u16) << 8),
            ],
            attr:          layout.attr,
            reserved:      layout.reserved,
            checksum:      layout.create_time_ms,
            name2:         [
                layout.create_time,
                layout.create_date,
                layout.last_access_date,
                layout.start_cluster_high,
                layout.last_modify_time,
                layout.last_modify_date,
            ],
            start_cluster: layout.start_cluster_low,
            name3:         [layout.file_size as u16, (layout.file_size >> 16) as u16],
        })
    }

    pub fn is_end(&self) -> bool {
        self.order & 0x40 != 0
    }

    pub fn is_deleted(&self) -> bool {
        self.order == 0xE5
    }

    pub fn is_empty(&self) -> bool {
        self.order == 0x00
    }

    pub fn is_valid(&self) -> bool {
        !self.is_end() && !self.is_deleted() && !self.is_empty()
    }

    pub fn name(&self) -> String {
        let mut name = String::new();
        let mut finished = false;
        for i in self.name1 {
            if i == 0x0000 {
                finished = true;
                break;
            }
            name.push(char::from_u32(i as u32).unwrap());
        }
        if finished {
            return name;
        }
        for i in self.name2 {
            if i == 0x0000 {
                finished = true;
                break;
            }
            name.push(char::from_u32(i as u32).unwrap());
        }
        if finished {
            return name;
        }
        for i in self.name3 {
            if i == 0x0000 {
                break;
            }
            name.push(char::from_u32(i as u32).unwrap());
        }
        name
    }
}
