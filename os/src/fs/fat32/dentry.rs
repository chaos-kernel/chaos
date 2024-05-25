use alloc::string::String;

pub struct Fat32Dentry {
    name: String,
    ext: String,
    attr: FileAttributes,
    file_size: u32,
    start_cluster: u32,
}

bitflags! {
    struct FileAttributes: u8 {
        const READ_ONLY  = 0b00000001;
        const HIDDEN     = 0b00000010;
        const SYSTEM     = 0b00000100;
        const VOLUME_ID  = 0b00001000;
        const DIRECTORY  = 0b00010000;
        const ARCHIVE    = 0b00100000;
    }
}

impl Fat32Dentry {
    pub fn from_layout(layout: &Fat32DentryLayout) -> Option<Self> {
        // not a valid entry
        if layout.name[0] == 0x00 || layout.name[0] == 0xE5 {
            return None;
        }
        let name_capital = layout.reserved & 0x08 != 0;
        let ext_capital = layout.reserved & 0x10 != 0;
        let mut name = String::new();
        for i in 0..8 {
            if layout.name[i] == 0x20 {
                break;
            }
            let c = if name_capital {
                layout.name[i].to_ascii_uppercase()
            } else {
                layout.name[i].to_ascii_lowercase()
            };
            name.push(c as char);
        }
        let mut ext = String::new();
        for i in 0..3 {
            if layout.ext[i] == 0x20 {
                break;
            }
            let c = if ext_capital {
                layout.ext[i].to_ascii_uppercase()
            } else {
                layout.ext[i].to_ascii_lowercase()
            };
            ext.push(c as char);
        }
        Some(Self {
            name,
            ext,
            attr: FileAttributes::from_bits_truncate(layout.attr),
            file_size: layout.file_size,
            start_cluster: (layout.start_cluster_high as u32) << 16 | layout.start_cluster_low as u32,
        })
    }

    pub fn name(&self) -> String {
        self.name.clone()
    }

    pub fn ext(&self) -> String {
        self.ext.clone()
    }

    pub fn fullname(&self) -> String {
        let mut fullname = self.name.clone();
        if !self.ext.is_empty() {
            fullname.push('.');
            fullname.push_str(&self.ext);
        }
        fullname
    }

    pub fn is_system(&self) -> bool {
        self.attr.contains(FileAttributes::SYSTEM)
    }

    pub fn is_dir(&self) -> bool {
        self.attr.contains(FileAttributes::DIRECTORY)
    }

    pub fn is_volume_id(&self) -> bool {
        self.attr.contains(FileAttributes::VOLUME_ID)
    }

    pub fn is_file(&self) -> bool {
        !self.is_dir() && !self.is_volume_id() && !self.is_system()
    }
}

#[repr(C)]
pub struct Fat32DentryLayout {
    pub name: [u8; 8],
    pub ext: [u8; 3],
    pub attr: u8,
    pub reserved: u8,
    pub create_time_ms: u8,
    pub create_time: u16,
    pub create_date: u16,
    pub last_access_date: u16,
    pub start_cluster_high: u16,
    pub last_modify_time: u16,
    pub last_modify_date: u16,
    pub start_cluster_low: u16,
    pub file_size: u32,
}


#[repr(C, packed)]
/// the layout of a fat32 long dentry
pub struct Fat32LDentryLayout {
    pub order: u8,
    pub name1: [u16; 5],
    pub attr: u8,
    pub reserved: u8,
    pub checksum: u8,
    pub name2: [u16; 6],
    pub start_cluster: u16,
    pub name3: [u16; 2],
}

impl Fat32LDentryLayout {
    pub fn from_short_layout(layout: &Fat32DentryLayout) -> Option<Self> {
        if layout.attr & 0x0F != 0x0F {
            return None;
        }
        Some(Self {
            order: layout.name[0],
            name1: [
                layout.name[1] as u16 | ((layout.name[2] as u16) << 8),
                layout.name[3] as u16 | ((layout.name[4] as u16) << 8),
                layout.name[5] as u16 | ((layout.name[6] as u16) << 8),
                layout.name[7] as u16 | ((layout.ext[0] as u16) << 8),
                layout.ext[1] as u16 | ((layout.ext[2] as u16) << 8),
            ],
            attr: layout.attr,
            reserved: layout.reserved,
            checksum: layout.create_time_ms,
            name2: [
                layout.create_time,
                layout.create_date,
                layout.last_access_date,
                layout.start_cluster_high,
                layout.last_modify_time,
                layout.last_modify_date,
            ],
            start_cluster: layout.start_cluster_low,
            name3: [
                layout.file_size as u16,
                (layout.file_size >> 16) as u16,
            ],
        })
    }

    pub fn is_last(&self) -> bool {
        self.order & 0x40 != 0
    }

    pub fn is_deleted(&self) -> bool {
        self.order == 0xE5
    }

    pub fn is_empty(&self) -> bool {
        self.order == 0x00
    }

    pub fn is_valid(&self) -> bool {
        !self.is_last() && !self.is_deleted() && !self.is_empty()
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