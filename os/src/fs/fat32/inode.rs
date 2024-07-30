use alloc::{
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};
use core::cmp::min;

use super::{
    dentry::{Fat32Dentry, FileAttributes},
    fs::Fat32FS,
    CLUSTER_SIZE,
};
use crate::{
    block::block_dev::BlockDevice,
    fs::{
        dentry::Dentry,
        file::File,
        fs::FileSystemType,
        inode::{Inode, InodeType, Stat, StatMode},
    },
    mm::UserBuffer,
};

pub struct Fat32Inode {
    pub type_:         Fat32InodeType,
    pub dentry:        Option<Arc<Fat32Dentry>>,
    pub start_cluster: usize,
    pub bdev:          Arc<dyn BlockDevice>,
    pub fs:            Arc<Fat32FS>,
}

impl Inode for Fat32Inode {
    fn fstype(&self) -> FileSystemType {
        FileSystemType::VFAT
    }
    fn lookup(self: Arc<Self>, name: &str) -> Option<Arc<Dentry>> {
        let fs = self.fs.as_ref();
        let mut sector_id = fs.fat.cluster_id_to_sector_id(self.start_cluster).unwrap();
        let mut offset = 0;
        while let Some(dentry) = fs.get_dentry(&mut sector_id, &mut offset) {
            let type_ = if dentry.is_file() {
                Fat32InodeType::File
            } else if dentry.is_dir() {
                Fat32InodeType::Dir
            } else {
                Fat32InodeType::VolumeId
            };
            // found the dentry
            if dentry.name() == name {
                let fat32inode = Fat32Inode {
                    type_,
                    start_cluster: dentry.start_cluster_id(),
                    fs: Arc::clone(&self.fs),
                    bdev: Arc::clone(&self.bdev),
                    dentry: Some(Arc::new(dentry)),
                };
                let dentry = Dentry::new(name, Arc::new(fat32inode));
                return Some(Arc::new(dentry));
            }
        }
        None
    }

    fn create(self: Arc<Self>, name: &str, type_: InodeType) -> Option<Arc<Dentry>> {
        if self.clone().lookup(name).is_some() {
            return None;
        }
        let fs = self.fs.as_ref();
        let attr = match type_ {
            InodeType::Regular => FileAttributes::ARCHIVE,
            InodeType::Directory => FileAttributes::DIRECTORY,
            _ => FileAttributes::ARCHIVE,
        };
        let start_cluster = fs.fat.alloc_new_cluster().unwrap();
        let dentry = fs
            .insert_dentry(self.start_cluster, name.to_string(), attr, 0, start_cluster)
            .unwrap();
        let type_ = if type_ == InodeType::Regular {
            Fat32InodeType::File
        } else {
            Fat32InodeType::Dir
        };
        let fat32inode = Fat32Inode {
            type_,
            start_cluster,
            fs: Arc::clone(&self.fs),
            bdev: Arc::clone(&self.bdev),
            dentry: Some(Arc::new(dentry)),
        };
        let dentry = Dentry::new(name, Arc::new(fat32inode));
        Some(Arc::new(dentry))
    }

    fn link(self: Arc<Self>, _name: &str, _target: Arc<Dentry>) -> bool {
        warn!("FAT32 does not support link");
        false
    }

    fn unlink(self: Arc<Self>, name: &str) -> bool {
        let fs = self.fs.as_ref();
        let mut sector_id = fs.fat.cluster_id_to_sector_id(self.start_cluster).unwrap();
        let mut offset = 0;
        while let Some(dentry) = fs.get_dentry(&mut sector_id, &mut offset) {
            if dentry.name() == name {
                fs.remove_dentry(&dentry);
                return true;
            }
        }
        false
    }

    fn ls(&self) -> Vec<String> {
        let fs = self.fs.as_ref();
        let mut v = Vec::new();
        let mut sector_id = fs.fat.cluster_id_to_sector_id(self.start_cluster).unwrap();
        let mut offset = 0;
        while let Some(dentry) = fs.get_dentry(&mut sector_id, &mut offset) {
            v.push(dentry.name());
        }
        v
    }

