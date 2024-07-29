use alloc::{
    string::{String, ToString},
    sync::Arc,
};

use super::inode::Inode;

pub struct Dentry {
    name:  String,
    inode: Arc<dyn Inode>,
}

impl Dentry {
    pub fn new(name: &str, inode: Arc<dyn Inode>) -> Self {
        Self {
            name: name.to_string(),
            inode,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn inode(&self) -> Arc<dyn Inode> {
        Arc::clone(&self.inode)
    }
}
