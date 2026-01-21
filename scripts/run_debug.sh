#!/bin/bash
set -e

BUILD_DIR="build"
KERNEL_ELF="$BUILD_DIR/kernel_debug.elf"
IMG_FILE="$BUILD_DIR/rootfs.img"

if [ ! -f "$KERNEL_ELF" ]; then
    echo "[!] Debug kernel not found. Build it first."
    exit 1
fi

if [ ! -f "$IMG_FILE" ]; then
    echo "[!] FAT32 image not found. Build it first."
    exit 1
fi

echo "[*] Starting QEMU in debug mode (waiting for GDB on port 1234)..."

qemu-system-arm \
    -M raspi0 \
    -kernel "$KERNEL_ELF" \
    -drive file=build/rootfs.img,format=raw,if=sd \
    -nographic \
    -S \
    -s