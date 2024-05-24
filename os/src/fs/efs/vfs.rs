//! index node(inode, namely file control block) layer
//!
//! The data struct and functions for the inode layer that service file-related system calls
//!
//! NOTICE: The difference between [`Inode`] and [`DiskInode`]  can be seen from their names: DiskInode in a relatively fixed location within the disk block, while Inode Is a data structure placed in memory that records file inode information.
use crate::fs::inode::{Inode, InodeMeta};

use super::{
    block_cache_sync_all, get_block_cache, BlockDevice, DirEntry, DiskInode, DiskInodeType,
    EasyFileSystem, DIRENT_SZ,
};
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::{Mutex, MutexGuard};

pub struct EfsInode {
    meta: InodeMeta,
}

impl Inode for EfsInode {
    fn meta(&self) -> InodeMeta {
        self.meta.clone()
    }

    fn set_meta(&mut self, meta: InodeMeta) {
        self.meta = meta;
    }

    fn fstat(&self) -> (usize, u32) {
        self.read_disk_inode(|disk_inode| {
            (self.meta.block_id, disk_inode.nlink)
        })
    }

    fn create(&self, name: &str) -> Option<Arc<dyn Inode>> {
        let mut fs = self.meta.fs.lock();
        let op = |root_inode: &mut DiskInode| {
            // assert it is a directory
            assert!(root_inode.is_dir());
            // has the file been created?
            self.find_inode_id(name, root_inode)
        };
        if self.modify_disk_inode(op).is_some() {
            return None;
        }
        // create a new file
        // alloc a inode with an indirect block
        let new_inode_id = fs.alloc_inode();
        // initialize inode
        let (new_inode_block_id, new_inode_block_offset) = fs.get_disk_inode_pos(new_inode_id);
        get_block_cache(new_inode_block_id as usize, Arc::clone(&self.meta.block_device))
            .lock()
            .modify(new_inode_block_offset, |new_inode: &mut DiskInode| {
                new_inode.initialize(DiskInodeType::File);
            });
        self.modify_disk_inode(|root_inode| {
            // append file in the dirent
            let file_count = (root_inode.size as usize) / DIRENT_SZ;
            let new_size = (file_count + 1) * DIRENT_SZ;
            // increase size
            self.increase_size(new_size as u32, root_inode, &mut fs);
            // write dirent
            let dirent = DirEntry::new(name, new_inode_id);
            root_inode.write_at(
                file_count * DIRENT_SZ,
                dirent.as_bytes(),
                &self.meta.block_device,
            );
        });

        let (block_id, block_offset) = fs.get_disk_inode_pos(new_inode_id);
        block_cache_sync_all();
        // return inode
        Some(Arc::new(Self::new(
            block_id,
            block_offset,
            self.meta.fs.clone(),
            self.meta.block_device.clone(),
        )))
        // release efs lock automatically by compiler
    }

    fn find(&self, name: &str) -> Option<Arc<dyn Inode>> {
        let fs = self.meta.fs.lock();
        self.read_disk_inode(|disk_inode| {
            self.find_inode_id(name, disk_inode).map(|inode_id| {
                let (block_id, block_offset) = fs.get_disk_inode_pos(inode_id);
                Arc::new(Self::new(
                    block_id,
                    block_offset,
                    self.meta.fs.clone(),
                    self.meta.block_device.clone(),
                )) as Arc<dyn Inode>
            })
        })
    }

    fn link(&self, old_name: &str, new_name: &str) -> Option<Arc<dyn Inode>>{
        if self.find(new_name).is_some() {
            return None;
        }
        let inode_id = self.read_disk_inode(|disk_inode| {
            self.find_inode_id(old_name, disk_inode)
        });
        if inode_id.is_none() {
            return None;
        }
        let inode_id = inode_id.unwrap();
        // increase count
        let mut fs = self.meta.fs.lock();
        let (block_id, block_offset) = fs.get_disk_inode_pos(inode_id);
        get_block_cache(block_id as usize, self.meta.block_device.clone())
            .lock()
            .modify(block_offset, |inode: &mut DiskInode| {
                inode.nlink += 1;
            });
        self.modify_disk_inode(|root_inode| {
            let file_count = (root_inode.size as usize) / DIRENT_SZ;
            let new_size = (file_count + 1) * DIRENT_SZ;
            self.increase_size(new_size as u32, root_inode, &mut fs);
            let new_dirent = DirEntry::new(new_name, inode_id);
            root_inode.write_at(file_count * DIRENT_SZ, new_dirent.as_bytes(), &self.meta.block_device);
        });
        block_cache_sync_all();
        Some(Arc::new(Self::new(
            block_id,
            block_offset,
            self.meta.fs.clone(),
            self.meta.block_device.clone(),
        )))
    }

    fn unlink(&self, name: &str) -> bool {
        let inode_id = self.read_disk_inode(|disk_inode| {
            self.find_inode_id(name, disk_inode)
        });
        if inode_id.is_none() {
            return false;
        }
        let inode_id = inode_id.unwrap();
        // decrease count
        let fs = self.meta.fs.lock();
        let (block_id, block_offset) = fs.get_disk_inode_pos(inode_id);
        drop(fs);
        if get_block_cache(block_id as usize, self.meta.block_device.clone())
            .lock()
            .modify(block_offset, |inode: &mut DiskInode| {
                inode.nlink -= 1;
                inode.nlink == 0
            })
        {
            self.find(name).unwrap().clear();
        }
        self.modify_disk_inode(|root_inode| {
            self.remove_dirent_by_name(name, root_inode);
        });
        block_cache_sync_all();
        true
    }

