# chaos

![USTB](./docs/image/USTB.jpg)

## 参赛文档

系统介绍文档在 [docs](./docs/) 文件夹。初赛文档是[这个](./docs/初赛文档.md)。

开发日志发布在队员的个人博客上：

- 陈宽宽：[【开发日志】chaos开发日志](https://sazikk.github.io/posts/%E5%BC%80%E5%8F%91%E6%97%A5%E5%BF%97-chaos%E5%BC%80%E5%8F%91%E6%97%A5%E5%BF%97/)
- 王诺贤：[https://note.bosswnx.xyz/](https://note.bosswnx.xyz/)

## 提示

[GitLab 仓库](https://gitlab.eduxiji.net/T202410008992750/oskernel2024-chaos)与 [GitHub 仓库](https://github.com/bosswnx/chaos/)保持同步。

## 参赛信息

- 参赛队名： chaos
- 参赛学校：北京科技大学
- 队伍成员：
  - 王诺贤：[bosswnx@outlook.com](mailto:bosswnx@outlook.com)
  - 陈宽宽：[ck_look@outlook.com](mailto:ck_look@outlook.com)
  - 乐一然：[ryan.yiran.le@gmail.com](mailto:ryan.yiran.le@gmail.com)

## 使用说明

在根目录中运行 `make all`，即可在根目录获得操作系统以及 SBI 的二进制文件

运行 `make run` 编译内核程序并使用qemu启动。

### 更改 chaos 初始进程

chaos 通过将初始进程的 elf 文件链接到内核镜像中，从而在系统启动之后运行。链接脚本位于 `os/src/link_initproc.S`。

脚本默认将 `user/target/riscv64gc-unknown-none-elf/release/initproc` 链接到内核中作为初始进程。通过修改 `.incbin` 来链接不同的应用程序作为初始进程。链接的文件必须要是 elf 格式文件。