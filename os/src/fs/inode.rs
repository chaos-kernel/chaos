//! inode

use alloc::{string::String, sync::Arc, vec::Vec};
use lazy_static::*;
use spin::Mutex;
use crate::{drivers::BLOCK_DEVICE, fs::efs::inode::EfsInode, mm::UserBuffer, sync::UPSafeCell};
use super::{efs::{BlockDevice, EasyFileSystem}, File};

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

/// Inode metadata
#[derive(Clone)]
pub struct InodeMeta {
    /// block id
    pub block_id: usize,
    /// block offset
    pub block_offset: usize,
    /// file system
    pub fs: Arc<Mutex<EasyFileSystem>>,
    /// block device
    pub block_device: Arc<dyn BlockDevice>,
}

impl InodeMeta {
    /// create a new InodeMeta
    pub fn new(
        block_id: usize,
        block_offset: usize,
        fs: Arc<Mutex<EasyFileSystem>>,
        block_device: Arc<dyn BlockDevice>,
    ) -> Self {
        Self {
            block_id,
            block_offset,
            fs,
            block_device,
        }
    }
}

/// Inode trait
pub trait Inode: Send + Sync {
    /// get metadata
    fn meta(&self) -> InodeMeta;
    /// set metadata
    fn set_meta(&mut self, meta: InodeMeta);
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

lazy_static! {
    /// The root inode
    pub static ref ROOT_INODE: Arc<EfsInode> = {
        let efs = EasyFileSystem::open(BLOCK_DEVICE.clone());
        Arc::new(EasyFileSystem::root_inode(&efs))
    };
}

/// List all apps in the root directory
pub fn list_apps() {
    println!("/**** APPS ****");
    for app in ROOT_INODE.ls() {
        println!("{}", app);
    }
    println!("**************/");
}

bitflags! {
    ///  The flags argument to the open() system call is constructed by ORing together zero or more of the following values:
    pub struct OpenFlags: u32 {
        /// readyonly
        const RDONLY = 0;
        /// writeonly
        const WRONLY = 1 << 0;
        /// read and write
        const RDWR = 1 << 1;
        /// create new file
        const CREATE = 1 << 9;
        /// truncate file size to 0
        const TRUNC = 1 << 10;
    }
}

impl OpenFlags {
    /// Do not check validity for simplicity
    /// Return (readable, writable)
    pub fn read_write(&self) -> (bool, bool) {
        if self.is_empty() {
            (true, false)
        } else if self.contains(Self::WRONLY) {
            (false, true)
        } else {
            (true, true)
        }
    }
}

/// Open a file
pub fn open_file(name: &str, flags: OpenFlags) -> Option<Arc<OSInode>> {
    trace!("kernel: open_file: name = {}, flags = {:?}", name, flags);
    let (readable, writable) = flags.read_write();
    if flags.contains(OpenFlags::CREATE) {
        if let Some(inode) = ROOT_INODE.find(name) {
            // clear size
            inode.clear();
            Some(Arc::new(OSInode::new(readable, writable, inode)))
        } else {
            // create file
            ROOT_INODE
                .create(name)
                .map(|inode| Arc::new(OSInode::new(readable, writable, inode)))
        }
    } else {
        ROOT_INODE.find(name).map(|inode| {
            if flags.contains(OpenFlags::TRUNC) {
                inode.clear();
            }
            Arc::new(OSInode::new(readable, writable, inode))
        })
    }
}

/// Link a file
pub fn link(old_name: &str, new_name: &str) -> Option<Arc<dyn Inode>> {
    ROOT_INODE.link(old_name, new_name)
}

/// Unlink a file
pub fn unlink(name: &str) -> bool {
    ROOT_INODE.unlink(name)
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