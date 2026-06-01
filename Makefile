# pi-os build system
#
# Usage:
#   make                       # build kernel for default target (x86)
#   make TARGET=arm build      # build ARM kernel for U-Boot
#   make TARGET=pi run         # build, image, and run Pi Zero in QEMU
#   make TARGET=x86 image      # create bootable GRUB ISO
#   make TARGET=x86 run        # build + GRUB ISO + run in QEMU
#   make DEBUG=1 run           # build with debug info, halt for gdb (-s -S)
#   make clean
#
# Targets:
#   arm   ARM kernel + U-Boot bootloader (qemu-system-arm -M virt)
#   pi    ARM kernel for Raspberry Pi Zero (qemu-system-arm -M raspi0)
#   x86   x86 kernel + GRUB bootloader   (qemu-system-i386)
#
# Required tools:
#   arm:  arm-none-eabi-gcc, mkimage, parted, mkfs.fat, mcopy,
#         u-boot-qemu (/usr/lib/u-boot/qemu_arm/u-boot.bin)
#   pi:   arm-none-eabi-gcc, mkfs.fat, mcopy
#   x86:  gcc (32-bit headers), grub-mkrescue, xorriso

TARGET   ?= x86
DEBUG    ?= 0

WORKSPACE := $(CURDIR)
BUILD     := $(WORKSPACE)/build

# ---------------------------------------------------------------------------
# Per-target configuration
# ---------------------------------------------------------------------------

ifeq ($(TARGET),arm)
  ARCH         := arm
  RUST_TARGET  := $(WORKSPACE)/targets/armv6-none.json
  RUST_OUT_BASE:= $(WORKSPACE)/target/armv6-none
  ASM_CC       := arm-none-eabi-gcc
  ASM_FLAGS    := -mcpu=arm1176jzf-s -mfloat-abi=soft -ffreestanding -nostdlib
  LINKER       := $(WORKSPACE)/kernel/linker-arm.ld
  LOAD_ADDR    := 0x40008000
else ifeq ($(TARGET),pi)
  ARCH         := arm
  RUST_TARGET  := $(WORKSPACE)/targets/armv6-none.json
  RUST_OUT_BASE:= $(WORKSPACE)/target/armv6-none
  ASM_CC       := arm-none-eabi-gcc
  ASM_FLAGS    := -mcpu=arm1176jzf-s -mfloat-abi=soft -ffreestanding -nostdlib
  LINKER       := $(WORKSPACE)/kernel/linker-arm.ld
  LOAD_ADDR    := 0x8000
else ifeq ($(TARGET),x86)
  ARCH         := x86
  RUST_TARGET  := $(WORKSPACE)/targets/x86-none.json
  RUST_OUT_BASE:= $(WORKSPACE)/target/x86-none
  ASM_CC       := gcc
  ASM_FLAGS    := -m32 -ffreestanding -nostdlib
  LINKER       := $(WORKSPACE)/kernel/linker-x86.ld
else
  $(error Unknown TARGET '$(TARGET)'. Use arm, pi, or x86)
endif

ifeq ($(DEBUG),1)
  CARGO_PROFILE :=
  PROFILE_DIR   := debug
  KERNEL_ELF    := $(BUILD)/kernel-$(TARGET)_debug.elf
  RUSTC_FLAGS   := -C debuginfo=2
  QEMU_DEBUG    := -s -S
else
  CARGO_PROFILE := --release
  PROFILE_DIR   := release
  KERNEL_ELF    := $(BUILD)/kernel-$(TARGET).elf
  RUSTC_FLAGS   :=
  QEMU_DEBUG    :=
endif

ASM_SRCS := $(shell find $(WORKSPACE)/kernel/src/arch/$(ARCH) -name '*.S' 2>/dev/null)
ASM_OBJS := $(patsubst %.S,$(BUILD)/%.o,$(notdir $(ASM_SRCS)))

LINK_ARGS := -C link-arg=-T$(LINKER) -C link-arg=--gc-sections \
             $(if $(filter arm,$(ARCH)),-C link-arg=--defsym=KERNEL_LOAD_ADDR=$(LOAD_ADDR)) \
             $(addprefix -C link-arg=,$(ASM_OBJS))

# ---------------------------------------------------------------------------
# Top-level goals
# ---------------------------------------------------------------------------

.PHONY: all build run image clean
.DEFAULT_GOAL := build

all: build

build: $(KERNEL_ELF)

run: run-$(TARGET)

image: image-$(TARGET)

clean:
	rm -rf $(BUILD) $(WORKSPACE)/target

# ---------------------------------------------------------------------------
# Kernel build
# ---------------------------------------------------------------------------

$(BUILD):
	@mkdir -p $(BUILD)

# Each .S file → .o in $(BUILD). vpath lets us write a single pattern rule
# regardless of where under kernel/src/arch/$(ARCH) the .S lives.
vpath %.S $(sort $(dir $(ASM_SRCS)))

$(BUILD)/%.o: %.S | $(BUILD)
	$(ASM_CC) -c $(ASM_FLAGS) $(if $(filter 1,$(DEBUG)),-g) $< -o $@

