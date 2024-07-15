use alloc::{collections::BTreeMap, string::String, sync::Arc, vec::Vec};

use lazy_static::lazy_static;
use spin::Mutex;

use super::inode::InodeOps;
use crate::block::block_dev::BlockDevice;

pub trait FileSystem: Sync + Send {
    fn load(bdev: Arc<dyn BlockDevice>) -> Arc<Self>
    where
        Self: Sized;
    fn fs_type() -> FileSystemType
    where
        Self: Sized;
    fn root_inode(self: Arc<Self>) -> Arc<dyn InodeOps>;
}

pub enum FileSystemType {
    FAT32,
    EXT4,
}

lazy_static! {
    pub static ref FS_MANAGER: Mutex<FileSystemManager> = Mutex::new(FileSystemManager::new());
}

pub struct FileSystemManager {
    root_fs: Option<Arc<dyn FileSystem>>,
    mounted_fs: BTreeMap<String, Arc<dyn FileSystem>>,
}

impl FileSystemManager {
    pub fn new() -> Self {
        Self {
            root_fs: None,
            mounted_fs: BTreeMap::new(),
        }
    }

    pub fn init(&mut self, fs: Arc<dyn FileSystem>) {
        self.root_fs = Some(fs);
    }

    pub fn mount(&mut self, fs: Arc<dyn FileSystem>, path: String) {
        self.mounted_fs.insert(path, fs);
    }

    pub fn get_fs(&self, path: &str) -> Option<Arc<dyn FileSystem>> {
        self.mounted_fs.get(path).cloned()
    }

    pub fn rootfs(&self) -> Option<Arc<dyn FileSystem>> {
        self.root_fs.clone()
    }
}
