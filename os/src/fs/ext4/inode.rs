use alloc::{string::String, sync::Arc, vec::Vec};

use ext4_rs::{ext4_path_check, inode, Ext4DirSearchResult, Ext4File, Ext4InodeRef};

use super::fs::Ext4FS;
use crate::fs::{
    dentry::Dentry,
    fs::FileSystemType,
    inode::{Inode, InodeType},
};

pub struct Ext4Inode {
    pub fs:  Arc<Ext4FS>,
    pub ino: u32,
}

impl Inode for Ext4Inode {
    fn fstype(&self) -> FileSystemType {
        FileSystemType::EXT4
    }
    fn clear(&self) {
        todo!()
    }
    fn create(self: Arc<Self>, _name: &str, _type_: InodeType) -> Option<Arc<Dentry>> {
        todo!()
    }

    fn lookup(self: Arc<Self>, name: &str) -> Option<Arc<Dentry>> {
        let mut file = Ext4File::new();
        self.fs
            .ext4
            .ext4_open_from(self.ino, &mut file, name, "r", false)
            .ok()?;
        let inode = Ext4Inode {
            fs:  self.fs.clone(),
            ino: file.inode,
        };
        let dentry = Dentry::new(name, Arc::new(inode));
        Some(Arc::new(dentry))
    }

    fn unlink(self: Arc<Self>, name: &str) -> bool {
        todo!()
    }

    fn link(self: Arc<Self>, _name: &str, _target: Arc<Dentry>) -> bool {
        todo!()
    }

    fn rename(self: Arc<Self>, _old_name: &str, _new_name: &str) -> bool {
        todo!()
    }

    fn mkdir(self: Arc<Self>, name: &str) -> Option<Arc<Dentry>> {
        todo!()
    }

    fn rmdir(self: Arc<Self>, name: &str) -> bool {
        todo!()
    }

    fn ls(&self) -> Vec<String> {
        todo!()
    }

    fn read_at(&self, offset: usize, buf: &mut [u8]) -> usize {
        let mut file = Ext4File::new();
        file.inode = self.ino;
        file.fpos = offset;
        file.fsize = offset as u64;
        let mut read_size = 0;
        self.fs
            .ext4
            .ext4_file_read(&mut file, buf, buf.len(), &mut read_size);
        read_size
    }

    fn write_at(&self, offset: usize, buf: &[u8]) -> usize {
        todo!()
    }
}
