//! Address Space [`MemorySet`] management of Process

use super::{frame_alloc, FrameTracker};
use super::{PTEFlags, PageTable, PageTableEntry};
use super::{PhysAddr, PhysPageNum, VirtAddr, VirtPageNum};
use super::{StepByOne, VPNRange};
use crate::config::{MEMORY_END, MMAP_BASE, MMIO, PAGE_SIZE, STACK_TOP, TRAMPOLINE};
use crate::sync::UPSafeCell;
use crate::syscall::errno::SUCCESS;
use crate::task::process::Flags;
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::arch::asm;
use lazy_static::*;
use riscv::register::satp;

extern "C" {
    fn stext();
    fn etext();
    fn srodata();
    fn erodata();
    fn sdata();
    fn edata();
    fn sbss_with_stack();
    fn ebss();
    fn ekernel();
    fn strampoline();
}

lazy_static! {
    /// The kernel's initial memory mapping(kernel address space)
    pub static ref KERNEL_SPACE: Arc<UPSafeCell<MemorySet>> =
        Arc::new(unsafe { UPSafeCell::new(MemorySet::new_kernel()) });
}

/// the kernel token
pub fn kernel_token() -> usize {
    KERNEL_SPACE.exclusive_access().token()
}

/// address space
pub struct MemorySet {
    /// page table
    pub page_table: PageTable,
    /// areas
    pub areas: Vec<MapArea>,
    /// heap
    heap_area: BTreeMap<VirtPageNum, FrameTracker>,
    // The memory area formed by mmap does not need to be modified
    // we can use MapArea in Vec to hold FramTracker
    // we set a fixed address as the start address for mmap_area
    // the virtual memorySet is big enough to use it that doesnt concern address conflicts
    pub mmap_area: BTreeMap<VirtPageNum, FrameTracker>,
    // mmap_base will never change
    pub mmap_base: VirtAddr,
    // always aligh to PAGE_SIZE
    pub mmap_end: VirtAddr,
}

impl MemorySet {
    /// Create a new empty `MemorySet`.
    pub fn new_bare() -> Self {
        Self {
            page_table: PageTable::new(),
            areas: Vec::new(),
            heap_area: BTreeMap::new(),
            mmap_area: BTreeMap::new(),
            mmap_base: MMAP_BASE.into(),
            mmap_end: MMAP_BASE.into(),
        }
    }
    /// Get he page table token
    pub fn token(&self) -> usize {
        self.page_table.token()
    }
    /// Assume that no conflicts.
    pub fn insert_framed_area(
        &mut self,
        start_va: VirtAddr,
        end_va: VirtAddr,
        permission: MapPermission,
    ) {
        self.push(
            MapArea::new(start_va, end_va, MapType::Framed, permission),
            None,
        );
    }

