use alloc::{boxed::Box, collections::BTreeMap, string::String, sync::Arc, vec::Vec};

use lazy_static::lazy_static;
use spin::Mutex;

use super::{dentry::Dentry, file::File};
use crate::{block::BLOCK_SZ, mm::UserBuffer, timer::TimeSpec};

pub struct Inode {
    ino:   u64,
    type_: InodeType,
    size:  u64,
    links: u32,
    ops:   Box<dyn InodeOps>,
    stat:  InodeStat,
}

impl Inode {
    /// new
    pub fn new(ino: u64, type_: InodeType, ops: Box<dyn InodeOps>) -> Self {
        Self {
            ino,
            type_,
            size: 0,
            links: 1,
            ops,
            stat: InodeStat::Synced,
        }
    }
    /// loopup an inode in the directory
    pub fn lookup(self: &Arc<Self>, name: &str) -> Option<Arc<Dentry>> {
        self.ops.lookup(&self, name)
    }
    /// create an inode in the directory
    pub fn create(&self, name: &str, type_: InodeType) -> Option<Arc<Dentry>> {
        self.ops.create(self, name, type_)
    }
    /// unlink an inode in the directory
    pub fn unlink(&self, name: &str) -> bool {
        self.ops.unlink(self, name)
    }
    /// link an inode in the directory
    pub fn link(&self, name: &str, dentry: Arc<Dentry>) -> bool {
        self.ops.link(self, name, dentry)
    }
    /// rename an inode in the directory
    pub fn rename(&self, old_name: &str, new_name: &str) -> bool {
        self.ops.rename(self, old_name, new_name)
    }
    /// make a directory in the directory
    pub fn mkdir(&self, name: &str) -> Option<Arc<Dentry>> {
        self.ops.mkdir(self, name)
    }
    /// remove a directory in the directory
    pub fn rmdir(&self, name: &str) -> bool {
        self.ops.rmdir(self, name)
    }
    /// list all inodes in the directory
    pub fn ls(&self) -> Vec<String> {
        self.ops.ls(self)
    }
    /// clear the inode
    pub fn clear(&self) {
        self.ops.clear(self)
    }
    /// sync the inode to disk
    pub fn sync(&self) {
        self.ops.sync(self)
    }
    /// read all data
    pub fn read_all(&self) -> Vec<u8> {
        let mut buffer = [0u8; 512];
        let mut read_size = 0;
        let mut v: Vec<u8> = Vec::new();
        loop {
            let len = self.ops.read_at(self, read_size, &mut buffer);
            if len == 0 {
                break;
            }
            read_size += len;
            v.extend_from_slice(&buffer[..len]);
        }
        v
    }
}

impl Drop for Inode {
    fn drop(&mut self) {
        if self.stat == InodeStat::Dirty {
            self.sync();
        }
    }
}

/* Inode Operators */

pub trait InodeOps: Send + Sync {
    /// lookup an inode in the directory with the name (just name not path)
    fn lookup(&self, inode: &Inode, name: &str) -> Option<Arc<Dentry>>;
    /// create an inode in the directory with the name and type
    fn create(&self, inode: &Inode, name: &str, type_: InodeType) -> Option<Arc<Dentry>>;
    /// unlink an inode in the directory with the name (just name not path)
    fn unlink(&self, inode: &Inode, name: &str) -> bool;
    /// link an inode in the directory with the name (just name not path)
    fn link(&self, inode: &Inode, name: &str, target: Arc<Dentry>) -> bool;
    /// rename an inode in the directory with the old name and new name
    fn rename(&self, inode: &Inode, old_name: &str, new_name: &str) -> bool;
    /// make a directory in the directory with the name
    fn mkdir(&self, inode: &Inode, name: &str) -> Option<Arc<Dentry>>;
    /// remove a directory in the directory with the name
    fn rmdir(&self, inode: &Inode, name: &str) -> bool;
    /// list all inodes in the directory
    fn ls(&self, inode: &Inode) -> Vec<String>;
    /// clear the inode
    fn clear(&self, inode: &Inode);
    /// read at the offset of the inode
    fn read_at(&self, inode: &Inode, offset: usize, buf: &mut [u8]) -> usize;
    /// write at the offset of the inode
    fn write_at(&self, inode: &Inode, offset: usize, buf: &[u8]) -> usize;
    /// sync the inode to disk
    fn sync(&self, inode: &Inode);
}

/* Inode Types */

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InodeType {
    Regular,
    Directory,
    BlockDevice,
    CharDevice,
    Pipe,
}

/* Inode Status */

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InodeStat {
    Dirty,
    Synced,
}

/* Inode Manager */

