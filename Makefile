DOCKER_NAME ?= rcore-tutorial-v3
.PHONY: docker build_docker all clean
	
docker:
	docker run --rm -it -v ${PWD}:/mnt -w /mnt ${DOCKER_NAME} bash

build_docker: 
	docker build -t ${DOCKER_NAME} .

fmt:
	cd easy-fs; cargo fmt; cd ../easy-fs-fuse cargo fmt; cd ../os ; cargo fmt; cd ../user; cargo fmt; cd ..

all:
	@cd os && make build
	@cp bootloader/rustsbi-qemu.bin sbi-qemu
	@cp os/target/riscv64gc-unknown-none-elf/release/os.bin kernel-qemu

sdcard-riscv.img:
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
	@echo "Cleaning..."
	@cd os && make clean
	@rm -f sbi-qemu kernel-qemu
	@rm -f sdcard-riscv.img

