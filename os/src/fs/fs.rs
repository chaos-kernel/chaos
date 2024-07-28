use alloc::{collections::BTreeMap, sync::Arc};

use spin::Mutex;

use super::{file::File, inode::Inode, path::Path};
use crate::sync::UPSafeCell;

pub trait FileSystem: Send + Sync {
    fn fs_type(&self) -> FileSystemType;
    fn root_inode(self: Arc<Self>) -> Arc<dyn Inode>;
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
    pub mounted_fs: BTreeMap<Path, Arc<dyn FileSystem>>,
}

impl FileSystemManager {
    pub fn new() -> Self {
        Self {
            mounted_fs: BTreeMap::new(),
        }
    }

    pub fn mount(&mut self, fs: Arc<dyn FileSystem>, path: &str) {
        let path = Path::new(path);
        self.mounted_fs.insert(path, fs);
    }

    pub fn unmount(&mut self, path: &str) {
        let path = Path::new(path);
        self.mounted_fs.remove(&path);
    }

    pub fn rootfs(&self) -> Arc<dyn FileSystem> {
        self.mounted_fs.get(&Path::new("/")).unwrap().clone()
    }
}
