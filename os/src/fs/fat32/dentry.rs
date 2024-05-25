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
        if layout.name[0] == 0x00 {
            return None;
        }
        let mut name = String::new();
        for i in 0..8 {
            if layout.name[i] == 0x20 {
                break;
            }
            name.push(layout.name[i] as char);
        }
        let mut ext = String::new();
        for i in 0..3 {
            if layout.ext[i] == 0x20 {
                break;
            }
            ext.push(layout.ext[i] as char);
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
    pub create_time: u8,
    pub create_date: u16,
    pub last_access_date: u16,
    pub start_cluster_high: u16,
    pub last_modify_time: u16,
    pub last_modify_date: u16,
    pub start_cluster_low: u16,
    pub file_size: u32,
}

