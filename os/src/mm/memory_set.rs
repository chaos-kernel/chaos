//! Address Space [`MemorySet`] management of Process

use alloc::{
    collections::BTreeMap,
    string::{String, ToString},
    sync::Arc,
    vec,
    vec::*,
};
use core::{
    arch::asm,
    fmt::{Display, Formatter},
    ptr,
};

use lazy_static::*;
use riscv::register::{satp, sstatus};

use super::{
    config::*,
    frame_alloc,
    translated_refmut,
    FrameTracker,
    PTEFlags,
    PageTable,
    PageTableEntry,
    PhysPageNum,
    StepByOne,
    VPNRange,
    VirtAddr,
    VirtPageNum,
};
use crate::{
    boards::CLOCK_FREQ,
    config::{
        KERNEL_SPACE_OFFSET,
        MEMORY_END,
        MMAP_BASE,
        MMIO,
        PAGE_SIZE,
        PAGE_SIZE_BITS,
        USER_STACK_SIZE,
        USER_TRAMPOLINE,
    },
    fs::{defs::OpenFlags, ROOT_INODE},
    mm::config::AT_PHENT,
    sync::UPSafeCell,
    syscall::errno::SUCCESS,
    task::process::Flags,
    utils::string::c_ptr_to_string,
};

extern "C" {
    fn rust_main();
    fn _start();
    fn skernel();
    fn stext();
    fn etext();
    fn srodata();
    fn erodata();
    fn sdata();
    fn edata();
    fn sbss_with_stack();
    fn ebss();
    fn ekernel();
}

lazy_static! {
    /// The kernel's initial memory mapping(kernel address space)
    pub static ref KERNEL_SPACE: Arc<UPSafeCell<MemorySet>> =
        Arc::new(unsafe { UPSafeCell::new(MemorySet::new_kernel()) });
}

/// the kernel token
pub fn kernel_token() -> usize {
    KERNEL_SPACE.exclusive_access(file!(), line!()).token()
}

/// address space
pub struct MemorySet {
    /// page table
    pub page_table: PageTable,
    /// areas
    pub areas:      Vec<MapArea>,
    /// heap
    heap_area:      BTreeMap<VirtPageNum, FrameTracker>,
    // The memory area formed by mmap does not need to be modified
    // we can use MapArea in Vec to hold FramTracker
    // we set a fixed address as the start address for mmap_area
    // the virtual memorySet is big enough to use it that doesnt concern address conflicts
    pub mmap_area:  BTreeMap<VirtPageNum, FrameTracker>,
    // mmap_base will never change
    pub mmap_base:  VirtAddr,
    // always aligh to PAGE_SIZE
    pub mmap_end:   VirtAddr,
}

impl MemorySet {
    /// Create a new empty `MemorySet`.
    pub fn new_bare() -> Self {
        Self {
            page_table: PageTable::new(),
            areas:      Vec::new(),
            heap_area:  BTreeMap::new(),
            mmap_area:  BTreeMap::new(),
            mmap_base:  MMAP_BASE.into(),
            mmap_end:   MMAP_BASE.into(),
        }
    }
    /// Create a new `MemorySet` with the same page table as the kernel.
    pub fn new_process() -> Self {
        let page_table = PageTable::new_process();
        debug!("new process page table token: {:#x}", page_table.token());
        Self {
            page_table,
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
        &mut self, start_va: VirtAddr, end_va: VirtAddr, permission: MapPermission,
    ) {
        self.push(
            MapArea::new(start_va, end_va, MapType::Framed, permission),
            None,
        );
    }

    pub fn insert_framed_area_with_data(
        &mut self, start_va: VirtAddr, end_va: VirtAddr, permission: MapPermission, data: &[u8],
    ) {
        self.push(
            MapArea::new(start_va, end_va, MapType::Framed, permission),
            Some(data),
        );
    }
    /// check if exist areas conflict with given virtial address
    pub fn is_conflict_with_va(&self, start_va: VirtAddr, end_va: VirtAddr) -> bool {
        self.areas
            .iter()
            .any(|area| area.is_conflict_with(start_va, end_va))
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
            warn!("remove area with start_vpn: {:#x}", start_vpn.0);
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
            // warn!(
            //     "push map area, vpn: {:#x} - {:#x}, perm: {:?} start copying",
            //     map_area.vpn_range.get_start().0,
            //     map_area.vpn_range.get_end().0,
            //     map_area.map_perm
            // );
            if map_area.vpn_range.get_start().0 == 0x162 {
                debug!("{:x?}", data);
            }
            map_area.copy_data(&mut self.page_table, data, 0);
        }
        self.areas.push(map_area);
    }

