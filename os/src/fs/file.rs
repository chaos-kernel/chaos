use alloc::vec::Vec;

use super::inode::Stat;
use crate::mm::UserBuffer;

/// trait File for all file types
pub trait File {
    /// the file readable?
    fn readable(&self) -> bool;
    /// the file writable?
    fn writable(&self) -> bool;
    /// read from the file to buf, return the number of bytes read
    fn read(&self, buf: UserBuffer) -> usize;
    /// read all data from the file
    fn read_all(&self) -> Vec<u8>;
    /// write to the file from buf, return the number of bytes writte
    fn write(&self, buf: UserBuffer) -> usize;
    /// get file status
    fn fstat(&self) -> Option<Stat>;
    /// is directory
    fn is_dir(&self) -> bool {
        let stat = self.fstat().unwrap();
        stat.is_dir()
    }
}
