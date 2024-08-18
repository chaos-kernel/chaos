PLATFORM := qemu
ARCH := riscv64
MODE := release
OUTPUT := target/$(ARCH)gc-unknown-none-elf/$(MODE)
TARGET := riscv64gc-unknown-none-elf
TFTP_PATH := ../../../tftpboot/

build:
	@cargo build  --$(MODE) --target $(TARGET)
	@rust-objcopy --strip-all $(OUTPUT)/testos -O binary $(OUTPUT)/testos.bin
	@cp $(OUTPUT)/testos.bin $(TFTP_PATH)

clean:
	@cargo clean