    fn read_at(&self, offset: usize, buf: &mut [u8]) -> usize {
        let fs = self.fs.as_ref();
        let cluster_id = self.start_cluster;
        let cluster_chain = fs.cluster_chain(cluster_id);
        let mut read_size = 0;
        let mut pos = 0;
        let mut cluster_buf = [0u8; CLUSTER_SIZE];
        for cluster_id in cluster_chain {
            if pos < offset {
                let pass_size = min(CLUSTER_SIZE, offset - pos);
                pos += pass_size;
                if pass_size == CLUSTER_SIZE {
                    continue;
                }
            }
            let dentry = self.dentry.clone().unwrap();
            fs.read_cluster(cluster_id, &mut cluster_buf);
            let copy_size = min(dentry.file_size() - pos, buf.len() - read_size);
            buf[read_size..read_size + copy_size]
                .copy_from_slice(&cluster_buf[pos % CLUSTER_SIZE..pos % CLUSTER_SIZE + copy_size]);
            read_size += copy_size;
            pos += copy_size;
            if read_size >= buf.len() || pos >= dentry.file_size() {
                break;
            }
        }
        read_size
    }

    fn write_at(&self, offset: usize, buf: &[u8]) -> usize {
        self.increase_size(offset + buf.len());
        let fs = self.fs.as_ref();
        let cluster_id = self.start_cluster;
        let cluster_chain = fs.cluster_chain(cluster_id);
        let mut write_size = 0;
        let mut pos = 0;
        let mut cluster_buf = [0u8; CLUSTER_SIZE];
        for cluster_id in cluster_chain {
            if pos < offset {
                let pass_size = min(CLUSTER_SIZE, offset - pos);
                pos += pass_size;
                if pass_size == CLUSTER_SIZE {
                    continue;
                }
            }
            fs.read_cluster(cluster_id, &mut cluster_buf);
            let copy_size = min(buf.len() - write_size, CLUSTER_SIZE - pos % CLUSTER_SIZE);
            cluster_buf[pos % CLUSTER_SIZE..pos % CLUSTER_SIZE + copy_size]
                .copy_from_slice(&buf[write_size..write_size + copy_size]);
            fs.write_cluster(cluster_id, &cluster_buf);
            write_size += copy_size;
            pos += copy_size;
            if write_size >= buf.len() {
                break;
            }
        }
        write_size
    }

    fn clear(&self) {
        self.set_file_size(0);
    }

    fn rename(self: Arc<Self>, _old_name: &str, _new_name: &str) -> bool {
        todo!("FAT32 rename");
    }

    fn mkdir(self: Arc<Self>, _name: &str) -> bool {
        todo!("FAT32 mkdir");
    }

    fn rmdir(self: Arc<Self>, _name: &str) -> bool {
        todo!("FAT32 rmdir");
    }
}

impl File for Fat32Inode {
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
            let read_size = self.read_at(total_read_size, slice);
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
            let len = self.read_at(total_read_size, &mut buffer);
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
            let write_size = self.write_at(total_write_size, slice);
            if write_size == 0 {
                break;
            }
            total_write_size += write_size;
        }
        total_write_size
    }

    fn fstat(&self) -> Option<Stat> {
        let st_mode = match self.type_ {
            Fat32InodeType::File => StatMode::FILE.bits(),
            Fat32InodeType::Dir => StatMode::DIR.bits(),
            _ => StatMode::NULL.bits(),
        };
        Some(Stat::new(
            0,
            0,
            st_mode,
            1,
            0,
            self.dentry.as_ref().unwrap().file_size() as i64,
            0,
            0,
            0,
        ))
    }
}

impl Fat32Inode {
    pub fn is_dir(&self) -> bool {
        self.type_ == Fat32InodeType::Dir
    }

    pub fn is_file(&self) -> bool {
        self.type_ == Fat32InodeType::File
    }

    pub fn file_size(&self) -> usize {
        self.dentry.as_ref().unwrap().file_size()
    }

    pub fn set_file_size(&self, size: usize) {
        self.dentry.as_ref().unwrap().set_file_size(size);
    }

    pub fn increase_size(&self, size: usize) {
        if size < self.file_size() {
            return;
        }
        self.set_file_size(size);
        let fs = self.fs.as_ref();
        let cluster_chain = fs.cluster_chain(self.start_cluster);
        if cluster_chain.len() * CLUSTER_SIZE >= size {
            return;
        }
        let mut last_cluster_id = *cluster_chain.last().unwrap();
        while cluster_chain.len() * CLUSTER_SIZE < size {
            last_cluster_id = fs.fat.increase_cluster(last_cluster_id).unwrap();
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Fat32InodeType {
    File,
    Dir,
    VolumeId,
}