    fn ls(&self) -> Vec<String> {
        let _fs = self.meta.fs.lock();
        self.read_disk_inode(|disk_inode| {
            let file_count = (disk_inode.size as usize) / DIRENT_SZ;
            let mut v: Vec<String> = Vec::new();
            for i in 0..file_count {
                let mut dirent = DirEntry::empty();
                assert_eq!(
                    disk_inode.read_at(i * DIRENT_SZ, dirent.as_bytes_mut(), &self.meta.block_device,),
                    DIRENT_SZ,
                );
                v.push(String::from(dirent.name()));
            }
            v
        })
    }

    fn read_at(&self, offset: usize, buf: &mut [u8]) -> usize {
        let _fs = self.meta.fs.lock();
        self.read_disk_inode(|disk_inode| disk_inode.read_at(offset, buf, &self.meta.block_device))
    }

    fn write_at(&self, offset: usize, buf: &[u8]) -> usize {
        let mut fs = self.meta.fs.lock();
        let size = self.modify_disk_inode(|disk_inode| {
            self.increase_size((offset + buf.len()) as u32, disk_inode, &mut fs);
            disk_inode.write_at(offset, buf, &self.meta.block_device)
        });
        block_cache_sync_all();
        size
    }

    fn clear(&self) {
        let mut fs = self.meta.fs.lock();
        self.modify_disk_inode(|disk_inode| {
            let size = disk_inode.size;
            let data_blocks_dealloc = disk_inode.clear_size(&self.meta.block_device);
            assert!(data_blocks_dealloc.len() == DiskInode::total_blocks(size) as usize);
            for data_block in data_blocks_dealloc.into_iter() {
                fs.dealloc_data(data_block);
            }
        });
        block_cache_sync_all();
    }
}

impl EfsInode {
    /// We should not acquire efs lock here.
    pub fn new(
        block_id: u32,
        block_offset: usize,
        fs: Arc<Mutex<EasyFileSystem>>,
        block_device: Arc<dyn BlockDevice>,
    ) -> Self {
        Self {
            meta: InodeMeta::new(
                block_id as usize,
                block_offset,
                fs,
                block_device,
            ),
        }
    }

    /// read the content of the disk inode on disk with 'f' function
    fn read_disk_inode<V>(&self, f: impl FnOnce(&DiskInode) -> V) -> V {
        get_block_cache(self.meta.block_id, Arc::clone(&self.meta.block_device))
            .lock()
            .read(self.meta.block_offset, f)
    }
    
    /// modify the content of the disk inode on disk with 'f' function
    fn modify_disk_inode<V>(&self, f: impl FnOnce(&mut DiskInode) -> V) -> V {
        get_block_cache(self.meta.block_id, Arc::clone(&self.meta.block_device))
            .lock()
            .modify(self.meta.block_offset, f)
    }

    /// find the disk inode id according to the file with 'name' by search the directory entries in the disk inode with Directory type
    fn find_inode_id(&self, name: &str, disk_inode: &DiskInode) -> Option<u32> {
        // assert it is a directory
        assert!(disk_inode.is_dir());
        let file_count = (disk_inode.size as usize) / DIRENT_SZ;
        let mut dirent = DirEntry::empty();
        for i in 0..file_count {
            assert_eq!(
                disk_inode.read_at(DIRENT_SZ * i, dirent.as_bytes_mut(), &self.meta.block_device,),
                DIRENT_SZ,
            );
            if dirent.name() == name {
                return Some(dirent.inode_id() as u32);
            }
        }
        None
    }

    /// Remove a directory entry by name
    fn remove_dirent_by_name(&self, name: &str, disk_inode: &mut DiskInode) {
        // assert it is a directory
        assert!(disk_inode.is_dir());
        let file_count = (disk_inode.size as usize) / DIRENT_SZ;
        let mut dirent = DirEntry::empty();
        for i in 0..file_count {
            assert_eq!(
                disk_inode.read_at(DIRENT_SZ * i, dirent.as_bytes_mut(), &self.meta.block_device,),
                DIRENT_SZ,
            );
            if dirent.name() == name {
                disk_inode.write_at(DIRENT_SZ * i, DirEntry::empty().as_bytes(), &self.meta.block_device);
                return;
            }
        }
    }

    /// increase the size of file( also known as 'disk inode')
    fn increase_size(
        &self,
        new_size: u32,
        disk_inode: &mut DiskInode,
        fs: &mut MutexGuard<EasyFileSystem>,
    ) {
        if new_size < disk_inode.size {
            return;
        }
        let blocks_needed = disk_inode.blocks_num_needed(new_size);
        let mut v: Vec<u32> = Vec::new();
        for _ in 0..blocks_needed {
            v.push(fs.alloc_data());
        }
        disk_inode.increase_size(new_size, v, &self.meta.block_device);
    }
}
