use core::cmp::min;

use alloc::{string::{String, ToString}, sync::Arc, vec::Vec};
use spin::Mutex;

use crate::{block::block_dev::BlockDevice, fs::{fat32::CLUSTER_SIZE, inode::{Inode, Stat, StatMode}}};

use super::{dentry::{self, Fat32Dentry, FileAttributes}, file_system::Fat32FS};

#[derive(Clone)]
pub struct Fat32Inode {
    pub type_: Fat32InodeType,
    pub dentry: Arc<Mutex<Fat32Dentry>>,
    pub start_cluster: usize,
    pub fs: Arc<Mutex<Fat32FS>>,
    pub bdev: Arc<dyn BlockDevice>,
}

impl Inode for Fat32Inode {
    fn fstat(self: Arc<Fat32Inode>) -> Stat {
        let st_mode = match self.type_ {
            Fat32InodeType::File => StatMode::FILE.bits(),
            Fat32InodeType::Dir => StatMode::DIR.bits(),
            _ => StatMode::NULL.bits(),
            
        };
        debug!("fstat: st_mode: {:#x} size: {:?}", st_mode,self.dentry.lock().file_size());
        Stat::new(
            0,
            0,
            st_mode,
            1,
            0,
            self.dentry.lock().file_size() as i64 ,
            0,
            0,
            0,
        )
    }
    
