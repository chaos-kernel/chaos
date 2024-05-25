mod super_block;
pub mod file_system;
mod inode;
mod dentry;
mod fat;

const CLUSTER_SIZE: usize = 4096;