    pub fn insert_framed_area_with_data(
        &mut self,
        start_va: VirtAddr,
        end_va: VirtAddr,
        permission: MapPermission,
        data: &[u8],
    ) {
        self.push(
            MapArea::new(start_va, end_va, MapType::Framed, permission),
            Some(data),
        );
    }
    /// check if exist areas conflict with given virtial address
    pub fn is_conflict_with_va(&self, start_va: VirtAddr, end_va: VirtAddr) -> bool {
        if let Some(_) = self
            .areas
            .iter()
            .find(|area| area.is_conflict_with(start_va, end_va))
        {
            true
        } else {
            false
        }
    }
    /// remove an area with start and end virtual address
    pub fn remove_area_with_va(&mut self, start_va: VirtAddr, end_va: VirtAddr) -> bool {
        if let Some(idx) = self.areas.iter_mut().position(|area| {
            area.vpn_range.get_start() == start_va.floor()
                && area.vpn_range.get_end() == end_va.ceil()
        }) {
            self.areas[idx].unmap(&mut self.page_table);
            self.areas.remove(idx);
            true
        } else {
            false
        }
    }
    /// remove a area
    pub fn remove_area_with_start_vpn(&mut self, start_vpn: VirtPageNum) {
        if let Some((idx, area)) = self
            .areas
            .iter_mut()
            .enumerate()
            .find(|(_, area)| area.vpn_range.get_start() == start_vpn)
        {
            area.unmap(&mut self.page_table);
            self.areas.remove(idx);
            unsafe {
                asm!("sfence.vma");
            }
        }
    }
    /// Add a new MapArea into this MemorySet.
    /// Assuming that there are no conflicts in the virtual address
    /// space.
    fn push(&mut self, mut map_area: MapArea, data: Option<&[u8]>) {
        map_area.map(&mut self.page_table);
        if let Some(data) = data {
            map_area.copy_data(&mut self.page_table, data);
        }
        self.areas.push(map_area);
    }
    /// Mention that trampoline is not collected by areas.
    fn map_trampoline(&mut self) {
        self.page_table.map(
            VirtAddr::from(TRAMPOLINE).into(),
            PhysAddr::from(strampoline as usize).into(),
            PTEFlags::R | PTEFlags::X,
        );
    }
    /// Without kernel stacks.
    pub fn new_kernel() -> Self {
        let mut memory_set = Self::new_bare();
        // map trampoline
        memory_set.map_trampoline();
        // map kernel sections
        info!(".text [{:#x}, {:#x})", stext as usize, etext as usize);
        info!(".rodata [{:#x}, {:#x})", srodata as usize, erodata as usize);
        info!(".data [{:#x}, {:#x})", sdata as usize, edata as usize);
        info!(
            ".bss [{:#x}, {:#x})",
            sbss_with_stack as usize, ebss as usize
        );
        info!("mapping .text section");
        memory_set.push(
            MapArea::new(
                (stext as usize).into(),
                (etext as usize).into(),
                MapType::Identical,
                MapPermission::R | MapPermission::X,
            ),
            None,
        );
        info!("mapping .rodata section");
        memory_set.push(
            MapArea::new(
                (srodata as usize).into(),
                (erodata as usize).into(),
                MapType::Identical,
                MapPermission::R,
            ),
            None,
        );
        info!("mapping .data section");
        memory_set.push(
            MapArea::new(
                (sdata as usize).into(),
                (edata as usize).into(),
                MapType::Identical,
                MapPermission::R | MapPermission::W,
            ),
            None,
        );
        info!("mapping .bss section");
        memory_set.push(
            MapArea::new(
                (sbss_with_stack as usize).into(),
                (ebss as usize).into(),
                MapType::Identical,
                MapPermission::R | MapPermission::W,
            ),
            None,
        );
        info!("mapping physical memory");
        memory_set.push(
            MapArea::new(
                (ekernel as usize).into(),
                MEMORY_END.into(),
                MapType::Identical,
                MapPermission::R | MapPermission::W,
            ),
            None,
        );
        info!("mapping memory-mapped registers");
        for pair in MMIO {
            memory_set.push(
                MapArea::new(
                    (*pair).0.into(),
                    ((*pair).0 + (*pair).1).into(),
                    MapType::Identical,
                    MapPermission::R | MapPermission::W,
                ),
                None,
            );
        }
        memory_set
    }
    /// Include sections in elf and trampoline and TrapContext and user stack,
    /// also returns user_sp_base and entry point.
    pub fn from_elf(elf_data: &[u8]) -> (Self, usize, usize, usize) {
        let mut memory_set = Self::new_bare();
        // map trampoline
        memory_set.map_trampoline();
        // map program headers of elf, with U flag
        let elf = xmas_elf::ElfFile::new(elf_data).unwrap();
        let elf_header = elf.header;
        let magic = elf_header.pt1.magic;
        assert_eq!(magic, [0x7f, 0x45, 0x4c, 0x46], "invalid elf!");
        let ph_count = elf_header.pt2.ph_count();
        let mut max_end_vpn = VirtPageNum(0);
        for i in 0..ph_count {
            let ph = elf.program_header(i).unwrap();
            if ph.get_type().unwrap() == xmas_elf::program::Type::Load {
                let start_va: VirtAddr = (ph.virtual_addr() as usize).into();
                let end_va: VirtAddr = ((ph.virtual_addr() + ph.mem_size()) as usize).into();
                let mut map_perm = MapPermission::U;
                let ph_flags = ph.flags();
                if ph_flags.is_read() {
                    map_perm |= MapPermission::R;
                }
                if ph_flags.is_write() {
                    map_perm |= MapPermission::W;
                }
                if ph_flags.is_execute() {
                    map_perm |= MapPermission::X;
                }
                let map_area = MapArea::new(start_va, end_va, MapType::Framed, map_perm);
                max_end_vpn = map_area.vpn_range.get_end();
                memory_set.push(
                    map_area,
                    Some(&elf.input[ph.offset() as usize..(ph.offset() + ph.file_size()) as usize]),
                );
            }
        }
        // map user stack with U flags
        let max_end_va: VirtAddr = max_end_vpn.into();
        let mut user_heap_base: usize = max_end_va.into();
        user_heap_base += PAGE_SIZE;
        (
            memory_set,
            user_heap_base,
            STACK_TOP,
            elf.header.pt2.entry_point() as usize,
        )
    }
    /// Create a new address space by copy code&data from a exited process's address space.
    pub fn from_existed_user(user_space: &Self) -> Self {
        let mut memory_set = Self::new_bare();
        // map trampoline
        memory_set.map_trampoline();
        // copy mmap
        memory_set.mmap_end = user_space.mmap_end;
        // copy data sections/trap_context/user_stack
        for area in user_space.areas.iter() {
            let new_area = MapArea::from_another(area);
            memory_set.push(new_area, None);
            // copy data from another space
            for vpn in area.vpn_range {
                let src_ppn = user_space.translate(vpn).unwrap().ppn();
                let dst_ppn = memory_set.translate(vpn).unwrap().ppn();
                dst_ppn
                    .get_bytes_array()
                    .copy_from_slice(src_ppn.get_bytes_array());
            }
        }
        // copy heap_area
        for (vpn, src_frame) in user_space.heap_area.iter() {
            let dst_frame = frame_alloc().unwrap();
            let dst_ppn = dst_frame.ppn;
            memory_set
                .page_table
                .map(*vpn, dst_ppn, PTEFlags::U | PTEFlags::R | PTEFlags::W);
            memory_set.heap_area.insert(*vpn, dst_frame);

            let src_ppn = src_frame.ppn;
            // copy data
            dst_ppn
                .get_bytes_array()
                .copy_from_slice(src_ppn.get_bytes_array());
        }
        // copy mmap_area
        for (vpn, src_frame) in user_space.mmap_area.iter() {
            let dst_frame = frame_alloc().unwrap();
            let dst_ppn = dst_frame.ppn;
            memory_set
                .page_table
                .map(*vpn, dst_ppn, PTEFlags::U | PTEFlags::R | PTEFlags::W);
            memory_set.mmap_area.insert(*vpn, dst_frame);

            let src_ppn = src_frame.ppn;
            // copy data
            dst_ppn
                .get_bytes_array()
                .copy_from_slice(src_ppn.get_bytes_array());
        }
        memory_set
    }
    /// Change page table by writing satp CSR Register.
    pub fn activate(&self) {
        let satp = self.page_table.token();
        unsafe {
            satp::write(satp);
            asm!("sfence.vma");
        }
    }
    /// Translate a virtual page number to a page table entry
    pub fn translate(&self, vpn: VirtPageNum) -> Option<PageTableEntry> {
        self.page_table.translate(vpn)
    }

