use alloc::{collections::BTreeMap, sync::Arc};

use super::{inode::Inode, path::Path};

pub struct FileSystem {
    type_:      FileSystemType,
    root_inode: Arc<Inode>,
}

/* File System Type */

#[derive(Debug, Clone, Copy)]
pub enum FileSystemType {
    VFAT,
    EXT4,
}

impl FileSystemType {
    pub fn from_str(name: &str) -> Option<Self> {
        match name {
            "vfat" => Some(Self::VFAT),
            "ext4" => Some(Self::EXT4),
            _ => panic!("[FileSystemType] unknown file system type"),
        }
    }

    pub fn to_str(&self) -> &'static str {
        match self {
            Self::VFAT => "vfat",
            Self::EXT4 => "ext4",
        }
    }
}

/* File System Manager */

pub struct FileSystemManager {
    pub mounted_fs: BTreeMap<Path, Arc<FileSystem>>,
}

impl FileSystemManager {
    pub fn new() -> Self {
        Self {
            mounted_fs: BTreeMap::new(),
        }
    }

    pub fn mount(&mut self, fs: Arc<FileSystem>, path: &str) {
        let path = Path::new(path);
        self.mounted_fs.insert(path, fs);
    }

    pub fn unmount(&mut self, path: &str) {
        let path = Path::new(path);
        self.mounted_fs.remove(&path);
    }

    pub fn rootfs(&self) -> Arc<FileSystem> {
        self.mounted_fs.get(&Path::new("/")).unwrap().clone()
    }
}
