use crate::mm::MapPermission;

pub const CLOCK_FREQ: usize = 400_0000;
pub const BLOCK_CACHE_FRAMES: usize = 1024 * 4 * 4;
pub const HEAP_SIZE: usize = 0x40_00000;

pub const PERMISSION_RW: MapPermission = MapPermission::union(MapPermission::R, MapPermission::W);
/// vf2的设备地址空间
/// The base address of control registers in VIRT_TEST/RTC/Virtio_Block device
pub const MMIO: &[(usize, usize, MapPermission)] = &[
    (0x17040000, 0x10000, PERMISSION_RW),     // RTC
    (0xc000000, 0x4000000, PERMISSION_RW),    //PLIC
    (0x00_1000_0000, 0x10000, PERMISSION_RW), // UART
];

pub type BlockDeviceImpl = crate::drivers::block::SDCard;

pub fn shutdown() -> ! {
    // 直接死循环
    loop {}
}
