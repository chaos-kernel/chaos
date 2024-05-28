# ChaOS

## 提交注意事项

对于 RISCV 平台：

本次大赛的初赛阶段评测使用 QEMU 虚拟环境，提交的项目根目录中必须包含一个 Makefile 文件，评测时会自动在您的项目中执行 `make all` 命令，您应该在 `Makefile` 中的 `all` 目标对操作系统进行编译，并生成 ELF 格式的 `sbi-qemu` 和 `kernel-qemu` 两个文件，即与 xv6-k210 运行 qemu 时的方式一致。

如果你的系统使用默认 SBI，则不需要生成 `sbi-qemu` 文件，运行QEMU时会自动设置 `-bios` 参数为 `default`。

同时 QEMU 启动时还会使用 `-drive file` 参数挂载 SD 卡镜像，SD 卡镜像为 FAT32 文件系统，没有分区表。在 SD 卡镜像的根目录里包含若干个预先编译好的 ELF 可执行文件（以下简称测试点），您的操作系统在启动后需要主动扫描 SD 卡，并依次运行其中每一个测试点，将其运行结果输出到串口上，评测系统会根据您操作系统的串口输出内容进行评分。您可以根据操作系统的完成度自由选择跳过其中若干个测试点，未被运行的测试点将不计分。测试点的执行顺序与评分无关，多个测试点只能串行运行，不可同时运行多个测试点。具体测试点的数量、内容以及编译方式将在赛题公布时同步发布。

当您的操作系统执行完所有测试点后，应该主动调用关机命令，评测机会在检测到 QEMU 进程退出后进行打分。



运行 Riscv QEMU 的完整命令为：

```bash
qemu-system-riscv64 -machine virt -kernel kernel-qemu -m 128M -nographic -smp 2 -bios sbi-qemu -drive file=sdcard.img,if=none,format=raw,id=x0  -device virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0 -device virtio-net-device,netdev=net -netdev user,id=net -initrd initrd.img
```
在运行 QEMU 的命令中，`-initrd initrd.img` 为可选项。如果你的 Makefile 生成了 `initrd.img`，则会在运行命令中添加此参数，否则运行命令中不包含 `-intird initrd.img` 参数。

## TODO

- [ ] FAT32 文件系统
  - [x] read
  - [x] write
  - [x] create
  - [x] unlink

- [ ] syscalls
  - [x] SYS_getcwd 17
  - [ ] SYS_pipe2 59
  - [x] SYS_dup 23
  - [x] SYS_dup3 24
  - [x] SYS_chdir 49
  - [x] SYS_openat 56
  - [x] SYS_close 57
  - [x] SYS_getdents64 61
  - [x] SYS_read 63
  - [x] SYS_write 64
  - [ ] SYS_linkat 37
  - [x] SYS_unlinkat 35
  - [x] SYS_mkdirat 34
  - [ ] SYS_umount2 39
  - [ ] SYS_mount 40
  - [x] SYS_fstat 80
  - [x] SYS_clone 220
  - [x] SYS_execve 221
  - [x] SYS_wait4 260
  - [x] SYS_exit 93
  - [x] SYS_getppid 173
  - [x] SYS_getpid 172
  - [x] SYS_brk 214
  - [ ] SYS_munmap 215
  - [ ] SYS_mmap 222
  - [x] SYS_times 153
  - [x] SYS_uname 160
  - [x] SYS_sched_yield 124
  - [x] SYS_gettimeofday 169
  - [x] SYS_nanosleep 101