    fn push_with_offset(&mut self, mut map_area: MapArea, offset: usize, data: Option<&[u8]>) {
        map_area.map(&mut self.page_table);
        if let Some(data) = data {
            map_area.copy_data(&mut self.page_table, data, offset)
        }
        self.areas.push(map_area);
    }
    /// Mention that trampoline is not collected by areas.
    // fn map_trampoline(&mut self) {
    //     self.page_table.map(
    //         VirtAddr::from(TRAMPOLINE).into(),
    //         PhysAddr::from(strampoline as usize).into(),
    //         PTEFlags::R | PTEFlags::X,
    //     );
    // }
    /// Without kernel stacks.
    #[no_mangle]
    pub fn new_kernel() -> Self {
        let mut memory_set = Self::new_bare();
        // map trampoline
        // memory_set.map_trampoline();
        // map kernel sections
        // let pc: usize;
        // unsafe {
        //     asm!(
        //         "auipc {}, 0",
        //         out(reg) pc,
        //     );
        // }
        // let satp = satp::read();
        // info!("Current satp: {:#x}", satp.bits());
        // info!("Current PC: {:#x}", pc);
        // let instruction: u32;
        // unsafe {
        //     instruction = core::ptr::read(pc as *const u32);
        // }
        // info!("Instruction at PC: {:#x}", instruction);
        info!("kernel entry: {:#x}", skernel as u64);
        info!("_start [{:#x}]", _start as usize);
        info!("rust_main [{:#x}]", rust_main as usize);
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
                    ((*pair).0 + (KERNEL_SPACE_OFFSET << PAGE_SIZE_BITS)).into(),
                    ((*pair).0 + (*pair).1 + (KERNEL_SPACE_OFFSET << PAGE_SIZE_BITS)).into(),
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
    pub fn from_elf(elf_data: &[u8]) -> (Self, usize, usize, usize, Vec<AuxHeader>) {
        let mut memory_set = Self::new_process();
        // map trampoline
        // memory_set.map_trampoline();
        // map program headers of elf, with U flag
        let elf = xmas_elf::ElfFile::new(elf_data).unwrap();
        let elf_header = elf.header;

        // auxv
        let mut auxv = vec![
            AuxHeader::new(AT_PHENT, elf_header.pt2.ph_entry_size() as usize),
            AuxHeader::new(AT_PHNUM, elf_header.pt2.ph_count() as usize),
            AuxHeader::new(AT_PAGESIZE, PAGE_SIZE as usize),
            AuxHeader::new(AT_FLAGS, 0),
            AuxHeader::new(AT_ENTRY, elf_header.pt2.entry_point() as usize),
            AuxHeader::new(AT_UID, 0),
            AuxHeader::new(AT_EUID, 0),
            AuxHeader::new(AT_GID, 0),
            AuxHeader::new(AT_EGID, 0),
            AuxHeader::new(AT_PLATFORM, 0),
            AuxHeader::new(AT_HWCAP, 0),
            AuxHeader::new(AT_CLKTCK, CLOCK_FREQ),
            AuxHeader::new(AT_SECURE, 0),
            AuxHeader::new(AT_NOELF, 0x112d),
        ];

        let magic = elf_header.pt1.magic;
        assert_eq!(magic, [0x7f, 0x45, 0x4c, 0x46], "invalid elf!");
        let ph_count = elf_header.pt2.ph_count();
        let mut max_end_vpn = VirtPageNum(0);
        let mut head_va: usize = 0;
        let mut interp_entry: Option<usize> = None;
        let mut interp_base: Option<usize> = None;

        for i in 0..ph_count {
            let ph = elf.program_header(i).unwrap();
            if ph.get_type().unwrap() == xmas_elf::program::Type::Load {
                let start_va: VirtAddr = (ph.virtual_addr() as usize).into();
                let page_offset = start_va.page_offset();
                let end_va: VirtAddr = ((ph.virtual_addr() + ph.mem_size()) as usize).into();
                let mut map_perm = MapPermission::U;
                let ph_flags = ph.flags();
                if head_va == 0 {
                    head_va = start_va.0;
                }
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

                if page_offset == 0 {
                    memory_set.push(
                        map_area,
                        Some(
                            &elf.input
                                [ph.offset() as usize..(ph.offset() + ph.file_size()) as usize],
                        ),
                    )
                } else {
                    memory_set.push_with_offset(
                        map_area,
                        page_offset,
                        Some(
                            &elf.input
                                [ph.offset() as usize..(ph.offset() + ph.file_size()) as usize],
                        ),
                    );
                }
            } else if ph.get_type().unwrap() == xmas_elf::program::Type::Interp {
                // // log!("[from_elf] .interp")
                // let mut path = String::from_utf8_lossy(
                //     &elf.input[ph.offset() as usize..(ph.offset() + ph.file_size() - 1) as usize],
                // )
                // .to_string();
                // match ROOT_INODE.open(&path, OpenFlags::O_RDONLY, false) {
                //     Ok(file) => {
                //         // let elf_data = file.read_all();
                //         let elf_data = file.map_to_kernel_space(SECOND_MMAP_BASE);
                //         let (entry, base) = memory_set.load_interp(elf_data);
                //         crate::mm::KERNEL_SPACE
                //             .exclusive_access()
                //             .remove_area_with_start_vpn(VirtAddr::from(SECOND_MMAP_BASE).floor());
                //         interp_entry = Some(entry);
                //         interp_base = Some(base);
                //     }
                //     Err(errno) => {
                //         panic!("[from_elf] Unkonwn interpreter path = {}", path);
                //     }
                // }
                todo!("interpreter not supported yet");
            }
        }

        if let Some(base) = interp_base {
            auxv.push(AuxHeader::new(AT_BASE, base));
        } else {
            auxv.push(AuxHeader::new(AT_BASE, 0));
        }

        auxv.push(AuxHeader::new(
            AT_PHDR,
            head_va + elf_header.pt2.ph_offset() as usize,
        ));

        // map user stack with U flags
        let max_end_va: VirtAddr = max_end_vpn.into();
        let mut user_stack_bottom: usize = max_end_va.into();
        user_stack_bottom += PAGE_SIZE;
        let user_stack_top: usize = user_stack_bottom + USER_STACK_SIZE;
        debug!("user_stack_bottom: {:#x}", user_stack_bottom);
        let user_heap_base: usize = user_stack_top + PAGE_SIZE;
        debug!("elf read completed!");
        (
            memory_set,
            user_heap_base,
            user_stack_top,
            elf.header.pt2.entry_point() as usize,
            auxv,
        )
    }
    /// Create a new address space by copy code&data from a exited process's address space.
    pub fn from_existed_user(user_space: &Self) -> Self {
        let mut memory_set = Self::new_process();
        // map trampoline
        // memory_set.map_trampoline();
        // copy mmap
        memory_set.mmap_end = user_space.mmap_end;
        // copy data sections/trap_context/user_stack
        for area in user_space.areas.iter() {
            // skip kernel space, cause it's already mapped
            if area.vpn_range.get_start().0 > KERNEL_SPACE_OFFSET {
                continue;
            }
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
        warn!("activate satp: {:#x}", satp);
        unsafe {
            satp::write(satp);
            asm!("sfence.vma");
        }
        let satp = satp::read();
        warn!("satp has been reset!! : {:#x}", satp.bits());
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
        &mut self, start_addr: usize, len: usize, offset: usize, context: Vec<u8>, flags: Flags,
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

    pub fn build_stack(
        &mut self, mut user_sp: usize, argv_vec: Vec<String>, mut envp_vec: Vec<String>,
        mut auxv_vec: Vec<AuxHeader>, token: usize,
    ) -> (usize, usize, usize, usize, usize) {
        // The structure of the user stack
        // STACK TOP (low address)
        //      argc
        //      *argv [] (with NULL as the end) 8 bytes each
        //      *envp [] (with NULL as the end) 8 bytes each
        //      auxv[] (with NULL as the end) 16 bytes each: now has PAGESZ(6)
        //      padding (16 bytes-align)
        //      rand bytes: Now set 0x00 ~ 0x0f (not support random) 16bytes
        //      String: platform "RISC-V64"
        //      Argument string(argv[])
        //      Environment String (envp[]): now has SHELL, PWD, LOGNAME, HOME, USER, PATH
        // STACK BOTTOM (high address)

        // let envp_vec = vec![
        //     String::from("SHELL=/bin/sh"),
        //     String::from("PWD=/"),
        //     String::from("USER=root"),
        //     String::from("MOTD_SHOWN=pam"),
        //     String::from("LANG=C.UTF-8"),
        //     String::from("INVOCATION_ID=e9500a871cf044d9886a157f53826684"),
        //     String::from("TERM=vt220"),
        //     String::from("SHLVL=2"),
        //     String::from("JOURNAL_STREAM=8:9265"),
        //     String::from("OLDPWD=/root"),
        //     String::from("_=busybox"),
        //     String::from("LOGNAME=root"),
        //     String::from("HOME=/"),
        //     String::from("LD_LIBRARY_PATH=/"),
        //     String::from("PATH=/:/bin/"),
        // ];

        trace!("building user stack sp:{:#x}", user_sp);

        // Enable kernel to visit user space
        unsafe {
            sstatus::set_sum(); //todo Use RAII
        }

        // envp_vec.push(String::from("PATH=/:/bin/"));

        let push_stack = |mmset: &mut MemorySet, parms: Vec<String>, user_sp: &mut usize| {
            //record parm ptr
            let mut ptr_vec: Vec<usize> = (0..=parms.len()).collect();

            //end with null
            ptr_vec[parms.len()] = 0;

            for index in 0..parms.len() {
                *user_sp -= parms[index].len() + 1;
                ptr_vec[index] = *user_sp;
                let mut p = *user_sp;

                //write chars to [user_sp,user_sp + len]
                for c in parms[index].as_bytes() {
                    *mmset.write_to_user_ptr(token, p as *mut u8) = *c;
                    // unsafe {
                    //     warn!(
                    //         "write char: {:?}",
                    //         *(VirtAddr::from(p).get_mut() as *mut char)
                    //     );
                    // }
                    p += 1;
                }
                *mmset.write_to_user_ptr(token, p as *mut u8) = 0;
                // unsafe {
                //     warn!(
                //         "write char: {:?}",
                //         *(VirtAddr::from(p).get_mut() as *mut char)
                //     );
                // }
            }
            ptr_vec
        };

        user_sp -= 2 * core::mem::size_of::<usize>();

        //========================= envp[] ==========================
        let envp = push_stack(self, envp_vec, &mut user_sp);
        // make sure aligned to 8b for k210
        user_sp -= user_sp % core::mem::size_of::<usize>();

        //========================= argv[] ==========================
        let argc = argv_vec.len();
        let argv = push_stack(self, argv_vec, &mut user_sp);
        // make the user_sp aligned to 8B for k210 platform
        user_sp -= user_sp % core::mem::size_of::<usize>();

        //========================= platform ==========================
        // let platform = "RISC-V64";
        // user_sp -= platform.len() + 1;
        // user_sp -= user_sp % core::mem::size_of::<usize>();
        // let mut p = user_sp;
        // for &c in platform.as_bytes() {
        //     *self.write_to_user_ptr(token, p as *mut u8) = c;
        //     unsafe {
        //         warn!(
        //             "write char: {:?}",
        //             *(VirtAddr::from(0x191_9810).get_mut() as *mut char)
        //         );
        //     }
        //     p += 1;
        // }
        // *self.write_to_user_ptr(token, p as *mut u8) = 0;
        // unsafe {
        //     warn!(
        //         "write char: {:?}",
        //         *(VirtAddr::from(0x191_9810).get_mut() as *mut char)
        //     );
        // }

        //========================= rand bytes ==========================
        user_sp -= 16;
        auxv_vec.push(AuxHeader::new(AT_RANDOM, user_sp));
        *self.write_to_user_ptr(token, user_sp as *mut usize) = 0x01020304050607;
        *self.write_to_user_ptr(
            token,
            (user_sp + core::mem::size_of::<usize>()) as *mut usize,
        ) = 0x08090a0b0c0d0e0f;

        //========================= padding ==========================
        user_sp -= user_sp % 16;

        //========================= auxv[] ==========================
        auxv_vec.push(AuxHeader::new(AT_EXECFN, argv[0]));
        auxv_vec.push(AuxHeader::new(AT_NULL, 0));
        user_sp -= auxv_vec.len() * core::mem::size_of::<AuxHeader>();
        let aux_base = user_sp;
        let mut addr = aux_base;
        for aux_header in auxv_vec {
            *self.write_to_user_ptr(token, addr as *mut usize) = aux_header._type;
            *self.write_to_user_ptr(token, (addr + core::mem::size_of::<usize>()) as *mut usize) =
                aux_header.value;
            addr += core::mem::size_of::<AuxHeader>();
        }

        //========================= *envp[] ==========================
        user_sp -= envp.len() * core::mem::size_of::<usize>();
        let envp_base = user_sp;
        let mut ustack_ptr = envp_base;
        for env_ptr in envp {
            *self.write_to_user_ptr(token, ustack_ptr as *mut usize) = env_ptr;
            // unsafe {
            //     warn!(
            //         "write char: {:?}",
            //         *(VirtAddr::from(ustack_ptr).get_mut() as *mut char)
            //     );
            // }
            ustack_ptr += core::mem::size_of::<usize>();
        }

        //========================= *argv[] ==========================
        user_sp -= argv.len() * core::mem::size_of::<usize>();
        let argv_base = user_sp;
        let mut ustack_ptr = argv_base;
        for argv_ptr in argv {
            *self.write_to_user_ptr(token, ustack_ptr as *mut usize) = argv_ptr;
            // unsafe {
            //     warn!(
            //         "write char: {:#x?}",
            //         *(VirtAddr::from(ustack_ptr).get_mut() as *mut usize)
            //     );
            // }
            ustack_ptr += core::mem::size_of::<usize>();
        }

        //========================= argc ==========================
        user_sp -= core::mem::size_of::<usize>();
        *self.write_to_user_ptr(token, user_sp as *mut usize) = argc;
        // unsafe {
        //     warn!(
        //         "write char: {:?}",
        //         *(VirtAddr::from(user_sp).get_mut() as *mut usize)
        //     );
        // }

        // Disable kernel to visit user space
        unsafe {
            sstatus::clear_sum(); //todo Use RAII
        }

        (user_sp, argc, argv_base, envp_base, aux_base)
    }

    /// 向另一个地址空间的地址写数据
    pub fn write_to_user_ptr<T>(&mut self, token: usize, ptr: *mut T) -> &'static mut T {
        let user_pagetable = PageTable::from_token(token);
        let va = VirtAddr::from(ptr as usize);
        let pa = user_pagetable.translate_va(va).unwrap();

        // debug!(
        //     "write_to_user_ptr: va: {:#x}, pa: {:#x}, token: {:#x}",
        //     va.0, pa.0, token
        // );
        self.page_table
            .map_allow_cover(va.floor(), pa.floor(), PTEFlags::R | PTEFlags::W);

        // debug!(
        //     "map pipe in user space token: {:#x}",
        //     self.page_table.token()
        // );

        let translated_ptr: &mut T = va.get_mut();
        translated_ptr
    }
}

pub struct MapArea {
    pub vpn_range:   VPNRange,
    pub data_frames: BTreeMap<VirtPageNum, FrameTracker>,
    pub map_type:    MapType,
    pub map_perm:    MapPermission,
}

impl MapArea {
    pub fn new(
        start_va: VirtAddr, end_va: VirtAddr, map_type: MapType, map_perm: MapPermission,
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
            vpn_range:   VPNRange::new(another.vpn_range.get_start(), another.vpn_range.get_end()),
            data_frames: BTreeMap::new(),
            map_type:    another.map_type,
            map_perm:    another.map_perm,
        }
    }
    pub fn map_one(&mut self, page_table: &mut PageTable, vpn: VirtPageNum) -> PhysPageNum {
        // debug!("map_one vpn: {:#x}", vpn.0);
        let ppn: PhysPageNum;
        match self.map_type {
            MapType::Identical => {
                ppn = PhysPageNum(vpn.0 - KERNEL_SPACE_OFFSET);
            }
            MapType::Framed => {
                let frame = frame_alloc().unwrap();
                ppn = frame.ppn;
                self.data_frames.insert(vpn, frame);
            }
        }
        let pte_flags = PTEFlags::from_bits(self.map_perm.bits).unwrap();
        page_table.map(vpn, ppn, pte_flags);
        // debug!(
        //     "map_one vpn: {:#x}, ppn: {:#x}, page_table: {:#x}",
        //     vpn.0,
        //     ppn.0,
        //     page_table.token()
        // );
        ppn
    }
    pub fn unmap_one(&mut self, page_table: &mut PageTable, vpn: VirtPageNum) {
        if self.map_type == MapType::Framed {
            self.data_frames.remove(&vpn);
        }
        page_table.unmap(vpn);
    }
    pub fn map(&mut self, page_table: &mut PageTable) {
        debug!(
            "map area, vpn: {:#x} - {:#x}, perm: {:?}, page_table: {:#x}",
            self.vpn_range.get_start().0,
            self.vpn_range.get_end().0,
            self.map_perm,
            page_table.token()
        );
        for vpn in self.vpn_range {
            self.map_one(page_table, vpn);
        }
    }
    pub fn unmap(&mut self, page_table: &mut PageTable) {
        warn!(
            "unmap area, vpn: {:#x} - {:#x}, perm: {:?}, page_table: {:#x}",
            self.vpn_range.get_start().0,
            self.vpn_range.get_end().0,
            self.map_perm,
            page_table.token()
        );
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
            self.map_one(page_table, vpn);
        }
        self.vpn_range = VPNRange::new(self.vpn_range.get_start(), new_end);
    }
    /// data: start-aligned but maybe with shorter length
    /// assume that all frames were cleared before
    pub fn copy_data(&mut self, page_table: &mut PageTable, data: &[u8], offset: usize) {
        assert_eq!(self.map_type, MapType::Framed);
        let mut start: usize = 0;
        let mut page_offset = offset;
        let mut current_vpn = self.vpn_range.get_start();
        let len = data.len();
        loop {
            let src = &data[start..len.min(start + PAGE_SIZE - page_offset)];
            let dst = &mut page_table
                .translate(current_vpn)
                .unwrap()
                .ppn()
                .get_bytes_array()[page_offset..(page_offset + src.len())];
            dst.copy_from_slice(src);
            start += PAGE_SIZE - page_offset;
            page_offset = 0;
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
    ///vpn - offset = ppn ;only for kernel space
    Identical,
    ///
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
    let mut kernel_space = KERNEL_SPACE.exclusive_access(file!(), line!());
    let mid_text: VirtAddr = (stext as usize + (etext as usize - stext as usize) / 2).into();
    let mid_rodata: VirtAddr =
        (srodata as usize + (erodata as usize - srodata as usize) / 2).into();
    let mid_data: VirtAddr = (sdata as usize + (edata as usize - sdata as usize) / 2).into();
    debug!(
        "mid text {:#x}, mid rodata {:#x}, mid data {:#x}",
        mid_text.0, mid_rodata.0, mid_data.0
    );
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
    info!("remap_test passed!");
}

pub struct AuxHeader {
    pub _type: usize,
    pub value: usize,
}

impl AuxHeader {
    #[inline]
    pub fn new(_type: usize, value: usize) -> Self {
        Self { _type, value }
    }
}

impl Display for AuxHeader {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "AuxHeader type: {} value: {}", self._type, self.value)
    }
}