    fn find(self: Arc<Self>, path: &str) -> Option<Arc<dyn Inode>> {
        let mut split = path.splitn(2, '/');
        let name = split.next().unwrap();
        let next = split.next();
        if name == "." {
            if next.is_some() {
                return self.find(next.unwrap());
            } else {
                return Some(self);
            }
        }
        let fs = self.fs.lock();
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
            if dentry.name() == name {
                let inode = Arc::new(Fat32Inode {
                    type_,
                    start_cluster: dentry.start_cluster_id(),
                    fs: Arc::clone(&self.fs),
                    bdev: Arc::clone(&self.bdev),
                    dentry: Arc::new(Mutex::new(dentry)),
                });
                if next.is_some() {
                    return Arc::clone(&inode).find(split.next().unwrap());
                } else {
                    return Some(inode);
                }
            }
        }
        None
    }
    
    fn create(self: Arc<Self>, name: &str, stat: StatMode) -> Option<Arc<dyn Inode>> {
        if self.clone().find(name).is_some() {
            return None;
        }
        let fs = self.fs.lock();
        let attr = match stat {
            StatMode::FILE => FileAttributes::ARCHIVE,
            StatMode::DIR => FileAttributes::DIRECTORY,
            _ => FileAttributes::ARCHIVE,
        };
        let start_cluster = fs.fat.alloc_new_cluster().unwrap();
        let dentry = fs.insert_dentry(self.start_cluster, name.to_string(), attr, 0, start_cluster).unwrap();
        let type_ = if stat == StatMode::FILE {
            Fat32InodeType::File
        } else {
            Fat32InodeType::Dir
        };
        Some(Arc::new(Fat32Inode {
                type_,
                start_cluster,
                fs: Arc::clone(&self.fs),
                bdev: Arc::clone(&self.bdev),
                dentry: Arc::new(Mutex::new(dentry)),
            })
        )
    }
    
    fn link(self: Arc<Self>, old_name: &str, new_name: &str) -> Option<Arc<dyn Inode>> {
        todo!()
    }
    
    fn unlink(self: Arc<Self>, path: &str) -> bool {
        let mut split = path.splitn(2, '/');
        let name = split.next().unwrap();
        let next = split.next();
        if name == "." {
            if next.is_some() {
                return self.unlink(next.unwrap());
            } else {
                return false;
            }
        }
        let fs = self.fs.lock();
        let mut sector_id = fs.fat.cluster_id_to_sector_id(self.start_cluster).unwrap();
        let mut offset = 0;
        while let Some(dentry) = fs.get_dentry(&mut sector_id, &mut offset) {
            if dentry.name() == name {
                if next.is_some() {
                    let inode = Arc::new(Fat32Inode {
                        type_: if dentry.is_file() {
                            Fat32InodeType::File
                        } else if dentry.is_dir() {
                            Fat32InodeType::Dir
                        } else {
                            Fat32InodeType::VolumeId
                        },
                        start_cluster: dentry.start_cluster_id(),
                        fs: Arc::clone(&self.fs),
                        bdev: Arc::clone(&self.bdev),
                        dentry: Arc::new(Mutex::new(dentry)),
                    });
                    return inode.unlink(next.unwrap());
                } else {
                    fs.remove_dentry(&dentry);
                    return true;
                }
            }
        }
        true
    }
    
    fn ls(self: Arc<Self>) -> Vec<String> {
        let fs = self.fs.lock();
        let mut v = Vec::new();
        let mut sector_id = fs.fat.cluster_id_to_sector_id(self.start_cluster).unwrap();
        let mut offset = 0;
        while let Some(dentry) = fs.get_dentry(&mut sector_id, &mut offset) {
            v.push(dentry.name());
        }
        v
    }
    
    fn read_at(self: Arc<Self>, offset: usize, buf: &mut [u8]) -> usize {
        let fs = self.fs.lock();
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
            let dentry = self.dentry.lock();
            fs.read_cluster(cluster_id, &mut cluster_buf);
            let copy_size = min(dentry.file_size() as usize - pos, buf.len() - read_size);
            buf[read_size..read_size + copy_size].copy_from_slice(&cluster_buf[pos % CLUSTER_SIZE..pos % CLUSTER_SIZE + copy_size]);
            read_size += copy_size;
            pos += copy_size;
            if read_size >= buf.len() || pos >= dentry.file_size() as usize {
                break;
            }
        }
        read_size
    }
    
    fn write_at(self: Arc<Self>, offset: usize, buf: &[u8]) -> usize {
        // self.increase_size(offset + buf.len());
        let fs = self.fs.lock();
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
            cluster_buf[pos % CLUSTER_SIZE..pos % CLUSTER_SIZE + copy_size].copy_from_slice(&buf[write_size..write_size + copy_size]);
            fs.write_cluster(cluster_id, &cluster_buf);
            write_size += copy_size;
            pos += copy_size;
            if write_size >= buf.len() {
                break;
            }
        }
        write_size
    }
    
    fn clear(self: Arc<Self>) {
        todo!()
    } 

    /// Get the current directory name
    fn current_dirname(self: Arc<Self>) -> Option<String> {
        if self.type_ != Fat32InodeType::Dir {
            return None;
        }
        let fs = self.fs.lock();
        let mut sector_id = fs.fat.cluster_id_to_sector_id(self.start_cluster).unwrap();
        let mut offset = 0;
        while let Some(dentry) = fs.get_dentry(&mut sector_id, &mut offset) {
            if dentry.name() == "." {
                return Some(dentry.name());
            }
        }
        None
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
        self.dentry.lock().file_size()
    }

    pub fn set_file_size(&self, size: usize) {
        self.dentry.lock().set_file_size(size);
    }

    pub fn increase_size(&self, size: usize) {
        if size < self.file_size() {
            return;
        }
        self.set_file_size(size);
        let fs = self.fs.lock();
        let cluster_chain = fs.cluster_chain(self.start_cluster);
        if cluster_chain.len() * CLUSTER_SIZE >= size {
            return;
        }
        let mut last_cluster_id = cluster_chain.last().unwrap().clone();
        while cluster_chain.len() * CLUSTER_SIZE < size {
            last_cluster_id = fs.fat.increase_cluster(last_cluster_id).unwrap();
        }
    }
}

#[derive(PartialEq, Clone, Copy)]
pub enum Fat32InodeType {
    File,
    Dir,
    VolumeId,
}