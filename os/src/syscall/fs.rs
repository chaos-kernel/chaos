use alloc::sync::Arc;
use core::{borrow::Borrow, cmp::min, mem::size_of, ops::Add, ptr};

use riscv::register::sstatus;

use crate::{
    fs::{
        defs::OpenFlags,
        file::{cast_file_to_inode, cast_inode_to_file},
        inode::Stat,
        open_file,
        pipe::make_pipe,
        Iovec,
        ROOT_INODE,
    },
    mm::{translated_byte_buffer, translated_refmut, translated_str},
    syscall::{
        errno::{EACCES, EBADF, EBUSY, ENOENT, ENOTDIR, ENOTTY},
        Dirent,
    },
    task::{current_task, current_user_token},
    utils::string::c_ptr_to_string,
};

pub const AT_FDCWD: i32 = -100;

/// write syscall
pub fn sys_write(fd: usize, buf: *const u8, len: usize) -> isize {
    trace!(
        "kernel:pid[{}] sys_write fd:{}",
        current_task().unwrap().pid.0,
        fd,
    );
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

        let buf = unsafe {
            sstatus::set_sum();
            let buf = core::slice::from_raw_parts(buf, len);
            sstatus::clear_sum();
            buf
        };
        file.write(buf) as isize
    } else {
        EBADF
    }
}
/// read syscall
pub fn sys_read(fd: usize, buf: *mut u8, len: usize) -> isize {
    trace!(
        "kernel:pid[{}] sys_read fd:{}",
        current_task().unwrap().pid.0,
        fd,
    );
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
        unsafe {
            sstatus::set_sum();
            let buf = core::slice::from_raw_parts_mut(buf, len);
            let ret = file.read(buf) as isize;
            sstatus::clear_sum();
            ret
        }
    } else {
        EBADF
    }
}
/// openat sys
pub fn sys_open(path: *const u8, flags: i32) -> isize {
    trace!("kernel:pid[{}] sys_open", current_task().unwrap().pid.0);
    let task = current_task().unwrap();
    let token = current_user_token();
    debug!("kernel: sys_open path: {:?}", path);
    let path = translated_str(token, path);
    let curdir = task
        .inner_exclusive_access(file!(), line!())
        .work_dir
        .clone();
    if let Some(dentry) = open_file(
        curdir.inode(),
        path.as_str(),
        OpenFlags::from_bits(flags).unwrap(),
    ) {
        let inode = dentry.inode();
        let mut inner = task.inner_exclusive_access(file!(), line!());
        let fd = inner.alloc_fd();
        let file = cast_inode_to_file(inode).unwrap();
        inner.fd_table[fd] = Some(file);
        fd as isize
    } else {
        ENOENT
    }
}
pub fn sys_openat(dirfd: i32, path: *const u8, flags: i32) -> isize {
    trace!("kernel:pid[{}] sys_openat", current_task().unwrap().pid.0);
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
    let mut inner = task.inner_exclusive_access(file!(), line!());
    let (pipe_read, pipe_write) = make_pipe();
    let read_fd = inner.alloc_fd();
    inner.fd_table[read_fd] = Some(pipe_read);
    let write_fd = inner.alloc_fd();
    inner.fd_table[write_fd] = Some(pipe_write);
    unsafe {
        sstatus::set_sum();
        *pipe = read_fd as u32;
        *pipe.add(1) = write_fd as u32;
        sstatus::clear_sum();
    }
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
        inner.fd_table[new_fd] = inner.fd_table[fd].clone();
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
        unsafe {
            sstatus::set_sum();
            *st = stat;
            sstatus::clear_sum();
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
    let curdir = current_task()
        .unwrap()
        .inner_exclusive_access(file!(), line!())
        .work_dir
        .clone();
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
    let curdir = current_task()
        .unwrap()
        .inner_exclusive_access(file!(), line!())
        .work_dir
        .clone();
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
    let path = c_ptr_to_string(path);
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
    trace!("kernel:pid[{}] sys_umount2", current_task().unwrap().pid.0);
    0
}

pub fn sys_mount(
    _source: *const u8, _target: *const u8, _fs: *const u8, _flags: u32, _data: *const u8,
) -> isize {
    trace!("kernel:pid[{}] sys_mount", current_task().unwrap().pid.0);
    0
}

pub fn sys_ioctl(fd: usize, request: usize, arg: usize) -> isize {
    trace!("kernel:pid[{}] sys_ioctl", current_task().unwrap().pid.0);
    // TODO:
    ENOTTY
    // let task = current_task().unwrap();
    // let mut inner = task.inner_exclusive_access(file!(), line!());
    // if fd >= inner.fd_table.len() {
    //     return EBADF;
    // }
    // if inner.fd_table[fd].is_none() {
    //     return EBADF;
    // }
    // if let Some(file) = &inner.fd_table[fd] {
    //     let file = file.clone();
    //     file.ioctl(request, arg1, arg2, arg3, arg4)
    // } else {
    //     EBADF
    // }
}

pub fn sys_writev(fd: usize, iov: usize, iovcnt: usize) -> isize {
    trace!("kernel:pid[{}] sys_writev", current_task().unwrap().pid.0);
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access(file!(), line!());
    if fd >= inner.fd_table.len() {
        return EBADF;
    }
    if inner.fd_table[fd].is_none() {
        return EBADF;
    }
    if let Some(file) = &inner.fd_table[fd] {
        if !file.writable() {
            return EACCES;
        }
        let file = file.clone();
        let mut total_len = 0;
        let iovec_size: usize = core::mem::size_of::<Iovec>();
        for i in 0..iovcnt {
            unsafe {
                sstatus::set_sum();
            }
            let current = iov.add(iovec_size * i);
            let iov_base = unsafe { (*(current as *const Iovec)).iov_base };
            let iov_len = unsafe { (*(current as *const Iovec)).iov_len };
            let buf = unsafe { core::slice::from_raw_parts(iov_base as *const u8, iov_len) };
            total_len += file.write(buf);
            unsafe {
                sstatus::clear_sum();
            }
        }

        total_len as isize
    } else {
        EBADF
    }
}

const F_DUPFD: i32 = 0;
const F_DUPFD_CLOEXEC: i32 = 1030;
const F_GETFD: i32 = 1;
const F_SETFD: i32 = 2;
const F_GETFL: i32 = 3;
const F_SETFL: i32 = 4;

pub fn sys_fcntl(fd: usize, cmd: i32, arg: usize) -> isize {
    trace!("kernel:pid[{}] sys_fcntl", current_task().unwrap().pid.0);
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access(file!(), line!());
    if fd >= inner.fd_table.len() {
        return EBADF;
    }
    if inner.fd_table[fd].is_none() {
        return EBADF;
    }
    match cmd {
        F_DUPFD => {
            let new_fd = inner.alloc_fd();
            inner.fd_table[new_fd] = inner.fd_table[fd].clone();
            debug!(
                "kernel:pid[{}] sys_fcntl F_DUPFD fd:{} => new_fd:{}",
                task.pid.0, fd, new_fd
            );
            new_fd as isize
        }
        F_DUPFD_CLOEXEC => {
            let new_fd = inner.alloc_fd();
            inner.fd_table[new_fd] = inner.fd_table[fd].clone();
            // TODO: fix this
            // inner.fd_table[new_fd].as_mut().unwrap().flags |= OpenFlags::CLOEXEC;
            debug!(
                "kernel:pid[{}] sys_fcntl F_DUPFD fd:{} => new_fd:{}",
                task.pid.0, fd, new_fd
            );
            new_fd as isize
        }
        F_SETFD => {
            // TODO: fix this
            // let flags = OpenFlags::from_bits(arg as u32).ok_or(SyscallErr::EINVAL)?;
            // inner.fd_table[fd].as_mut().unwrap().flags = flags;
            0
        }
        F_SETFL => {
            // TODO: fix this
            // let flags = OpenFlags::from_bits(arg as u32).ok_or(SyscallErr::EINVAL)?;
            // inner.fd_table[fd].as_mut().unwrap().flags = flags;
            0
        }
        F_GETFD | F_GETFL => {
            todo!()
        }
        _ => {
            todo!()
        }
    }
}

pub fn sys_sendfile(out_fd: usize, in_fd: usize, offset: usize, count: usize) -> isize {
    trace!("kernel:pid[{}] sys_sendfile", current_task().unwrap().pid.0);
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access(file!(), line!());
    if out_fd >= inner.fd_table.len() || in_fd >= inner.fd_table.len() {
        return EBADF;
    }
    if inner.fd_table[out_fd].is_none() || inner.fd_table[in_fd].is_none() {
        return EBADF;
    }
    let out_file = inner.fd_table[out_fd].as_ref().unwrap().clone();
    let in_file = inner.fd_table[in_fd].as_ref().unwrap().clone();
    drop(inner);
    let buf = out_file.read_all();
    in_file.write(&buf) as isize
}