    ///Remove all `MapArea`
    pub fn recycle_data_pages(&mut self) {
        self.areas.clear();
    }

    /// shrink the area to new_end
    #[allow(unused)]
    pub fn shrink_to(&mut self, start: VirtAddr, new_end: VirtAddr) -> bool {
        if let Some(area) = self
            .areas
            .iter_mut()
            .find(|area| area.vpn_range.get_start() == start.floor())
        {
            area.shrink_to(&mut self.page_table, new_end.ceil());
            true
        } else {
            false
        }
    }

    /// append the area to new_end
    #[allow(unused)]
    pub fn append_to(&mut self, start: VirtAddr, new_end: VirtAddr) -> bool {
        if let Some(area) = self
            .areas
            .iter_mut()
            .find(|area| area.vpn_range.get_start() == start.floor())
        {
            area.append_to(&mut self.page_table, new_end.ceil());
            true
        } else {
            false
        }
    }

    /// map new heap area
    pub fn map_heap(&mut self, mut current_addr: VirtAddr, aim_addr: VirtAddr) -> isize {
        // log!("[map_heap] start_addr = {:#x}, end_addr = {:#x}", current_addr.0, aim_addr.0);
        loop {
            if current_addr.0 >= aim_addr.0 {
                break;
            }
            // We use BTreeMap to save FrameTracker which makes management quite easy
            // alloc a new FrameTracker
            let frame = frame_alloc().unwrap();
            let ppn = frame.ppn;
            let vpn: VirtPageNum = current_addr.floor();
            // log!("[map_heap] map vpn = {:#x}, ppn = {:#x}", vpn.0, ppn.0);
            self.page_table
                .map(vpn, ppn, PTEFlags::U | PTEFlags::R | PTEFlags::W);
            self.heap_area.insert(vpn, frame);
            current_addr = VirtAddr::from(current_addr.0 + PAGE_SIZE);
        }
        0
    }

