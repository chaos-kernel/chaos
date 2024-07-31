use alloc::sync::Arc;

use defs::OpenFlags;
use dentry::Dentry;
use ext4::fs::Ext4FS;
use fs::FileSystemManager;
use inode::{Inode, InodeType};
use lazy_static::lazy_static;
use spin::Mutex;

use crate::drivers::BLOCK_DEVICE;

pub mod defs;
pub mod dentry;
pub mod ext4;
mod fat32;
pub mod file;
mod fs;
pub mod inode;
mod path;
pub mod pipe;
pub mod stdio;

lazy_static! {
    pub static ref FS_MANAGER: Mutex<FileSystemManager> = Mutex::new(FileSystemManager::new());
}

lazy_static! {
    pub static ref ROOT_INODE: Arc<dyn Inode> = {
        let ext4fs = Arc::new(Ext4FS::new(BLOCK_DEVICE.clone()));
        FS_MANAGER.lock().mount(ext4fs, "/");
        FS_MANAGER.lock().rootfs().root_inode()
    };
}

pub fn init() {
    let _root = ROOT_INODE.clone();
}

/// Open a file
pub fn open_file(inode: Arc<dyn Inode>, name: &str, flags: OpenFlags) -> Option<Arc<Dentry>> {
    // TODO: read_write
    // let (readable, writable) = flags.read_write();
    if flags.contains(OpenFlags::O_CREAT) {
        if let Some(dentry) = inode.clone().lookup(name) {
            // clear size
            dentry.inode().clear();
            Some(dentry)
        } else {
            // create file
            let type_ = if flags.contains(OpenFlags::O_DIRECTORY) {
                InodeType::Directory
            } else {
                InodeType::Regular
            };
            let dentry = inode.create(name, type_)?;
            Some(dentry)
        }
    } else if let Some(dentry) = inode.lookup(name) {
        if flags.contains(OpenFlags::O_TRUNC) {
            dentry.inode().clear();
        }
        Some(dentry)
    } else {
        None
    }
}

pub struct Iovec {
    pub iov_base: usize,
    pub iov_len:  usize,
}
