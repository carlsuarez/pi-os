#!/bin/bash
set -e

BUILD_DIR="build"
KERNEL_ELF="$BUILD_DIR/kernel.elf"
IMG_FILE="$BUILD_DIR/rootfs.img"

if [ ! -f "$KERNEL_ELF" ]; then
    echo "[!] Release kernel not found. Build it first."
    exit 1
fi

if [ ! -f "$IMG_FILE" ]; then
    echo "[!] FAT32 image not found. Build it first."
    exit 1
fi

echo "[*] Starting QEMU (release)..."

qemu-system-arm \
    -M raspi0 \
    -kernel "$KERNEL_ELF" \
    -drive file=build/rootfs.img,format=raw,if=sd \
    -nographic \
