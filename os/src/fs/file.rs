use alloc::sync::Arc;

use crate::mm::UserBuffer;

use super::inode::{Inode, OSInode, Stat};



/// trait File for all file types
pub trait File: {
    /// the file readable?
    fn readable(&self) -> bool;
    /// the file writable?
    fn writable(&self) -> bool;
    /// read from the file to buf, return the number of bytes read
    fn read(&self, buf: UserBuffer) -> usize;
    /// write to the file from buf, return the number of bytes written
    fn write(&self, buf: UserBuffer) -> usize;
    /// get file status
    fn fstat(&self) -> Option<Stat>;
    /// is directory
    fn is_dir(&self) -> bool {
        let stat = self.fstat().unwrap();
        stat.is_dir()
    }
}
