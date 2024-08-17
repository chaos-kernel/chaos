mod qemu;
mod visionfive2;

#[cfg(feature = "visionfive2")]
pub use visionfive2::*;

#[cfg(feature = "qemu")]
pub use qemu::*;

// 这里按照编译feature暴露两批不同接口，实现适配不同平台