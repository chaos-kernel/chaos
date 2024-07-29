use alloc::sync::Arc;
use core::{borrow::Borrow, cmp::min, mem::size_of, ptr};

use crate::{
    fs::{
        defs::OpenFlags,
        file::{cast_file_to_inode, cast_inode_to_file},
        inode::Stat,
        open_file,
        pipe::make_pipe,
        ROOT_INODE,
    },
    mm::{translated_byte_buffer, translated_refmut, translated_str, UserBuffer},
    syscall::{
        errno::{EACCES, EBADF, EBUSY, ENOENT, ENOTDIR},
        Dirent,
    },
    task::{current_process, current_task, current_user_token},
};

pub const AT_FDCWD: i32 = -100;

/// write syscall
pub fn sys_write(fd: usize, buf: *const u8, len: usize) -> isize {
    trace!(
        "kernel:pid[{}] sys_write fd:{}",
        current_task().unwrap().pid.0,
        fd,
    );
    let token = current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access(file!(), line!());
    if fd >= inner.fd_table.len() {
        return EBADF;
    }
    if let Some(file) = &inner.fd_table[fd] {
        if !file.writable() {
            return EACCES;
        }
        let file = file.clone();
        // release current task TCB manually to avoid multi-borrow
        drop(inner);
        file.write(UserBuffer::new(translated_byte_buffer(token, buf, len))) as isize
    } else {
        EBADF
    }
}
/// read syscall
pub fn sys_read(fd: usize, buf: *const u8, len: usize) -> isize {
    trace!(
        "kernel:pid[{}] sys_read fd:{}",
        current_task().unwrap().pid.0,
        fd,
    );
    let token = current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access(file!(), line!());
    if fd >= inner.fd_table.len() {
        return EBADF;
    }
    if let Some(file) = &inner.fd_table[fd] {
        let file = file.clone();
        if !file.readable() {
            return EACCES;
        }
        // release current task TCB manually to avoid multi-borrow
        drop(inner);
        trace!("kernel: sys_read .. file.read");
        file.read(UserBuffer::new(translated_byte_buffer(token, buf, len))) as isize
    } else {
        EBADF
    }
}
/// openat sys
pub fn sys_open(path: *const u8, flags: i32) -> isize {
    trace!(
        "kernel:pid[{}] sys_open",
        current_task().unwrap().process.upgrade().unwrap().getpid()
    );
    let process = current_process();
    let token = current_user_token();
    debug!("kernel: sys_open path: {:?}", path);
    let path = translated_str(token, path);
    let curdir = process.inner_exclusive_access().work_dir.clone();
    if let Some(dentry) = open_file(
        curdir.inode(),
        path.as_str(),
        OpenFlags::from_bits(flags).unwrap(),
    ) {
        let inode = dentry.inode();
        let mut inner = process.inner_exclusive_access();
        let fd = inner.alloc_fd();
        let file = cast_inode_to_file(inode).unwrap();
        inner.fd_table[fd] = Some(file);
        fd as isize
    } else {
        ENOENT
    }
}
pub fn sys_openat(dirfd: i32, path: *const u8, flags: i32) -> isize {
    trace!(
        "kernel:pid[{}] sys_openat",
        current_task().unwrap().process.upgrade().unwrap().getpid()
    );
    if dirfd == AT_FDCWD {
        return sys_open(path, flags);
    }
    let dirfd = dirfd as usize;
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access(file!(), line!());
    if dirfd >= inner.fd_table.len() {
        return EBADF;
    }
    if inner.fd_table[dirfd].is_none() {
        return EBADF;
    }
    let dir = inner.fd_table[dirfd].as_ref().unwrap().clone();
    // TODO: 好像无法判断是否是目录
    // if !dir.is_dir() {
    //     return -1;
    // }
    let inode = cast_file_to_inode(dir).unwrap();
    let token = inner.memory_set.token();
    let path = translated_str(token, path);
    if let Some(dentry) = open_file(inode, path.as_str(), OpenFlags::from_bits(flags).unwrap()) {
        let fd = inner.alloc_fd();
        let inode = dentry.inode();
        let file = cast_inode_to_file(inode).unwrap();
        inner.fd_table[fd] = Some(file);
        fd as isize
    } else {
        ENOENT
    }
}
/// close syscall
pub fn sys_close(fd: usize) -> isize {
    trace!(
        "kernel:pid[{}] sys_close fd:{}",
        current_task().unwrap().pid.0,
        fd,
    );
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access(file!(), line!());
    if fd >= inner.fd_table.len() {
        return EBADF;
    }
    if inner.fd_table[fd].is_none() {
        return EBADF;
    }
    inner.fd_table[fd].take();
    0
}
/// pipe syscall
pub fn sys_pipe(pipe: *mut u32) -> isize {
    trace!("kernel:pid[{}] sys_pipe", current_task().unwrap().pid.0);
    let task = current_task().unwrap();
    let token = current_user_token();
    let mut inner = task.inner_exclusive_access(file!(), line!());
    let (pipe_read, pipe_write) = make_pipe();
    let read_fd = inner.alloc_fd();
    inner.fd_table[read_fd] = Some(pipe_read);
    let write_fd = inner.alloc_fd();
    inner.fd_table[write_fd] = Some(pipe_write);
    *translated_refmut(token, pipe) = read_fd as u32;
    *translated_refmut(token, unsafe { pipe.add(1) }) = write_fd as u32;
    0
}
/// dup syscall
pub fn sys_dup(fd: usize) -> isize {
    trace!("kernel:pid[{}] sys_dup", current_task().unwrap().pid.0);
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access(file!(), line!());
    if fd >= inner.fd_table.len() {
        return EBADF;
    }
    if inner.fd_table[fd].is_none() {
        return EBADF;
    }
    let new_fd = inner.alloc_fd();
    inner.fd_table[new_fd] = Some(Arc::clone(inner.fd_table[fd].as_ref().unwrap()));
    new_fd as isize
}

