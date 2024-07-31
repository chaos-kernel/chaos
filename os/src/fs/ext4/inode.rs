use alloc::{string::String, sync::Arc, vec::Vec};

use ext4_rs::{Ext4File, Ext4InodeRef};

use super::fs::Ext4FS;
use crate::{
    fs::{
        dentry::Dentry,
        file::File,
        fs::FileSystemType,
        inode::{Inode, InodeType, Stat},
    },
    sync::UPSafeCell,
};

pub struct Ext4Inode {
    pub fs:    Arc<Ext4FS>,
    pub ino:   u32,
    pub inner: UPSafeCell<Ext4InodeInner>,
}

pub struct Ext4InodeInner {
    pub fpos: usize,
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
            fs:    self.fs.clone(),
            ino:   file.inode,
            inner: unsafe { UPSafeCell::new(Ext4InodeInner { fpos: 0 }) },
        };
        let dentry = Dentry::new(name, Arc::new(inode));
        Some(Arc::new(dentry))
    }

    fn unlink(self: Arc<Self>, name: &str) -> bool {
        self.fs.ext4.ext4_file_remove(self.ino, name).is_ok()
    }

    fn link(self: Arc<Self>, _name: &str, _target: Arc<Dentry>) -> bool {
        todo!()
    }

    fn rename(self: Arc<Self>, _old_name: &str, _new_name: &str) -> bool {
        todo!()
    }

    fn mkdir(self: Arc<Self>, name: &str) -> bool {
        self.fs.ext4.ext4_dir_mk(self.ino, name).is_ok()
    }

    fn rmdir(self: Arc<Self>, name: &str) -> bool {
        self.fs.ext4.ext4_dir_remove(self.ino, name).is_ok()
    }

    fn ls(&self) -> Vec<String> {
        self.fs
            .ext4
            .read_dir_entry(self.ino as u64)
            .iter()
            .map(|x| x.get_name())
            .collect()
    }

    fn read_at(&self, offset: usize, buf: &mut [u8]) -> usize {
        let mut file = Ext4File::new();
        file.inode = self.ino;
        file.fpos = offset;
        file.fsize = offset as u64;
        let mut read_size = 0;
        let _ = self
            .fs
            .ext4
            .ext4_file_read(&mut file, buf, buf.len(), &mut read_size);
        read_size
    }

    fn write_at(&self, offset: usize, buf: &[u8]) -> usize {
        let inode_ref = Ext4InodeRef::get_inode_ref(Arc::downgrade(&self.fs.ext4), self.ino);
        let mut file = Ext4File::new();
        file.fpos = offset;
        file.fsize = inode_ref.inner.inode.inode_get_size();
        self.fs.ext4.ext4_file_write(&mut file, buf, buf.len());
        buf.len()
    }
}

impl File for Ext4Inode {
    fn fstat(&self) -> Option<Stat> {
        todo!()
    }
    fn is_dir(&self) -> bool {
        todo!()
    }
    fn read(&self, buf: &mut [u8]) -> usize {
        // TODO: 暂时不考虑 pos
        let mut inner = self.inner.exclusive_access(file!(), line!());
        let read_size = self.read_at(inner.fpos, buf);
        inner.fpos += read_size;
        read_size
    }
    fn readable(&self) -> bool {
        true
    }
    fn writable(&self) -> bool {
        true
    }
    fn write(&self, buf: &[u8]) -> usize {
        // 暂时不考虑 pos
        let write_size = self.write_at(0, buf);
        write_size
    }
    fn read_all(&self) -> Vec<u8> {
        todo!()
    }
    fn hang_up(&self) -> bool {
        todo!()
    }
}
