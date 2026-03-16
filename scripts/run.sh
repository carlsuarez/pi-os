#!/bin/bash
set -e
WORKSPACE_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BUILD_DIR="$WORKSPACE_ROOT/build"
UBOOT="/usr/lib/u-boot/qemu_arm/u-boot.bin"

TARGET="qemu"   # default
DEBUG=0

for arg in "$@"; do
    [[ "$arg" == "--target=uboot" ]] && TARGET="uboot"
    [[ "$arg" == "--target=qemu"  ]] && TARGET="qemu"
    [[ "$arg" == "--debug"        ]] && DEBUG=1
done

QEMU_COMMON="-nographic -serial mon:stdio"
[[ $DEBUG -eq 1 ]] && QEMU_COMMON+=" -s -S"

case "$TARGET" in
    # ------------------------------------------------------------------
    # QEMU: raspi0 machine, loads ELF directly — closest to Pi Zero W HW
    # ------------------------------------------------------------------
    qemu)
        KERNEL_ELF="$BUILD_DIR/kernel-qemu.elf"
        [[ $DEBUG -eq 1 ]] && KERNEL_ELF="$BUILD_DIR/kernel-qemu_debug.elf"

        if [[ ! -f "$KERNEL_ELF" ]]; then
            echo "[!] $KERNEL_ELF not found. Run: build.sh --target=qemu"
            exit 1
        fi

        echo "[*] Running on raspi0 (QEMU)..."
        qemu-system-arm \
            -M raspi0 \
            -kernel "$KERNEL_ELF" \
            -drive file="$BUILD_DIR/rootfs.img",format=raw,if=sd \
            $QEMU_COMMON
        ;;

    # ------------------------------------------------------------------
    # UBOOT: virt machine with U-Boot BIOS, boots via uImage+boot.scr
    # ------------------------------------------------------------------
    uboot)
        if [[ ! -f "$UBOOT" ]]; then
            echo "[!] U-Boot not found. Run: sudo apt install u-boot-qemu"
            exit 1
        fi
        if [[ ! -f "$BUILD_DIR/uboot_disk.img" ]]; then
            echo "[!] uboot_disk.img not found. Run: mkimg.sh --target=uboot"
            exit 1
        fi

        echo "[*] Running on virt/arm1176 with U-Boot..."
        qemu-system-arm \
            -M virt \
            -cpu arm1176 \
            -m 256M \
            -bios "$UBOOT" \
            -drive if=virtio,format=raw,file="$BUILD_DIR/uboot_disk.img" \
            $QEMU_COMMON
        ;;
esac