use alloc::{boxed::Box, sync::Arc};

use dentry::Dentry;
use fat32::fs::Fat32FS;
use flags::OpenFlags;
use fs::FileSystemManager;
use inode::{Inode, InodeType};
use lazy_static::lazy_static;

use crate::drivers::BLOCK_DEVICE;

pub mod dentry;
mod fat32;
pub mod file;
pub mod flags;
mod fs;
pub mod inode;
mod path;
pub mod pipe;
pub mod stdio;

lazy_static! {
    pub static ref FS_MANAGER: FileSystemManager = FileSystemManager::new();
}

lazy_static! {
    pub static ref ROOT_INODE: Arc<Inode> = {
        let fs = Fat32FS::load(BLOCK_DEVICE.clone());
        let root_inode = Fat32FS::root_inode(&fs);
        Arc::new(Inode::new(1, InodeType::Directory, Box::new(root_inode)))
    };
}

/// Open a file
pub fn open_file(inode: &Arc<Inode>, name: &str, flags: OpenFlags) -> Option<Arc<Dentry>> {
    // TODO: read_write
    // let (readable, writable) = flags.read_write();
    if flags.contains(OpenFlags::CREATE) {
        if let Some(dentry) = inode.lookup(name) {
            // clear size
            dentry.inode().clear();
            Some(dentry)
        } else {
            // create file
            let type_ = if flags.contains(OpenFlags::DIRECTORY) {
                InodeType::Directory
            } else {
                InodeType::Regular
            };
            let dentry = inode.create(name, type_)?;
            Some(dentry)
        }
    } else {
        if let Some(dentry) = inode.lookup(name) {
            if flags.contains(OpenFlags::TRUNC) {
                dentry.inode().clear();
            }
            Some(dentry)
        } else {
            None
        }
    }
}
