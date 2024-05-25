//! inode

use alloc::{string::String, sync::Arc, vec::Vec};
use lazy_static::*;
use crate::{drivers::BLOCK_DEVICE, fs::fat32::file_system::Fat32FS, mm::UserBuffer, sync::UPSafeCell};
use super::{efs::EasyFileSystem, file::File};

/// inode in memory
pub struct OSInode {
    readable: bool,
    writable: bool,
    inner: UPSafeCell<OSInodeInner>,
}

/// inner of inode in memory
pub struct OSInodeInner {
    pos: usize,
    inode: Arc<dyn Inode>,
}

impl OSInode {
    /// create a new inode in memory
    pub fn new(readable: bool, writable: bool, inode: Arc<dyn Inode>) -> Self {
        trace!("kernel: OSInode::new");
        Self {
            readable,
            writable,
            inner: unsafe { UPSafeCell::new(OSInodeInner { pos: 0, inode }) },
        }
    }
    /// read all data from the inode in memory
    pub fn read_all(&self) -> Vec<u8> {
        trace!("kernel: OSInode::read_all");
        let mut inner = self.inner.exclusive_access();
        let mut buffer = [0u8; 512];
        let mut v: Vec<u8> = Vec::new();
        loop {
            let len = inner.inode.read_at(inner.pos, &mut buffer);
            if len == 0 {
                break;
            }
            inner.pos += len;
            v.extend_from_slice(&buffer[..len]);
        }
        v
    }
}

impl Inode for OSInode {
    fn fstat(&self) -> (usize, u32) {
        let inner = self.inner.exclusive_access();
        inner.inode.fstat()
    }

    fn find(&self, name: &str) -> Option<Arc<dyn Inode>> {
        let inner = self.inner.exclusive_access();
        inner.inode.find(name)
    }

    fn create(&self, name: &str) -> Option<Arc<dyn Inode>> {
        let inner = self.inner.exclusive_access();
        inner.inode.create(name)
    }

    fn link(&self, old_name: &str, new_name: &str) -> Option<Arc<dyn Inode>> {
        let inner = self.inner.exclusive_access();
        inner.inode.link(old_name, new_name)
    }

    fn unlink(&self, name: &str) -> bool {
        let inner = self.inner.exclusive_access();
        inner.inode.unlink(name)
    }

    fn ls(&self) -> Vec<String> {
        let inner = self.inner.exclusive_access();
        inner.inode.ls()
    }

    fn read_at(&self, offset: usize, buf: &mut [u8]) -> usize {
        let inner = self.inner.exclusive_access();
        inner.inode.read_at(offset, buf)
    }

    fn write_at(&self, offset: usize, buf: &[u8]) -> usize {
        let inner = self.inner.exclusive_access();
        inner.inode.write_at(offset, buf)
    }

    fn clear(&self) {
        let inner = self.inner.exclusive_access();
        inner.inode.clear();
    }
}

/// Inode trait
pub trait Inode: Send + Sync {
    /// get status of file
    fn fstat(&self) -> (usize, u32);
    /// find the disk inode of the file with 'name'
    fn find(&self, name: &str) -> Option<Arc<dyn Inode>>;
    /// create a file with 'name' in the root directory
    fn create(&self, name: &str) -> Option<Arc<dyn Inode>>;
    /// create a link with a disk inode under current inode
    fn link(&self, old_name: &str, new_name: &str) -> Option<Arc<dyn Inode>>;
    /// Remove a link under current inode
    fn unlink(&self, name: &str) -> bool;
    /// list the file names in the root directory
    fn ls(&self) -> Vec<String>;
    /// Read the content in offset position of the file into 'buf'
    fn read_at(&self, offset: usize, buf: &mut [u8]) -> usize;
    /// Write the content in 'buf' into offset position of the file
    fn write_at(&self, offset: usize, buf: &[u8]) -> usize;
    /// Set the file(disk inode) length to zero, delloc all data blocks of the file.
    fn clear(&self);
}

/// The stat of a inode
#[repr(C)]
#[derive(Debug)]
pub struct Stat {
    /// ID of device containing file
    pub dev: u64,
    /// inode number
    pub ino: u64,
    /// file type and mode
    pub mode: StatMode,
    /// number of hard links
    pub nlink: u32,
    /// unused pad
    pad: [u64; 7],
}

impl Stat {
    /// create a new Stat, assuming is a file
    pub fn new(ino: u64, nlink: u32) -> Self {
        Self {
            dev: 0,
            ino,
            mode: StatMode::FILE,
            nlink,
            pad: [0; 7]
        }
    }
}

bitflags! {
    /// The mode of a inode
    /// whether a directory or a file
    pub struct StatMode: u32 {
        /// null
        const NULL  = 0;
        /// directory
        const DIR   = 0o040000;
        /// ordinary regular file
        const FILE  = 0o100000;
    }
}

lazy_static! {
    /// The root inode
    pub static ref ROOT_INODE: Arc<OSInode> = {
        let fs = Fat32FS::load(BLOCK_DEVICE.clone());
        let root_inode = Fat32FS::root_inode(&fs);
        let inode: Arc<dyn Inode> = Arc::new(root_inode);
        Arc::new(OSInode { 
            readable: true, 
            writable: true,
            inner: unsafe { UPSafeCell::new(OSInodeInner { pos: 0, inode }) }
        })
    };
}

impl File for OSInode {
    /// file readable?
    fn readable(&self) -> bool {
        self.readable
    }
    /// file writable?
    fn writable(&self) -> bool {
        self.writable
    }
    /// read file data into buffer
    fn read(&self, mut buf: UserBuffer) -> usize {
        trace!("kernel: OSInode::read");
        let mut inner = self.inner.exclusive_access();
        let mut total_read_size = 0usize;
        for slice in buf.buffers.iter_mut() {
            let read_size = inner.inode.read_at(inner.pos, *slice);
            if read_size == 0 {
                break;
            }
            inner.pos += read_size;
            total_read_size += read_size;
        }
        total_read_size
    }
    /// write buffer data into file
    fn write(&self, buf: UserBuffer) -> usize {
        trace!("kernel: OSInode::write");
        let mut inner = self.inner.exclusive_access();
        let mut total_write_size = 0usize;
        for slice in buf.buffers.iter() {
            let write_size = inner.inode.write_at(inner.pos, *slice);
            assert_eq!(write_size, slice.len());
            inner.pos += write_size;
            total_write_size += write_size;
        }
        total_write_size
    }
    fn fstat(&self) -> Option<(usize, u32)> {
        let inner = self.inner.exclusive_access();
        Some(inner.inode.fstat())
    }
}