    /// mmap
    pub fn mmap(
        &mut self,
        start_addr: usize,
        len: usize,
        offset: usize,
        context: Vec<u8>,
        flags: Flags,
    ) -> isize {
        let start_addr_align: usize;
        let end_addr_align: usize;
        if flags.contains(Flags::MAP_FIXED) && start_addr != 0 {
            // MAP_FIXED
            // alloc page one by one
            start_addr_align = ((start_addr) + PAGE_SIZE - 1) & (!(PAGE_SIZE - 1));
            end_addr_align = ((start_addr + len) + PAGE_SIZE - 1) & (!(PAGE_SIZE - 1));
        } else {
            start_addr_align = ((self.mmap_end.0) + PAGE_SIZE - 1) & (!(PAGE_SIZE - 1));
            end_addr_align = ((self.mmap_end.0 + len) + PAGE_SIZE - 1) & (!(PAGE_SIZE - 1));
        }
        self.mmap_end = (end_addr_align + PAGE_SIZE).into();
        let vpn_range = VPNRange::new(
            VirtAddr::from(start_addr_align).floor(),
            VirtAddr::from(end_addr_align).floor(),
        );
        if flags.contains(Flags::MAP_FIXED) && start_addr != 0 {
            // alloc memory
            for vpn in vpn_range {
                // let frame = frame_alloc().unwrap();
                match self.mmap_area.get(&vpn) {
                    Some(_) => {
                        debug!("[mmap] vpn = {:#x} has been mapped, skip", vpn.0);
                    }
                    None => {
                        let frame = frame_alloc().unwrap();
                        let ppn = frame.ppn;
                        self.mmap_area.insert(vpn, frame);
                        self.page_table.map(
                            vpn,
                            ppn,
                            PTEFlags::R | PTEFlags::W | PTEFlags::U | PTEFlags::X,
                        );
                    }
                }
            }
        } else {
            // alloc memory
            for vpn in vpn_range {
                let frame = frame_alloc().unwrap();
                let ppn = frame.ppn;
                self.mmap_area.insert(vpn, frame);
                self.page_table.map(
                    vpn,
                    ppn,
                    PTEFlags::R | PTEFlags::W | PTEFlags::U | PTEFlags::X,
                );
            }
        }
        debug!(
            "[mmap] context.len() = {}, offset = {}, len = {}",
            context.len(),
            offset,
            len
        );

        // MAP_ANONYMOUS标志代表不与文件关联的匿名映射
        if !flags.contains(Flags::MAP_ANONYMOUS) {
            let mut start: usize = offset;
            let mut current_vpn = vpn_range.get_start();
            loop {
                let src = &context[start..len.min(start + PAGE_SIZE)];
                let dst = &mut self
                    .page_table
                    .translate(current_vpn)
                    .unwrap()
                    .ppn()
                    .get_bytes_array()[..src.len()];
                dst.copy_from_slice(src);
                start += PAGE_SIZE;
                if start >= len {
                    break;
                }
                current_vpn.step();
            }
        }
        debug!(
            "[mmap] start_addr_align = {:#x}, end_addr_align = {:#x}",
            start_addr_align, end_addr_align
        );
        start_addr_align as isize
    }

    ///munmap
    pub fn munmap(&mut self, start_addr: usize, len: usize) -> isize {
        let start_addr_align = ((start_addr) + PAGE_SIZE - 1) & (!(PAGE_SIZE - 1));
        let end_addr_align = ((start_addr + len) + PAGE_SIZE - 1) & (!(PAGE_SIZE - 1));
        let vpn_range = VPNRange::new(
            VirtAddr::from(start_addr_align).floor(),
            VirtAddr::from(end_addr_align).floor(),
        );
        for vpn in vpn_range {
            self.mmap_area.remove(&vpn);
        }
        SUCCESS
    }
}

pub struct MapArea {
    pub vpn_range: VPNRange,
    pub data_frames: BTreeMap<VirtPageNum, FrameTracker>,
    pub map_type: MapType,
    pub map_perm: MapPermission,
}