$(KERNEL_ELF): $(ASM_OBJS) | $(BUILD)
	@echo "[*] Building $(TARGET) kernel ($(PROFILE_DIR))..."
	cargo +nightly rustc $(CARGO_PROFILE) \
	    -Z build-std=core,alloc,compiler_builtins \
	    -Z build-std-features=compiler-builtins-mem \
	    -Z json-target-spec \
	    -p kernel --target $(RUST_TARGET) -- \
	    $(RUSTC_FLAGS) $(LINK_ARGS)
	cp $(RUST_OUT_BASE)/$(PROFILE_DIR)/kernel $@
	@echo "[+] $@"

# ---------------------------------------------------------------------------
# image-arm: U-Boot disk for qemu-system-arm -M virt
# ---------------------------------------------------------------------------

UBOOT_BIN := /usr/lib/u-boot/qemu_arm/u-boot.bin

.PHONY: image-arm
image-arm: $(BUILD)/uboot_disk.img

$(BUILD)/uboot_disk.img: $(KERNEL_ELF) bootloader/uboot/boot.cmd | $(BUILD)
	@echo "[*] Creating U-Boot disk image..."
	arm-none-eabi-objcopy -O binary $(KERNEL_ELF) $(BUILD)/kernel-arm.bin
	mkimage -A arm -O linux -T kernel -C none \
	    -a $(LOAD_ADDR) -e $(LOAD_ADDR) -n "pi-os" \
	    -d $(BUILD)/kernel-arm.bin $(BUILD)/uImage
	mkimage -A arm -O linux -T script -C none -n "boot" \
	    -d bootloader/uboot/boot.cmd $(BUILD)/boot.scr
	dd if=/dev/zero of=$@ bs=1M count=64 status=none
	parted -s $@ mklabel msdos
	parted -s $@ mkpart primary fat16 1MiB 100%
	dd if=/dev/zero of=$(BUILD)/part.img bs=1M count=63 status=none
	mkfs.fat -F 16 $(BUILD)/part.img >/dev/null
	mcopy -i $(BUILD)/part.img $(BUILD)/uImage   ::uImage
	mcopy -i $(BUILD)/part.img $(BUILD)/boot.scr ::boot.scr
	dd if=$(BUILD)/part.img of=$@ bs=512 seek=2048 conv=notrunc status=none
	@rm -f $(BUILD)/part.img
	@echo "[+] $@"

.PHONY: run-arm
run-arm: $(BUILD)/uboot_disk.img
	@test -f $(UBOOT_BIN) || { echo "[!] $(UBOOT_BIN) missing. apt install u-boot-qemu"; exit 1; }
	qemu-system-arm \
	    -M virt -cpu arm1176 -m 256M \
	    -bios $(UBOOT_BIN) \
	    -drive if=virtio,format=raw,file=$(BUILD)/uboot_disk.img \
	    -display none -serial stdio $(QEMU_DEBUG)

# ---------------------------------------------------------------------------
# image-pi: FAT32 rootfs for qemu-system-arm -M raspi0
# (kernel is loaded directly via -kernel, no firmware needed in QEMU)
# ---------------------------------------------------------------------------

.PHONY: image-pi
image-pi: $(BUILD)/rootfs.img

$(BUILD)/rootfs.img: $(KERNEL_ELF) | $(BUILD)
	@echo "[*] Creating Pi rootfs..."
	dd if=/dev/zero of=$@ bs=1M count=64 status=none
	mkfs.fat -F32 -n ROOTFS $@ >/dev/null
	echo "Hello from QEMU FAT32!" | mcopy -i $@ - ::test.txt
	@echo "[+] $@"

.PHONY: run-pi
run-pi: $(KERNEL_ELF) $(BUILD)/rootfs.img
	qemu-system-arm \
	    -M raspi0 \
	    -kernel $(KERNEL_ELF) \
	    -drive file=$(BUILD)/rootfs.img,format=raw,if=sd \
	    -display none -serial stdio $(QEMU_DEBUG)

# ---------------------------------------------------------------------------
# image-x86: bootable GRUB ISO via grub-mkrescue (no sudo, no losetup)
# ---------------------------------------------------------------------------

X86_ISO     := $(BUILD)/pi-os-x86.iso
X86_ISODIR  := $(BUILD)/iso

.PHONY: image-x86
image-x86: $(X86_ISO)

$(X86_ISO): $(KERNEL_ELF) bootloader/grub/grub.cfg | $(BUILD)
	@echo "[*] Creating GRUB ISO..."
	@rm -rf $(X86_ISODIR)
	@mkdir -p $(X86_ISODIR)/boot/grub
	cp $(KERNEL_ELF) $(X86_ISODIR)/boot/kernel.bin
	cp bootloader/grub/grub.cfg $(X86_ISODIR)/boot/grub/grub.cfg
	grub-mkrescue -o $@ $(X86_ISODIR)
	@echo "[+] $@"

.PHONY: run-x86
run-x86: $(X86_ISO)
	qemu-system-i386 \
	    -drive format=raw,file=$(X86_ISO) \
	    -m 128M -boot d -serial stdio $(QEMU_DEBUG)