pub struct InodeManager {
    inodes: BTreeMap<u64, Arc<Inode>>,
}

impl InodeManager {
    pub fn new() -> Self {
        Self {
            inodes: BTreeMap::new(),
        }
    }

    pub fn get(&self, ino: &u64) -> Option<Arc<Inode>> {
        let inode = self.inodes.get(ino);
        match inode {
            Some(inode) => Some(Arc::clone(inode)),
            None => None,
        }
    }

    pub fn insert(&mut self, inode: Inode) {
        self.inodes.insert(inode.ino, Arc::new(inode));
    }

    pub fn remove(&mut self) {
        todo!();
    }
}

lazy_static! {
    pub static ref INODE_MANAGER: Mutex<InodeManager> = todo!();
}

/* Inode Stat */

#[repr(C)]
#[derive(Debug)]
pub struct Stat {
    /// ID of device containing file
    st_dev:      u64,
    /// Inode number
    st_ino:      u64,
    /// File type and mode   
    st_mode:     u32,
    /// Number of hard links
    st_nlink:    u32,
    /// User ID of the file's owner.
    st_uid:      u32,
    /// Group ID of the file's group.
    st_gid:      u32,
    /// Device ID (if special file)
    st_rdev:     u64,
    __pad:       u64,
    /// Size of file, in bytes.
    pub st_size: i64,
    /// Optimal block size for I/O.
    st_blksize:  u32,
    __pad2:      i32,
    /// Number 512-byte blocks allocated.
    st_blocks:   u64,
    /// Backward compatibility. Used for time of last access.
    st_atime:    TimeSpec,
    /// Time of last modification.
    st_mtime:    TimeSpec,
    /// Time of last status change.
    st_ctime:    TimeSpec,
    __unused:    u64,
}

impl Stat {
    /// create a new stat
    pub fn new(
        st_dev: u64, st_ino: u64, st_mode: u32, st_nlink: u32, st_rdev: u64, st_size: i64,
        st_atime_sec: i64, st_mtime_sec: i64, st_ctime_sec: i64,
    ) -> Self {
        Self {
            st_dev,
            st_ino,
            st_mode,
            st_nlink,
            st_uid: 0,
            st_gid: 0,
            st_rdev,
            __pad: 0,
            st_size,
            st_blksize: BLOCK_SZ as u32,
            __pad2: 0,
            st_blocks: (st_size as u64 + BLOCK_SZ as u64 - 1) / BLOCK_SZ as u64,
            st_atime: TimeSpec {
                tv_sec:  st_atime_sec as usize,
                tv_nsec: 0,
            },
            st_mtime: TimeSpec {
                tv_sec:  st_mtime_sec as usize,
                tv_nsec: 0,
            },
            st_ctime: TimeSpec {
                tv_sec:  st_ctime_sec as usize,
                tv_nsec: 0,
            },
            __unused: 0,
        }
    }
    /// check whether the inode is a directory
    pub fn is_dir(&self) -> bool {
        StatMode::from_bits(self.st_mode)
            .unwrap()
            .contains(StatMode::DIR)
    }

    /// check whether the inode is a file
    pub fn is_file(&self) -> bool {
        StatMode::from_bits(self.st_mode)
            .unwrap()
            .contains(StatMode::FILE)
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

/* Impl File for Inode */

impl File for Inode {
    fn readable(&self) -> bool {
        // TODO:
        true
    }

    fn writable(&self) -> bool {
        // TODO:
        true
    }

    fn read(&self, mut buf: UserBuffer) -> usize {
        let mut total_read_size = 0;
        for slice in buf.buffers.iter_mut() {
            let read_size = self.ops.read_at(self, total_read_size, slice);
            if read_size == 0 {
                break;
            }
            total_read_size += read_size;
        }
        total_read_size
    }

    fn read_all(&self) -> Vec<u8> {
        let mut buffer = [0u8; 512];
        let mut v: Vec<u8> = Vec::new();
        let mut total_read_size = 0;
        loop {
            let len = self.ops.read_at(self, total_read_size, &mut buffer);
            if len == 0 {
                break;
            }
            total_read_size += len;
            v.extend_from_slice(&buffer[..len]);
        }
        v
    }

    fn write(&self, buf: UserBuffer) -> usize {
        let mut total_write_size = 0;
        for slice in buf.buffers.iter() {
            let write_size = self.ops.write_at(self, total_write_size, slice);
            if write_size == 0 {
                break;
            }
            total_write_size += write_size;
        }
        total_write_size
    }

    fn fstat(&self) -> Option<Stat> {
        Some(Stat::new(
            0,
            self.ino,
            0,
            self.links,
            0,
            self.size as i64,
            0,
            0,
            0,
        ))
    }
}