/// dup3 syscall
pub fn sys_dup3(fd: usize, new_fd: usize) -> isize {
    trace!("kernel:pid[{}] sys_dup3", current_task().unwrap().pid.0);
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access(file!(), line!());
    if fd >= inner.fd_table.len() {
        return EBADF;
    }
    if inner.fd_table[fd].is_none() {
        return EBADF;
    }
    while inner.fd_table.len() <= new_fd {
        inner.fd_table.push(None);
    }
    if inner.fd_table[new_fd].is_some() {
        return EBUSY;
    }
    inner.fd_table[new_fd] = Some(Arc::clone(inner.fd_table[fd].as_ref().unwrap()));
    new_fd as isize
}

/// YOUR JOB: Implement fstat.
pub fn sys_fstat(fd: usize, st: *mut Stat) -> isize {
    trace!("kernel:pid[{}] sys_fstat", current_task().unwrap().pid.0);
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access(file!(), line!());
    if fd >= inner.fd_table.len() {
        return EBADF;
    }
    if inner.fd_table[fd].is_none() {
        return EBADF;
    }
    if let Some(file) = &inner.fd_table[fd] {
        let file = file.clone();
        let stat = file.fstat();
        if stat.is_none() {
            return EBADF;
        }
        let stat = stat.unwrap();
        let mut v =
            translated_byte_buffer(inner.get_user_token(), st as *const u8, size_of::<Stat>());
        unsafe {
            let mut p = stat.borrow() as *const Stat as *const u8;
            for slice in v.iter_mut() {
                let len = slice.len();
                ptr::copy_nonoverlapping(p, slice.as_mut_ptr(), len);
                p = p.add(len);
            }
        }
    }
    0
}

/// YOUR JOB: Implement linkat.
pub fn sys_linkat(old_name: *const u8, new_name: *const u8) -> isize {
    trace!("kernel:pid[{}] sys_linkat", current_task().unwrap().pid.0);
    let token = current_user_token();
    let old_name = translated_str(token, old_name);
    let new_name = translated_str(token, new_name);
    let curdir = current_process().inner_exclusive_access().work_dir.clone();
    let target = curdir.inode().lookup(old_name.as_str()).unwrap();
    if curdir.inode().link(&new_name, target) {
        0
    } else {
        ENOENT
    }
}

/// YOUR JOB: Implement unlinkat.
pub fn sys_unlinkat(name: *const u8) -> isize {
    trace!("kernel:pid[{}] sys_unlinkat", current_task().unwrap().pid.0);
    let token = current_user_token();
    let name = translated_str(token, name);
    let curdir = current_process().inner_exclusive_access().work_dir.clone();
    if curdir.inode().unlink(&name) {
        0
    } else {
        ENOENT
    }
}

pub fn sys_getcwd(buf: *mut u8, len: usize) -> isize {
    trace!("kernel:pid[{}] sys_getcwd", current_task().unwrap().pid.0);
    let token = current_user_token();
    if let path = current_task()
        .unwrap()
        .inner_exclusive_access(file!(), line!())
        .work_dir
        .clone()
        .name()
    {
        let len = core::cmp::min(len, path.len());
        let mut v = translated_byte_buffer(token, buf, len);
        unsafe {
            let mut p = path.as_bytes().as_ptr();
            for slice in v.iter_mut() {
                let len = slice.len();
                ptr::copy_nonoverlapping(p, slice.as_mut_ptr(), len);
                p = p.add(len);
            }
        }
        buf as isize
    } else {
        ENOENT
    }
}

