use alloc::{string::String, sync::Arc, vec::Vec};

use ext4_rs::InodeFileType;

use super::fs::Ext4FS;
use crate::fs::{
    dentry::Dentry,
    fs::FileSystemType,
    inode::{Inode, InodeType},
};

pub struct Ext4Inode {
    pub fs:  Arc<Ext4FS>,
    pub ino: u64,
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
        let attr = self
            .fs
            .ext4
            .exclusive_access()
            .fuse_lookup(self.ino, name)
            .ok()?;
        let inode = Ext4Inode {
            fs:  Arc::clone(&self.fs),
            ino: attr.ino,
        };
        let dentry = Dentry::new(name, Arc::new(inode));
        Some(Arc::new(dentry))
    }

    fn unlink(self: Arc<Self>, name: &str) -> bool {
        self.fs
            .ext4
            .exclusive_access()
            .fuse_unlink(self.ino, name)
            .is_ok()
    }

    fn link(self: Arc<Self>, _name: &str, _target: Arc<Dentry>) -> bool {
        todo!()
    }

    fn rename(self: Arc<Self>, _old_name: &str, _new_name: &str) -> bool {
        todo!()
    }

    fn mkdir(self: Arc<Self>, name: &str) -> Option<Arc<Dentry>> {
        self.fs
            .ext4
            .exclusive_access()
            .fuse_mkdir(
                self.ino,
                name,
                InodeFileType::bits(&InodeFileType::S_IFDIR) as u32,
                0,
            )
            .ok()?;
        let dir = self
            .fs
            .ext4
            .exclusive_access()
            .fuse_lookup(self.ino, name)
            .ok()?;
        let inode = Ext4Inode {
            fs:  Arc::clone(&self.fs),
            ino: dir.ino,
        };
        let dentry = Dentry::new(name, Arc::new(inode));
        Some(Arc::new(dentry))
    }

    fn rmdir(self: Arc<Self>, name: &str) -> bool {
        self.fs
            .ext4
            .exclusive_access()
            .fuse_rmdir(self.ino, name)
            .is_ok()
    }

    fn ls(&self) -> Vec<String> {
        todo!()
    }

    fn read_at(&self, offset: usize, buf: &mut [u8]) -> usize {
        let mut read_size = 0;
        if let Ok(ret_v) = self.fs.ext4.exclusive_access().fuse_read(
            self.ino,
            0,
            offset as i64,
            buf.len() as u32,
            0,
            None,
        ) {
            read_size += ret_v.len();
            buf.copy_from_slice(&ret_v);
        }
        read_size
    }

    fn write_at(&self, offset: usize, buf: &[u8]) -> usize {
        if let Ok(write_size) =
            self.fs
                .ext4
                .exclusive_access()
                .fuse_write(self.ino, 0, offset as i64, buf, 0, 0, None)
        {
            write_size
        } else {
            0
        }
    }
}
