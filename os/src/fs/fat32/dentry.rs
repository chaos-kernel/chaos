use alloc::string::String;

pub struct Fat32Dentry {
    name: String,
    attr: FileAttributes,
    file_size: u32,
    start_cluster: u32,
    deleted: bool,
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
    pub fn new(name: String, attr: FileAttributes, file_size: u32, start_cluster: u32) -> Self {
        Self {
            name,
            attr,
            file_size,
            start_cluster,
            deleted: false,
        }
    }

    pub fn from_layout(layout: &Fat32DentryLayout) -> Option<Self> {
        // not a valid entry
        if layout.is_empty() {
            return None;
        }
        if layout.is_deleted() {
            return Some(Self {
                name: String::new(),
                attr: FileAttributes::empty(),
                file_size: 0,
                start_cluster: 0,
                deleted: true,
            });
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
        if !ext.is_empty() {
            name.push('.');
            name.push_str(&ext);
        }
        Some(Self {
            name,
            attr: FileAttributes::from_bits_truncate(layout.attr),
            file_size: layout.file_size,
            start_cluster: (layout.start_cluster_high as u32) << 16 | layout.start_cluster_low as u32,
            deleted: layout.is_deleted(),
        })
    }

    pub fn name(&self) -> String {
        self.name.clone()
    }

    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }

    pub fn start_cluster(&self) -> u32 {
        self.start_cluster
    }
    
    pub fn file_size(&self) -> u32 {
        self.file_size
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

    pub fn is_deleted(&self) -> bool {
        self.deleted
    }
}

#[derive(Debug)]
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

impl Fat32DentryLayout {
    pub fn from_dentry(dentry: &Fat32Dentry) -> Self {
        let mut name = [0u8; 8];
        let mut ext = [0u8; 3];
        let mut name_capital = false;
        let mut ext_capital = false;
        let mut i = 0;
        for c in dentry.name.chars() {
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
            attr: dentry.attr.bits(),
            reserved: if name_capital { 0x08 } else { 0x00 } | if ext_capital { 0x10 } else { 0x00 },
            create_time_ms: 0,
            create_time: 0,
            create_date: 0,
            last_access_date: 0,
            start_cluster_high: (dentry.start_cluster >> 16) as u16,
            last_modify_time: 0,
            last_modify_date: 0,
            start_cluster_low: dentry.start_cluster as u16,
            file_size: dentry.file_size,
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