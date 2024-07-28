use alloc::{
    string::{String, ToString},
    sync::{Arc, Weak},
    vec::Vec,
};

use super::inode::Inode;

pub struct Dentry {
    name:   String,
    inode:  Arc<dyn Inode>,
    parent: Option<Weak<Dentry>>,
    child:  Vec<Arc<Dentry>>, // cached children
}

impl Dentry {
    pub fn new(name: &str, inode: Arc<dyn Inode>, parent: Option<Weak<Dentry>>) -> Self {
        Self {
            name: name.to_string(),
            inode,
            parent,
            child: Vec::new(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn inode(&self) -> Arc<dyn Inode> {
        Arc::clone(&self.inode)
    }
}
