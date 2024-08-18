# Building
TARGET := riscv64gc-unknown-none-elf
MODE := release
KERNEL_ELF := target/$(TARGET)/$(MODE)/os
KERNEL_BIN := $(KERNEL_ELF).bin
DISASM_TMP := target/$(TARGET)/$(MODE)/asm
FS_IMG := ../sdcard-riscv.img
FS_IMG_PATH := ../testcases
APPS := ../user/src/bin/*
OFFLINE :=
MAKEFLAGS += --no-print-directory
# 定义一个变量来表示当前的 kernel 目标
KERNEL_TARGET := kernel

ifeq ($(MAKECMDGOALS),vf2)
    KERNEL_TARGET := kernel-vf2
endif

# BOARD
BOARD := qemu
SBI ?= rustsbi
BOOTLOADER := ../bootloader/$(SBI)-$(BOARD).bin

# Building mode argument
ifeq ($(MODE), release)
	MODE_ARG := --release
endif

# KERNEL ENTRY
KERNEL_ENTRY_PA_QEMU := 0x80200000
KERNEL_ENTRY_PA_VF2 := 0x40020000

# Binutils
OBJDUMP := rust-objdump --arch-name=riscv64
OBJCOPY := rust-objcopy --binary-architecture=riscv64

# Disassembly
DISASM ?= -x

build: config $(KERNEL_BIN)

config:
	@rm -rf .cargo
	@cp -r cargo .cargo

env:
ifeq ($(OFFLINE),)
	(rustup target list | grep "riscv64gc-unknown-none-elf (installed)") || rustup target add $(TARGET)
	cargo install cargo-binutils
	rustup component add rust-src
	rustup component add llvm-tools-preview
endif

# 修改 $(KERNEL_BIN) 的依赖
$(KERNEL_BIN): $(KERNEL_TARGET)
	@$(OBJCOPY) $(KERNEL_ELF) --strip-all -O binary $@

fs-img:
	@rm -f $(FS_IMG)
	@cp $(FS_IMG_PATH)/$(FS_IMG) .

# 默认的 kernel 目标
kernel:
	@echo Platform: $(BOARD)
	@cargo build $(MODE_ARG) \
	--offline \
	-q 
# 离线构建
# 安静模式

# 为 visionfive2 操作定义一个特殊的 kernel 目标
kernel-vf2:
	@echo Platform: VisionFive 2
	@cargo build $(MODE_ARG) \
	--offline \
	--features visionfive2 \
	--no-default-features \
	-q
# 离线构建
# 使用 visionfive2 特性
# 禁用默认特性，不写这个会莫名其妙启用默认特性
# 安静模式，忽略warning用

clean:
	@cargo clean

disasm: kernel
	@$(OBJDUMP) $(DISASM) $(KERNEL_ELF) | less

disasm-vim: kernel
	@$(OBJDUMP) $(DISASM) $(KERNEL_ELF) > $(DISASM_TMP)
	@vim $(DISASM_TMP)
	@rm $(DISASM_TMP)

run: build fs-img run-inner

run-inner:
	@qemu-system-riscv64 \
		-M 128m \
		-machine virt \
		-nographic \
		-kernel $(KERNEL_BIN) \
		-drive file=$(FS_IMG),if=none,format=raw,id=x0 \
        -device virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0

debug: build
	@tmux new-session -d \
		"qemu-system-riscv64 -machine virt -nographic -bios $(BOOTLOADER) -device loader,file=$(KERNEL_BIN),addr=$(KERNEL_ENTRY_PA_QEMU) -s -S" && \
		tmux split-window -h "riscv64-unknown-elf-gdb -ex 'file $(KERNEL_ELF)' -ex 'set arch riscv:rv64' -ex 'target remote localhost:1234'" && \
		tmux -2 attach-session -d


gdbserver: build
	@qemu-system-riscv64 -smp 2 -M 128m -machine virt -nographic  -kernel $(KERNEL_BIN) \
	-drive file=$(FS_IMG),if=none,format=raw,id=x0 \
        -device virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0 \
	-s -S

gdbclient:
	@riscv64-unknown-elf-gdb -ex 'file $(KERNEL_ELF)' -ex 'set arch riscv:rv64' -ex 'target remote localhost:1234'

vf2: build
	
	

.PHONY: build env kernel clean disasm disasm-vim run-inner fs-img gdbserver gdbclient config vf2