pub fn sys_chdir(path: *const u8) -> isize {
    trace!("kernel:pid[{}] sys_chdir", current_task().unwrap().pid.0);
    let token = current_user_token();
    let path = translated_str(token, path);
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access(file!(), line!());
    let dir = inner.work_dir.clone();
    let inode = dir.inode();
    let dir = open_file(inode, &path, OpenFlags::O_RDWR | OpenFlags::O_DIRECTORY);
    inner.work_dir = dir.unwrap();
    0
}

pub fn sys_mkdirat64(dirfd: i32, path: *const u8, _mode: u32) -> isize {
    trace!("kernel:pid[{}] sys_mkdirat", current_task().unwrap().pid.0);
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access(file!(), line!());
    let inode;
    if dirfd == AT_FDCWD {
        inode = ROOT_INODE.clone();
    } else {
        let dirfd = dirfd as usize;
        if dirfd >= inner.fd_table.len() {
            return EBADF;
        }
        if inner.fd_table[dirfd].is_none() {
            return EBADF;
        }
        let dir = inner.fd_table[dirfd].as_ref().unwrap().clone();
        if !dir.is_dir() {
            return ENOTDIR;
        }
        inode = cast_file_to_inode(dir).unwrap();
    }
    let token = inner.memory_set.token();
    let path = translated_str(token, path);
    if let Some(_) = open_file(inode.clone(), &path, OpenFlags::O_RDONLY) {
        return -1;
    }
    if let Some(dentry) = open_file(
        inode.clone(),
        &path,
        OpenFlags::O_DIRECTORY | OpenFlags::O_CREAT,
    ) {
        let fd = inner.alloc_fd();
        let inode = dentry.inode();
        let file = cast_inode_to_file(inode).unwrap();
        inner.fd_table[fd] = Some(file);
        fd as isize
    } else {
        EACCES //TODO: to be confirmed
    }
}

pub fn sys_getdents64(dirfd: i32, buf: *mut u8, len: usize) -> isize {
    trace!(
        "kernel:pid[{}] sys_getdents64",
        current_task().unwrap().pid.0
    );
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access(file!(), line!());
    let inode;
    if dirfd == AT_FDCWD {
        inode = ROOT_INODE.clone();
    } else {
        let dirfd = dirfd as usize;
        if dirfd >= inner.fd_table.len() {
            return EBADF;
        }
        if inner.fd_table[dirfd].is_none() {
            return EBADF;
        }
        let dir = inner.fd_table[dirfd].as_ref().unwrap().clone();
        if !dir.is_dir() {
            return ENOTDIR;
        }
        inode = cast_file_to_inode(dir).unwrap();
    }
    let token = inner.memory_set.token();
    let mut v = translated_byte_buffer(token, buf, len);
    let mut read_size = 0usize;
    let mut offset_in_slice = 0usize;
    let mut slice_index = 0usize;
    let mut is_end = true;
    for name in inode.ls() {
        let dirent_len = 19 + name.len() + 1;
        if read_size + dirent_len > len {
            is_end = false;
            break;
        }
        // TODO: 这里 vec 的长度不同会导致内核 LoadPageFault，先这样处理
        let mut mbuf = [0u8; 35];
        let mut p = mbuf.as_mut() as *mut [u8] as *mut u8;
        let dirent = p as *mut Dirent;
        unsafe {
            *dirent = Dirent::new(read_size + dirent_len, dirent_len as u16, &name);
        }
        let mut copy_len = 0;
        while copy_len < dirent_len {
            let copy_size = min(
                dirent_len - copy_len,
                v[slice_index].len() - offset_in_slice,
            );
            unsafe {
                ptr::copy_nonoverlapping(
                    p,
                    v[slice_index][offset_in_slice..].as_mut_ptr(),
                    copy_size,
                );
                p = p.add(copy_size);
            }
            read_size += copy_size;
            offset_in_slice += copy_size;
            copy_len += copy_size;
            if offset_in_slice == v[slice_index].len() {
                offset_in_slice = 0;
                slice_index += 1;
                if slice_index == v.len() {
                    break;
                }
            }
        }
    }
    if is_end {
        0
    } else {
        read_size as isize
    }
}

pub fn sys_umount2(_target: *const u8, _flags: i32) -> isize {
    trace!(
        "kernel:pid[{}] sys_umount2",
        current_task().unwrap().process.upgrade().unwrap().getpid()
    );
    0
}

pub fn sys_mount(
    _source: *const u8, _target: *const u8, _fs: *const u8, _flags: u32, _data: *const u8,
) -> isize {
    trace!("kernel:pid[{}] sys_mount", current_task().unwrap().pid.0);
    0
}
