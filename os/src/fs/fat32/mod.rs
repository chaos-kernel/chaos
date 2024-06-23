mod dentry;
mod fat;
pub mod file_system;
mod inode;
mod super_block;

const CLUSTER_SIZE: usize = 4096;