impl MapArea {
    pub fn new(
        start_va: VirtAddr,
        end_va: VirtAddr,
        map_type: MapType,
        map_perm: MapPermission,
    ) -> Self {
        let start_vpn: VirtPageNum = start_va.floor();
        let end_vpn: VirtPageNum = end_va.ceil();
        Self {
            vpn_range: VPNRange::new(start_vpn, end_vpn),
            data_frames: BTreeMap::new(),
            map_type,
            map_perm,
        }
    }
    pub fn from_another(another: &Self) -> Self {
        Self {
            vpn_range: VPNRange::new(another.vpn_range.get_start(), another.vpn_range.get_end()),
            data_frames: BTreeMap::new(),
            map_type: another.map_type,
            map_perm: another.map_perm,
        }
    }
    pub fn map_one(&mut self, page_table: &mut PageTable, vpn: VirtPageNum) {
        let ppn: PhysPageNum;
        match self.map_type {
            MapType::Identical => {
                ppn = PhysPageNum(vpn.0);
            }
            MapType::Framed => {
                let frame = frame_alloc().unwrap();
                ppn = frame.ppn;
                self.data_frames.insert(vpn, frame);
            }
        }
        let pte_flags = PTEFlags::from_bits(self.map_perm.bits).unwrap();
        page_table.map(vpn, ppn, pte_flags);
    }
    pub fn unmap_one(&mut self, page_table: &mut PageTable, vpn: VirtPageNum) {
        if self.map_type == MapType::Framed {
            self.data_frames.remove(&vpn);
        }
        page_table.unmap(vpn);
    }
    pub fn map(&mut self, page_table: &mut PageTable) {
        for vpn in self.vpn_range {
            self.map_one(page_table, vpn);
        }
    }
    pub fn unmap(&mut self, page_table: &mut PageTable) {
        for vpn in self.vpn_range {
            self.unmap_one(page_table, vpn);
        }
    }
    #[allow(unused)]
    pub fn shrink_to(&mut self, page_table: &mut PageTable, new_end: VirtPageNum) {
        for vpn in VPNRange::new(new_end, self.vpn_range.get_end()) {
            self.unmap_one(page_table, vpn)
        }
        self.vpn_range = VPNRange::new(self.vpn_range.get_start(), new_end);
    }
    #[allow(unused)]
    pub fn append_to(&mut self, page_table: &mut PageTable, new_end: VirtPageNum) {
        for vpn in VPNRange::new(self.vpn_range.get_end(), new_end) {
            self.map_one(page_table, vpn)
        }
        self.vpn_range = VPNRange::new(self.vpn_range.get_start(), new_end);
    }
    /// data: start-aligned but maybe with shorter length
    /// assume that all frames were cleared before
    pub fn copy_data(&mut self, page_table: &mut PageTable, data: &[u8]) {
        assert_eq!(self.map_type, MapType::Framed);
        let mut start: usize = 0;
        let mut current_vpn = self.vpn_range.get_start();
        let len = data.len();
        loop {
            let src = &data[start..len.min(start + PAGE_SIZE)];
            let dst = &mut page_table
                .translate(current_vpn)
                .unwrap()
                .ppn()
                .get_bytes_array()[..src.len()];
            dst.copy_from_slice(src);
            start += PAGE_SIZE;
            if start >= len {
                break;
            }
            current_vpn.step();
        }
    }
    #[allow(unused)]
    /// check if area is confilct with given range
    pub fn is_conflict_with(&self, start_va: VirtAddr, end_va: VirtAddr) -> bool {
        let start_vpn = start_va.floor();
        let end_vpn = end_va.ceil();
        !(start_vpn >= self.vpn_range.get_end() || end_vpn <= self.vpn_range.get_start())
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum MapType {
    Identical,
    Framed,
}

bitflags! {
    /// map permission corresponding to that in pte: `R W X U`
    pub struct MapPermission: u8 {
        ///Readable
        const R = 1 << 1;
        ///Writable
        const W = 1 << 2;
        ///Excutable
        const X = 1 << 3;
        ///Accessible in U mode
        const U = 1 << 4;
    }
}

/// test map function in page table
#[allow(unused)]
pub fn remap_test() {
    let mut kernel_space = KERNEL_SPACE.exclusive_access();
    let mid_text: VirtAddr = ((stext as usize + etext as usize) / 2).into();
    let mid_rodata: VirtAddr = ((srodata as usize + erodata as usize) / 2).into();
    let mid_data: VirtAddr = ((sdata as usize + edata as usize) / 2).into();
    assert!(!kernel_space
        .page_table
        .translate(mid_text.floor())
        .unwrap()
        .writable(),);
    assert!(!kernel_space
        .page_table
        .translate(mid_rodata.floor())
        .unwrap()
        .writable(),);
    assert!(!kernel_space
        .page_table
        .translate(mid_data.floor())
        .unwrap()
        .executable(),);
    println!("remap_test passed!");
}
