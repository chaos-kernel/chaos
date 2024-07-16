use alloc::{
    collections::BTreeMap,
    string::{String, ToString},
    sync::Arc,
};

use lazy_static::lazy_static;
use spin::Mutex;

use super::inode::InodeOps;
use crate::block::block_dev::BlockDevice;

/* FileSystem */

pub trait FileSystem: Sync + Send {
    fn load(bdev: Arc<dyn BlockDevice>) -> Arc<Self>
    where Self: Sized;
    fn fs_type(&self) -> FileSystemType;
    fn root_inode(self: Arc<Self>) -> Arc<dyn InodeOps>;
}

/* FileSystemType */

pub enum FileSystemType {
    FAT32,
    EXT4,
}

impl FileSystemType {
    #[allow(unused)]
    pub fn from_str(fs_type: &str) -> Self {
        match fs_type {
            "fat32" => FileSystemType::FAT32,
            "ext4" => FileSystemType::EXT4,
            _ => panic!("[FileSystemType] unsupported filesystem type"),
        }
    }

    #[allow(unused)]
    pub fn to_string(&self) -> String {
        match self {
            FileSystemType::FAT32 => "fat32".to_string(),
            FileSystemType::EXT4 => "ext4".to_string(),
        }
    }
}

/* FileSystemManager */

lazy_static! {
    pub static ref FS_MANAGER: Mutex<FileSystemManager> = Mutex::new(FileSystemManager::new());
}

pub struct FileSystemManager {
    /// mounted filesystem <solid mount path, fs>
    mounted_fs: BTreeMap<String, Arc<dyn FileSystem>>,
}

impl FileSystemManager {
    pub fn new() -> Self {
        Self {
            mounted_fs: BTreeMap::new(),
        }
    }

    /// mount a filesystem to a path (must be solid)
    pub fn mount(&mut self, fs: Arc<dyn FileSystem>, path: &str) {
        trace!(
            "[filesystem] mount {} to {}",
            fs.fs_type().to_string(),
            path
        );
        self.mounted_fs.insert(path.to_string(), fs);
    }

    /// unmount a filesystem
    pub fn unmount(&mut self, path: &str) {
        trace!("[filesystem] unmount {}", path);
        self.mounted_fs.remove(path);
    }

    pub fn get_fs(&self, path: &str) -> Option<Arc<dyn FileSystem>> {
        self.mounted_fs.get(path).cloned()
    }

    pub fn rootfs(&self) -> Option<Arc<dyn FileSystem>> {
        self.mounted_fs.get("/").cloned()
    }
}
