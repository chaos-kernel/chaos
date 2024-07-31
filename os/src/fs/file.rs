use alloc::{sync::Arc, vec::Vec};
use core::any::Any;

use super::{
    ext4::inode::Ext4Inode,
    fat32::inode::Fat32Inode,
    inode::{Inode, Stat},
};
use crate::mm::UserBuffer;

/// trait File for all file types
pub trait File: Any + Send + Sync {
    /// the file readable?
    fn readable(&self) -> bool;
    /// the file writable?
    fn writable(&self) -> bool;
    /// read from the file to buf, return the number of bytes read
    fn read(&self, buf: &mut [u8]) -> usize;
    /// read all data from the file
    fn read_all(&self) -> Vec<u8>;
    /// write to the file from buf, return the number of bytes writte
    fn write(&self, buf: &[u8]) -> usize;
    /// get file status
    fn fstat(&self) -> Option<Stat>;
    /// is directory
    fn is_dir(&self) -> bool {
        if let Some(stat) = self.fstat() {
            stat.is_dir()
        } else {
            true
        }
    }
}

// TODO: 优化这个函数
pub fn cast_file_to_inode(file: Arc<dyn File>) -> Option<Arc<dyn Inode>> {
    unsafe {
        let file_ptr = Arc::into_raw(file);
        let file_ref = &*(file_ptr as *const dyn Any);
        if file_ref.is::<Fat32Inode>() {
            let inode_ptr = file_ptr as *const Fat32Inode;
            let inode = Arc::from_raw(inode_ptr);
            Some(inode)
        } else {
            // 如果转换失败，我们需要重新创建原始的 Arc 以避免内存泄漏
            let _ = Arc::from_raw(file_ptr);
            None
        }
    }
}

pub fn cast_inode_to_file(inode: Arc<dyn Inode>) -> Option<Arc<dyn File>> {
    unsafe {
        let inode_ptr = Arc::into_raw(inode);
        let inode_ref = &*(inode_ptr as *const dyn Any);
        if inode_ref.is::<Fat32Inode>() {
            let file_ptr = inode_ptr as *const Fat32Inode;
            let file = Arc::from_raw(file_ptr);
            Some(file)
        } else if inode_ref.is::<Ext4Inode>() {
            let file_ptr = inode_ptr as *const Ext4Inode;
            let file = Arc::from_raw(file_ptr);
            Some(file)
        } else {
            // 如果转换失败，我们需要重新创建原始的 Arc 以避免内存泄漏
            let _ = Arc::from_raw(inode_ptr);
            None
        }
    }
}
