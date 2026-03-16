#!/bin/bash
set -e

WORKSPACE_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BUILD_DIR="$WORKSPACE_ROOT/build"
KERNEL_ELF="$BUILD_DIR/kernel.elf"
UIMAGE="$BUILD_DIR/uImage"
LOAD_ADDR="0x40008000"

if [[ ! -f "$KERNEL_ELF" ]]; then
    echo "[!] $KERNEL_ELF not found. Run build.sh first."
    exit 1
fi

arm-none-eabi-objcopy -O binary "$KERNEL_ELF" "$BUILD_DIR/kernel.bin"

mkimage -A arm -O linux -T kernel -C none \
    -a $LOAD_ADDR -e $LOAD_ADDR \
    -n "pi-os" \
    -d "$BUILD_DIR/kernel.bin" \
    "$UIMAGE"

# Build boot script
mkimage -A arm -O linux -T script -C none -n "boot" \
    -d "$WORKSPACE_ROOT/bootloader/uboot/boot.cmd" \
    "$BUILD_DIR/boot.scr"

# Pack onto disk
DISK="$BUILD_DIR/uboot_disk.img"
dd if=/dev/zero of="$DISK" bs=1M count=64 status=none

# Create MBR partition table with one FAT16 partition
parted -s "$DISK" mklabel msdos
parted -s "$DISK" mkpart primary fat16 1MiB 100%

# Format just the partition (offset 1MiB = 2048 sectors * 512 = 1048576 bytes)
PART_OFFSET=$((2048 * 512))
PART_SIZE=$((63 * 1024 * 1024))  # ~63MiB
dd if=/dev/zero of="$BUILD_DIR/part.img" bs=1M count=63 status=none
mkfs.fat -F 16 "$BUILD_DIR/part.img" > /dev/null
mcopy -i "$BUILD_DIR/part.img" "$UIMAGE"             ::uImage
mcopy -i "$BUILD_DIR/part.img" "$BUILD_DIR/boot.scr" ::boot.scr

# Inject partition image into disk at 1MiB offset
dd if="$BUILD_DIR/part.img" of="$DISK" bs=512 seek=2048 conv=notrunc status=none
rm "$BUILD_DIR/part.img"

echo "[+] uImage: $UIMAGE"
echo "[+] U-Boot disk: $DISK"