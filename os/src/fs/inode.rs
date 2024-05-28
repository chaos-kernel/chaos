//! inode

use alloc::{string::{String, ToString}, sync::Arc, vec::Vec};
use lazy_static::*;
use crate::{drivers::BLOCK_DEVICE, fs::fat32::file_system::Fat32FS, mm::UserBuffer, sync::UPSafeCell};
use super::file::{File};

/// inode in memory
pub struct OSInode {
    readable: bool,
    writable: bool,
    inner: UPSafeCell<OSInodeInner>,
}

/// inner of inode in memory
pub struct OSInodeInner {
    pos: usize,
    name: String,
    inode: Arc<dyn Inode>,
}

impl OSInode {
    /// create a new inode in memory
    pub fn new(readable: bool, writable: bool, name: String, inode: Arc<dyn Inode>) -> Self {
        trace!("kernel: OSInode::new");
        Self {
            readable,
            writable,
            inner: unsafe { UPSafeCell::new(OSInodeInner { pos: 0, name, inode }) },
        }
    }
    /// read all data from the inode in memory
    pub fn read_all(&self) -> Vec<u8> {
        trace!("kernel: OSInode::read_all");
        let mut inner = self.inner.exclusive_access();
        let mut buffer = [0u8; 512];
        let mut v: Vec<u8> = Vec::new();
        loop {
            let len = inner.inode.clone().read_at(inner.pos, &mut buffer);
            if len == 0 {
                break;
            }
            inner.pos += len;
            v.extend_from_slice(&buffer[..len]);
        }
        v
    }
    /// get the status of the inode in memory
    pub fn fstat(&self) -> Stat {
        let inner = self.inner.exclusive_access();
        inner.inode.clone().fstat()
    }
    /// find the inode in memory with 'name'
    pub fn find(&self, name: &str) -> Option<Arc<OSInode>> {
        let inner = self.inner.exclusive_access();
        if let Some(inode) = inner.inode.clone().find(name) {
            Some(Arc::new(OSInode::new(true, true, name.to_string(), inode)))
        } else {
            None
        }
    }
    /// create a inode in memory with 'name'
    pub fn create(&self, name: &str, stat: StatMode) -> Option<Arc<OSInode>> {
        let inner = self.inner.exclusive_access();
        if let Some(inode) = inner.inode.clone().create(name, stat) {
            Some(Arc::new(OSInode::new(true, true, name.to_string(), inode)))
        } else {
            None
        }
    }
    /// link a inode in memory with 'old_name' and 'new_name'
    pub fn link(&self, old_name: &str, new_name: &str) -> Option<Arc<OSInode>> {
        let inner = self.inner.exclusive_access();
        if let Some(inode) = inner.inode.clone().link(old_name, new_name) {
            Some(Arc::new(OSInode::new(true, true, new_name.to_string(), inode)))
        } else {
            None
        }
    }
    /// unlink a inode in memory with 'name'
    pub fn unlink(&self, name: &str) -> bool {
        let inner = self.inner.exclusive_access();
        inner.inode.clone().unlink(name)
    }
    /// list the file names in the inode in memory
    pub fn ls(&self) -> Vec<String> {
        let inner = self.inner.exclusive_access();
        inner.inode.clone().ls()
    }
    /// read the content in offset position of the inode in memory into 'buf'
    pub fn read_at(&self, offset: usize, buf: &mut [u8]) -> usize {
        let inner = self.inner.exclusive_access();
        inner.inode.clone().read_at(offset, buf)
    }
    /// write the content in 'buf' into offset position of the inode in memory
    pub fn write_at(&self, offset: usize, buf: &[u8]) -> usize {
        let inner = self.inner.exclusive_access();
        inner.inode.clone().write_at(offset, buf)
    }
    /// set the inode in memory length to zero, delloc all data blocks of the inode
    pub fn clear(&self) {
        let inner = self.inner.exclusive_access();
        inner.inode.clone().clear();
    }
    /// get the name
    pub fn name(&self) -> Option<String> {
        let inner = self.inner.exclusive_access();
        inner.name.clone().into()
    }
}

/// Inode trait
pub trait Inode: Send + Sync {
    /// get status of file
    fn fstat(self: Arc<Self>) -> Stat;
    /// find the disk inode of the file with 'name'
    fn find(self: Arc<Self>, name: &str) -> Option<Arc<dyn Inode>>;
    /// create a file with 'name' in the root directory
    fn create(self: Arc<Self>, name: &str, stat: StatMode) -> Option<Arc<dyn Inode>>;
    /// create a link with a disk inode under current inode
    fn link(self: Arc<Self>, old_name: &str, new_name: &str) -> Option<Arc<dyn Inode>>;
    /// Remove a link under current inode
    fn unlink(self: Arc<Self>, name: &str) -> bool;
    /// list the file names in the root directory
    fn ls(self: Arc<Self>) -> Vec<String>;
    /// Read the content in offset position of the file into 'buf'
    fn read_at(self: Arc<Self>, offset: usize, buf: &mut [u8]) -> usize;
    /// Write the content in 'buf' into offset position of the file
    fn write_at(self: Arc<Self>, offset: usize, buf: &[u8]) -> usize;
    /// Set the file(disk inode) length to zero, delloc all data blocks of the file.
    fn clear(self: Arc<Self>);
    /// Get the current directory name
    fn current_dirname(self: Arc<Self>) -> Option<String>;
}

/// The stat of a inode
#[repr(C)]
#[derive(Debug)]
pub struct Stat {
    /// ID of device containing file
    pub dev: u64,
    /// file type and mode
    pub mode: StatMode,
    /// number of hard links
    pub nlink: u32,
    /// unused pad
    pad: [u64; 7],
}

impl Stat {
    /// create a new stat
    pub fn new(mode: StatMode, nlink: u32) -> Self {
        Self {
            dev: 0,
            mode,
            nlink,
            pad: [0; 7],
        }
    }
    /// check whether the inode is a directory
    pub fn is_dir(&self) -> bool {
        self.mode.contains(StatMode::DIR)
    }

    /// check whether the inode is a file
    pub fn is_file(&self) -> bool {
        self.mode.contains(StatMode::FILE)
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
            inner: unsafe { UPSafeCell::new(OSInodeInner { pos: 0, name: "/".to_string(), inode }) }
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
            let read_size = inner.inode.clone().read_at(inner.pos, *slice);
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
            let write_size = inner.inode.clone().write_at(inner.pos, *slice);
            assert_eq!(write_size, slice.len());
            inner.pos += write_size;
            total_write_size += write_size;
        }
        total_write_size
    }
    fn fstat(&self) -> Option<Stat> {
        let inner = self.inner.exclusive_access();
        Some(inner.inode.clone().fstat())
    }
}