//! File trait & inode(dir, file, pipe, stdin, stdout)

mod fat32;
pub(crate) mod file;
mod fs;
pub mod inode;
mod pipe;
mod stdio;

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
        const CREATE = 1 << 6;
        /// truncate file size to 0
        const TRUNC = 1 << 10;
        /// directory
        const DIRECTORY = 1 << 21;
    }
}

/// Open a file
pub fn open_file(inode: &Inode, name: &str, flags: OpenFlags) -> Option<Arc<Inode>> {
    // TODO: read_write
    // let (readable, writable) = flags.read_write();
    if flags.contains(OpenFlags::CREATE) {
        if let Some(inode) = ROOT_INODE.find(name) {
            // clear size
            inode.clear();
            Some(inode)
        } else {
            // create file
            let stat = if flags.contains(OpenFlags::DIRECTORY) {
                StatMode::DIR
            } else {
                StatMode::FILE
            };
            inode.create(name, stat)
        }
    } else {
        inode.find(name).map(|inode| {
            if flags.contains(OpenFlags::TRUNC) {
                inode.clear();
            }
            inode
        })
    }
}

/// Link a file
pub fn link(old_name: &str, new_name: &str) -> Option<Arc<Inode>> {
    ROOT_INODE.link(old_name, new_name)
}

/// Unlink a file
pub fn unlink(name: &str) -> bool {
    ROOT_INODE.unlink(name)
}

/// List all apps in the root directory
pub fn list_apps() {
    println!("/**** APPS ****");
    for app in ROOT_INODE.ls() {
        println!("{}", app);
    }
    println!("**************/");
}

use alloc::sync::Arc;

use inode::{Inode, InodeOps, Stat, StatMode, ROOT_INODE};
pub use pipe::{make_pipe, Pipe};
pub use stdio::{Stdin, Stdout};
