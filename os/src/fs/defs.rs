use bitflags::bitflags;

bitflags! {
    pub struct OpenFlags: i32 {
        const O_RDONLY    = 0o0;
        const O_WRONLY    = 0o1;
        const O_RDWR      = 0o2;
        const O_CREAT     = 0o100;
        const O_EXCL      = 0o200;
        const O_NOCTTY    = 0o400;
        const O_TRUNC     = 0o1000;
        const O_APPEND    = 0o2000;
        const O_NONBLOCK  = 0o4000;
        const O_DSYNC     = 0o10000;
        const O_SYNC      = 0o4010000;
        const O_RSYNC     = 0o4010000;
        const O_DIRECTORY = 0o200000;
        const O_NOFOLLOW  = 0o400000;
        const O_CLOEXEC   = 0o2000000;

        // 一些常用的组合
        const O_ASYNC     = 0o20000;
        const O_DIRECT    = 0o40000;
        const O_LARGEFILE = 0o100000;
        const O_NOATIME   = 0o1000000;
        const O_PATH      = 0o10000000;
        const O_TMPFILE   = 0o20200000;
    }
}

bitflags! {
    pub struct FileMode: u32 {
        const S_IRWXU = 0o700;  // 用户（所有者）读、写、执行权限
        const S_IRUSR = 0o400;  // 用户读权限
        const S_IWUSR = 0o200;  // 用户写权限
        const S_IXUSR = 0o100;  // 用户执行权限

        const S_IRWXG = 0o070;  // 组读、写、执行权限
        const S_IRGRP = 0o040;  // 组读权限
        const S_IWGRP = 0o020;  // 组写权限
        const S_IXGRP = 0o010;  // 组执行权限

        const S_IRWXO = 0o007;  // 其他用户读、写、执行权限
        const S_IROTH = 0o004;  // 其他用户读权限
        const S_IWOTH = 0o002;  // 其他用户写权限
        const S_IXOTH = 0o001;  // 其他用户执行权限

        const S_ISUID = 0o4000; // 设置用户ID
        const S_ISGID = 0o2000; // 设置组ID
        const S_ISVTX = 0o1000; // 粘滞位
    }
}
