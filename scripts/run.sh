#!/bin/bash
set -e
WORKSPACE_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BUILD_DIR="$WORKSPACE_ROOT/build"
UBOOT="/usr/lib/u-boot/qemu_arm/u-boot.bin"
TARGET="qemu"
DEBUG=0

for arg in "$@"; do
    [[ "$arg" == "--target=uboot" ]] && TARGET="uboot"
    [[ "$arg" == "--target=qemu"  ]] && TARGET="qemu"
    [[ "$arg" == "--target=x86"   ]] && TARGET="x86"
    [[ "$arg" == "--debug"        ]] && DEBUG=1
done

DEBUG_FLAGS=""
if [[ $DEBUG -eq 1 ]]; then
    DEBUG_FLAGS="-s -S"
fi

case "$TARGET" in
# ============================================================
# QEMU ARM (Raspberry Pi Zero emulation)
# ============================================================
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
        -display none \
        -serial stdio \
        $DEBUG_FLAGS
    ;;

# ============================================================
# U-Boot ARM
# ============================================================
uboot)
    if [[ ! -f "$UBOOT" ]]; then
        echo "[!] U-Boot not found. Run: sudo apt install u-boot-qemu"
        exit 1
    fi
    if [[ ! -f "$BUILD_DIR/uboot_disk.img" ]]; then
        echo "[!] uboot_disk.img not found. Run: mkimg.sh --target=uboot"
        exit 1
    fi
    echo "[*] Running U-Boot on QEMU ARM virt..."
    qemu-system-arm \
        -M virt \
        -cpu arm1176 \
        -m 256M \
        -bios "$UBOOT" \
        -drive if=virtio,format=raw,file="$BUILD_DIR/uboot_disk.img" \
        -display none \
        -serial stdio \
        $DEBUG_FLAGS
    ;;

# ============================================================
# x86 (GRUB boot image)
# ============================================================
x86)
    KERNEL_ELF="$BUILD_DIR/kernel-x86.elf"
    [[ $DEBUG -eq 1 ]] && KERNEL_ELF="$BUILD_DIR/kernel-x86_debug.elf"
    if [[ ! -f "$KERNEL_ELF" ]]; then
        echo "[!] $KERNEL_ELF not found. Run: build.sh --target=x86"
        exit 1
    fi
    if [[ ! -f "pi-os-x86.img" ]]; then
        echo "[!] pi-os-x86.img not found."
        exit 1
    fi
    echo "[*] Running on GRUB x86 image..."
    qemu-system-i386 \
        -drive format=raw,file=pi-os-x86.img \
        -m 128M \
        -boot c \
        -serial stdio \
        $DEBUG_FLAGS
    ;;

*)
    echo "[!] Usage: run.sh --target=[qemu|uboot|x86] [--debug]"
    exit 1
    ;;
esac