DOCKER_NAME ?= rcore-tutorial-v3
MAKEFLAGS += --no-print-directory

.PHONY: docker build_docker all clean env

all: fmt
	@echo "Building user..."
	@cd user && make build
	@echo "Building os..."
	@cd os && make build
	@echo "Copying sbi-qemu..."
	@cp bootloader/rustsbi-qemu.bin sbi-qemu
	@echo "Copying kernel-qemu..."
	@cp os/target/riscv64gc-unknown-none-elf/release/os.bin kernel-qemu

env:
	@echo "Setting up cargo environment..."
	@cd os && make env

docker:
	docker run --rm -it -v ${PWD}:/mnt -w /mnt ${DOCKER_NAME} bash

build_docker: 
	docker build -t ${DOCKER_NAME} .

fmt:
	@echo "Formatting..."
	@cd os; cargo fmt;

sdcard-riscv.img:
	@echo "Downloading sdcard-riscv.img.gz..."
	@wget https://github.com/oscomp/testsuits-for-oskernel/releases/download/2024-final-rv/sdcard-riscv.img.gz
	@echo "Extracting sdcard-riscv.img.gz..."
	@gzip -dk sdcard-riscv.img.gz

run: all sdcard-riscv.img
	@qemu-system-riscv64 \
		-machine virt \
		-kernel kernel-qemu \
		-m 128M \
		-nographic \
		-smp 2 \
		-bios sbi-qemu \
		-drive file=sdcard-riscv.img,if=none,format=raw,id=x0  \
		-device virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0 \
		-device virtio-net-device,netdev=net \
		-netdev user,id=net

clean: 
	@echo "Cleaning os..."
	@cd os && make clean
	@echo "Cleaning user..."
	@cd user && make clean
	@echo "Removing kernel-qemu..."
	@rm -f sbi-qemu kernel-qemu
	@echo "Removing sdcard-riscv.img..."
	@rm -f os/sdcard-riscv.img

debug: all sdcard-riscv.img
	@tmux new-session -d \
			"qemu-system-riscv64 -machine virt -m 128M -nographic -smp 2 -bios sbi-qemu -drive file=sdcard-riscv.img,if=none,format=raw,id=x0 -device virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0 -s -S" && \
			tmux split-window -h "riscv64-unknown-elf-gdb -ex 'file kernel-qemu' -ex 'set arch riscv:rv64' -ex 'target remote localhost:1234'" && \
			tmux -2 attach-session -d