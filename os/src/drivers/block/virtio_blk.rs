use core::ptr::NonNull;

use super::BlockDevice;
use crate::config::{KERNEL_SPACE_OFFSET, PAGE_SIZE};

use crate::mm::{
    frame_alloc_contiguous, frame_dealloc, FrameTracker, KernelAddr, PhysAddr, PhysPageNum,
    VirtAddr, KERNEL_SPACE,
};
use crate::sync::UPSafeCell;
use alloc::vec::Vec;
use lazy_static::*;
// use virtio_drivers::{Hal, VirtIOBlk, VirtIOHeader};
use virtio_drivers::device::blk::VirtIOBlk;
use virtio_drivers::transport::mmio::{MmioTransport, VirtIOHeader};
use virtio_drivers::{BufferDirection, Hal};

#[allow(unused)]
const VIRTIO0: usize = 0x10001000 + KERNEL_SPACE_OFFSET * PAGE_SIZE;
/// VirtIOBlock device driver strcuture for virtio_blk device
pub struct VirtIOBlock(UPSafeCell<VirtIOBlk<VirtioHal, MmioTransport>>);

lazy_static! {
    /// The global io data queue for virtio_blk device
    static ref QUEUE_FRAMES: UPSafeCell<Vec<FrameTracker>> = unsafe { UPSafeCell::new(Vec::new()) };
}

unsafe impl Send for VirtIOBlock {}
unsafe impl Sync for VirtIOBlock {}

impl BlockDevice for VirtIOBlock {
    /// Read a block from the virtio_blk device
    fn read_block(&self, block_id: usize, buf: &mut [u8]) {
        let mut res = self.0.exclusive_access().read_blocks(block_id, buf);
        // debug!("read_block: {:?}, block_id: {:}", res, block_id);
        if res.is_err() {
            error!("Error when reading VirtIOBlk, block_id {}", block_id);
            let mut times = 0 as usize;
            while res.is_err() {
                warn!("read_block: retrying block_id: {:}", block_id);
                res = self.0.exclusive_access().read_blocks(block_id, buf);
                times += 1;
                if times > 10 {
                    panic!("read_block {}: failed after 10 retries", block_id);
                }
            }
            warn!(
                "read_block: block_id: {:} success after {:} retries",
                block_id, times
            );
            // debug!("buf: {:#x?}", buf);
            res.unwrap()
        } else {
            res.unwrap()
        }
    }
    ///
    fn write_block(&self, block_id: usize, buf: &[u8]) {
        self.0
            .exclusive_access()
            .write_blocks(block_id, buf)
            .expect("Error when writing VirtIOBlk");
    }
}

impl VirtIOBlock {
    #[allow(unused)]
    /// Create a new VirtIOBlock driver with VIRTIO0 base_addr for virtio_blk device
    pub fn new() -> Self {
        unsafe {
            let header = &mut *(VIRTIO0 as *mut VirtIOHeader);
            Self(UPSafeCell::new(
                VirtIOBlk::<VirtioHal, MmioTransport>::new(
                    MmioTransport::new(header.into()).unwrap(),
                )
                .unwrap(),
            ))
        }
    }
}

pub struct VirtioHal;

unsafe impl Hal for VirtioHal {
    /// Allocates and zeroes the given number of contiguous physical pages of DMA memory for VirtIO
    /// use.
    fn dma_alloc(
        pages: usize,
        _direction: BufferDirection,
    ) -> (virtio_drivers::PhysAddr, NonNull<u8>) {
        let (frames, root_ppn) = frame_alloc_contiguous(pages);
        let pa: PhysAddr = root_ppn.into();
        (pa.0, unsafe {
            NonNull::new_unchecked(KernelAddr::from(pa).0 as *mut u8)
        })
    }
    /// Deallocates the given contiguous physical DMA memory pages.
    unsafe fn dma_dealloc(
        paddr: virtio_drivers::PhysAddr,
        _vaddr: NonNull<u8>,
        pages: usize,
    ) -> i32 {
        let pa = PhysAddr::from(paddr);
        let mut ppn_base: PhysPageNum = pa.into();
        for _ in 0..pages {
            frame_dealloc(ppn_base);
            ppn_base.0 += 1;
        }
        0
    }
    /// Converts a physical address used for MMIO to a virtual address which the driver can access.
    unsafe fn mmio_phys_to_virt(paddr: virtio_drivers::PhysAddr, size: usize) -> NonNull<u8> {
        NonNull::new_unchecked(KernelAddr::from(PhysAddr::from(paddr)).0 as *mut u8)
    }
    /// Shares the given memory range with the device, and returns the physical address that the
    /// device can use to access it.
    unsafe fn share(buffer: NonNull<[u8]>, direction: BufferDirection) -> virtio_drivers::PhysAddr {
        unsafe {
            KERNEL_SPACE
                .exclusive_access()
                .page_table
                .translate_va(VirtAddr::from(buffer.as_ptr() as *const usize as usize))
                .unwrap()
                .0
        }
    }
    /// Unshares the given memory range from the device and (if necessary) copies it back to the
    /// original buffer.
    unsafe fn unshare(
        paddr: virtio_drivers::PhysAddr,
        buffer: NonNull<[u8]>,
        direction: BufferDirection,
    ) {
        //todo!();
    }
}
