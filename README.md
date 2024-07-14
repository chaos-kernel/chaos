# ChaOS

![USTB](./docs/image/USTB.jpg)

## 参赛文档

系统介绍文档在 [docs](./docs/) 文件夹。初赛文档是 [这个](./docs/初赛文档.md)。

开发日志发布在队员的个人博客上：

- 陈宽宽：[【开发日志】chaos开发日志](https://sazikk.github.io/posts/%E5%BC%80%E5%8F%91%E6%97%A5%E5%BF%97-chaos%E5%BC%80%E5%8F%91%E6%97%A5%E5%BF%97/)
- 王诺贤：[https://note.bosswnx.xyz/](https://note.bosswnx.xyz/)

[GitLab 仓库](https://gitlab.eduxiji.net/T202410008992750/oskernel2024-chaos) 与 [GitHub 仓库](https://github.com/bosswnx/chaos/) 保持同步。
 
## 参赛信息

- 参赛队名： chaos
- 参赛学校：北京科技大学
- 队伍成员：
  - 王诺贤：[bosswnx@outlook.com](mailto:bosswnx@outlook.com)
  - 陈宽宽：[ck_look@outlook.com](mailto:ck_look@outlook.com)
  - 乐一然：[ryan.yiran.le@gmail.com](mailto:ryan.yiran.le@gmail.com)

## 使用说明

*注意：以下所有指令皆在项目根目录下执行*

若是第一次编译 chaos，需要运行 `make env` 来配置 cargo 编译环境。

运行 `make all` 来编译项目，可在根目录获得操作系统以及 SBI 的二进制文件。

运行 `make run` 来编译项目并且启动 QEMU 运行内核。

## 开发环境配置

推荐开发环境为 x86_64 架构 Ubuntu 22.04 LTS，其他平台的开发稳定性不作保证。

推荐使用 vscode + rust-analyzer 插件进行开发。

首先安装 Rust：

```bash
curl https://sh.rustup.rs -sSf | sh
```

安装过程中全程选择默认选项即可。

可以将 Rust 的包管理器 cargo 的源替换成中科大源。打开或新建 `~/.cargo/config.toml` 文件，添加以下内容：

```toml
[source.crates-io]
replace-with = 'ustc'

[source.ustc]
registry = "sparse+https://mirrors.ustc.edu.cn/crates.io-index/"
```

然后编译安装 QEMU 7.0.0：

```bash
# 切换到 home 目录
cd ~
# 安装编译所需的依赖包
sudo apt install autoconf automake autotools-dev curl libmpc-dev libmpfr-dev libgmp-dev \
              gawk build-essential bison flex texinfo gperf libtool patchutils bc \
              zlib1g-dev libexpat-dev pkg-config  libglib2.0-dev libpixman-1-dev git tmux python3 ninja-build
# 下载源码包
# 如果下载速度过慢可以使用我们提供的百度网盘链接：https://pan.baidu.com/s/1z-iWIPjxjxbdFS2Qf-NKxQ
# 提取码 8woe
wget https://download.qemu.org/qemu-7.0.0.tar.xz
# 解压
tar xvJf qemu-7.0.0.tar.xz
# 编译安装并配置 RISC-V 支持
cd qemu-7.0.0
./configure --target-list=riscv64-softmmu,riscv64-linux-user
make -j$(nproc)
```

将编译得到的以下三个目录添加到 `PATH` 中：

```bash
export PATH="$HOME/qemu-7.0.0/build/:$PATH"
export PATH="$HOME/qemu-7.0.0/build/riscv64-softmmu:$PATH"
export PATH="$HOME/qemu-7.0.0/build/riscv64-linux-user:$PATH"
```

也可将其移至其他地方存放，只要能放在 `PATH` 中即可。

重启终端，确认 QEMU 版本：

```bash
qemu-system-riscv64 --version
qemu-riscv64 --version
```

如果正确识别指令并输出版本为 `7.0.0`，即说明 QEMU 安装正确。

## rust-analyzer 插件 ``can't find crate for `test` `` 报错解决

在根目录下新建文件 `.vscode/settings.json`，添加以下内容：

```json
{
    // Prevent "can't find crate for `test`" error on no_std
    // Ref: https://github.com/rust-lang/vscode-rust/issues/729
    "rust-analyzer.cargo.target": "riscv64gc-unknown-none-elf",
    "rust-analyzer.checkOnSave.allTargets": false,
    // "rust-analyzer.cargo.features": [
    //     "board_qemu"
    // ]
}
```

重新加载 rust-analyzer 即可。

## 更改 chaos 初始进程

chaos 通过将初始进程的 elf 文件链接到内核镜像中，从而在系统启动之后运行。链接脚本位于 `os/src/link_initproc.S`。

脚本默认将 `user/target/riscv64gc-unknown-none-elf/release/initproc` 链接到内核中作为初始进程。通过修改 `.incbin` 来链接不同的应用程序作为初始进程。链接的文件必须要是 elf 格式文件。