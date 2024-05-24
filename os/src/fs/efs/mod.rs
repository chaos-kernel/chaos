//！ easy-fs
//！
//！ easy-fs is a simple file system implementation.
//!
//! [`EasyFileSystem`]'s disk layout overall design -- [`SuperBlock`] is reflected in the contents of individual sectors on the disk, while the logical file & directory tree structure obtained by parsing the disk layout is accessed through the data structure in memory, which means that it involves access to both disk and memory.
//!
//! They( [`DiskInode`] in disk and [`Inode`] in memory) have different access modes. For the disk, you need to send a request to the disk in a software way to indirectly read and write. so, we also nee to pay special attention to which data structures are store on disk and which are stored in memory.
//!
//! easy-fs itself is divided into different levels, forming a hierarchical and modular design architecture. The easy-fs crate can be roughly divided into five different levels from bottom to top:
//!
//! - Disk block device interface layer
//! - Block cache layer
//! - Disk layout & data structure layer
//! - Disk block manager layer
//! - index node(inode, namely file control block) layer

extern crate alloc;

pub mod bitmap;
pub mod file_system;
pub mod layout;
pub mod inode;

use bitmap::Bitmap;
use crate::block::block_dev;
use crate::block::block_cache::*;
pub use block_dev::BlockDevice;
pub use file_system::EasyFileSystem;
pub use layout::*;
