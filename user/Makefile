TARGET := riscv64gc-unknown-none-elf
MODE := release
APP_DIR := src/bin
TARGET_DIR := target/$(TARGET)/$(MODE)
BUILD_DIR := build
OBJDUMP := rust-objdump --arch-name=riscv64
OBJCOPY := rust-objcopy --binary-architecture=riscv64
PY := python3

TEST_ELF_DIR := ../testcases

ifeq ($(MODE), release)
	MODE_ARG := --release
endif

APPS := $(wildcard $(APP_DIR)/*.rs)

TEST_CASES := $(filter-out %.sh %/ %txt,$(wildcard $(TEST_ELF_DIR)/*))

ELFS := $(patsubst $(APP_DIR)/%.rs, $(TARGET_DIR)/%, $(APPS))

TEST_CASES_ELFS := $(patsubst $(TEST_ELF_DIR)/%, $(TARGET_DIR)/%, $(TEST_CASES))

binary:
	@echo $(ELFS)
	@echo $(TEST_CASES_ELFS)
	cargo build $(MODE_ARG) ;
	@$(foreach test_elf, $(TEST_CASES), \
		cp $(test_elf) $(TARGET_DIR)/;)
	@$(foreach elf, $(ELFS), \
		$(OBJCOPY) $(elf) --strip-all -O binary $(patsubst $(TARGET_DIR)/%, $(TARGET_DIR)/%.bin, $(elf)); \
		cp $(elf) $(patsubst $(TARGET_DIR)/%, $(TARGET_DIR)/%.elf, $(elf));) 
	@$(foreach test_elf, $(TEST_CASES_ELFS), \
		$(OBJCOPY) $(test_elf) --strip-all -O binary $(patsubst $(TARGET_DIR)/%, $(TARGET_DIR)/%.bin, $(test_elf)); \
		cp $(test_elf) $(patsubst $(TARGET_DIR)/%, $(TARGET_DIR)/%.elf, $(test_elf));)

disasm:
	@$(foreach elf, $(ELFS), \
		$(OBJDUMP) $(elf) -S > $(patsubst $(TARGET_DIR)/%, $(TARGET_DIR)/%.asm, $(elf));)
	@$(foreach t, $(ELFS), cp $(t).asm $(BUILD_DIR)/asm/;)

pre:
	@mkdir -p $(BUILD_DIR)/bin/
	@mkdir -p $(BUILD_DIR)/elf/
	@mkdir -p $(BUILD_DIR)/app/
	@mkdir -p $(BUILD_DIR)/asm/
	@$(foreach t, $(APPS), cp $(t) $(BUILD_DIR)/app/;)

build: clean pre binary
	@$(foreach t, $(ELFS), cp $(t).bin $(BUILD_DIR)/bin/;)
	@$(foreach t, $(ELFS), cp $(t).elf $(BUILD_DIR)/elf/;)
	@$(foreach t, $(TEST_CASES_ELFS), cp $(t).bin $(BUILD_DIR)/bin/;)
	@$(foreach t, $(TEST_CASES_ELFS), cp $(t).elf $(BUILD_DIR)/elf/;)

clean:
	@cargo clean
	@rm -rf $(BUILD_DIR)

all: build

.PHONY: elf binary build clean all