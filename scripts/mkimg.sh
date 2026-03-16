#!/bin/bash
set -e
WORKSPACE_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BUILD_DIR="$WORKSPACE_ROOT/build"

TARGET="qemu"   # default

for arg in "$@"; do
    [[ "$arg" == "--target=uboot" ]] && TARGET="uboot"
    [[ "$arg" == "--target=pi"    ]] && TARGET="pi"
    [[ "$arg" == "--target=qemu"  ]] && TARGET="qemu"
done

echo "[*] Target: $TARGET"

# ------------------------------------------------------------------
# Helper: download Pi firmware files if missing
# ------------------------------------------------------------------
fetch_pi_firmware() {
    local FIRMWARE_DIR="$1"
    local BASE_URL="https://github.com/raspberrypi/firmware/raw/master/boot"
    local FILES=(bootcode.bin start.elf fixup.dat)

    mkdir -p "$FIRMWARE_DIR"

    for f in "${FILES[@]}"; do
        if [[ ! -f "$FIRMWARE_DIR/$f" ]]; then
            echo "[*] Downloading $f..."
            curl -fsSL "$BASE_URL/$f" -o "$FIRMWARE_DIR/$f"
            echo "[+] $f -> $FIRMWARE_DIR/$f"
        else
            echo "[*] $f already present, skipping download."
        fi
    done
}

case "$TARGET" in
    # ------------------------------------------------------------------
    # QEMU: flat FAT32 image with a test file, loaded directly by QEMU
    # ------------------------------------------------------------------
    qemu)
        IMG_FILE="$BUILD_DIR/rootfs.img"
        KERNEL_ELF="$BUILD_DIR/kernel-qemu.elf"

        if [[ ! -f "$KERNEL_ELF" ]]; then
            echo "[!] $KERNEL_ELF not found. Run: build.sh --target=qemu"
            exit 1
        fi

        if [[ -f "$IMG_FILE" && "$IMG_FILE" -nt "$KERNEL_ELF" ]]; then
            echo "[*] rootfs.img up to date, skipping."
            exit 0
        fi

        echo "[*] Creating QEMU FAT32 rootfs..."
        dd if=/dev/zero of="$IMG_FILE" bs=1M count=64 status=none
        mkfs.fat -F32 -n ROOTFS "$IMG_FILE" > /dev/null
        echo "Hello from QEMU FAT32!" | mcopy -i "$IMG_FILE" - ::test.txt
        echo "[+] Image: $IMG_FILE"
        ;;

    # ------------------------------------------------------------------
    # UBOOT: MBR disk image with FAT16 partition containing uImage+boot.scr
    # Load address 0x40008000 matches linker-uboot.ld and boot.cmd
    # ------------------------------------------------------------------
    uboot)
        KERNEL_ELF="$BUILD_DIR/kernel-uboot.elf"
        UIMAGE="$BUILD_DIR/uImage"
        LOAD_ADDR="0x40008000"

        if [[ ! -f "$KERNEL_ELF" ]]; then
            echo "[!] $KERNEL_ELF not found. Run: build.sh --target=uboot"
            exit 1
        fi

        arm-none-eabi-objcopy -O binary "$KERNEL_ELF" "$BUILD_DIR/kernel-uboot.bin"

        mkimage -A arm -O linux -T kernel -C none \
            -a $LOAD_ADDR -e $LOAD_ADDR \
            -n "pi-os" \
            -d "$BUILD_DIR/kernel-uboot.bin" \
            "$UIMAGE"

        mkimage -A arm -O linux -T script -C none -n "boot" \
            -d "$WORKSPACE_ROOT/bootloader/uboot/boot.cmd" \
            "$BUILD_DIR/boot.scr"

        DISK="$BUILD_DIR/uboot_disk.img"
        dd if=/dev/zero of="$DISK" bs=1M count=64 status=none
        parted -s "$DISK" mklabel msdos
        parted -s "$DISK" mkpart primary fat16 1MiB 100%

        dd if=/dev/zero of="$BUILD_DIR/part.img" bs=1M count=63 status=none
        mkfs.fat -F 16 "$BUILD_DIR/part.img" > /dev/null
        mcopy -i "$BUILD_DIR/part.img" "$UIMAGE"             ::uImage
        mcopy -i "$BUILD_DIR/part.img" "$BUILD_DIR/boot.scr" ::boot.scr

        dd if="$BUILD_DIR/part.img" of="$DISK" bs=512 seek=2048 conv=notrunc status=none
        rm "$BUILD_DIR/part.img"

        echo "[+] uImage: $UIMAGE"
        echo "[+] U-Boot disk: $DISK"
        ;;

    # ------------------------------------------------------------------
    # PI: SD card layout for Pi Zero W GPU bootloader
    # Expects firmware files in bootloader/pi/:
    #   bootcode.bin, start.elf, fixup.dat
    # Load address 0x8000 matches linker-pi.ld
    # ------------------------------------------------------------------
    pi)
        KERNEL_ELF="$BUILD_DIR/kernel-pi.elf"
        FIRMWARE_DIR="$WORKSPACE_ROOT/bootloader/pi"
        DISK="$BUILD_DIR/pi_sd.img"

        if [[ ! -f "$KERNEL_ELF" ]]; then
            echo "[!] $KERNEL_ELF not found. Run: build.sh --target=pi"
            exit 1
        fi

        fetch_pi_firmware "$FIRMWARE_DIR"

        arm-none-eabi-objcopy -O binary "$KERNEL_ELF" "$BUILD_DIR/kernel.img"

        echo "[*] Creating Pi SD card image..."
        dd if=/dev/zero of="$DISK" bs=1M count=64 status=none
        parted -s "$DISK" mklabel msdos
        parted -s "$DISK" mkpart primary fat32 1MiB 100%

        dd if=/dev/zero of="$BUILD_DIR/part.img" bs=1M count=63 status=none
        mkfs.fat -F 32 -n BOOT "$BUILD_DIR/part.img" > /dev/null

        mcopy -i "$BUILD_DIR/part.img" "$FIRMWARE_DIR/bootcode.bin" ::bootcode.bin
        mcopy -i "$BUILD_DIR/part.img" "$FIRMWARE_DIR/start.elf"    ::start.elf
        mcopy -i "$BUILD_DIR/part.img" "$FIRMWARE_DIR/fixup.dat"    ::fixup.dat
        mcopy -i "$BUILD_DIR/part.img" "$BUILD_DIR/kernel.img"      ::kernel.img
        mcopy -i "$BUILD_DIR/part.img" \
            "$WORKSPACE_ROOT/bootloader/pi/config.txt"              ::config.txt

        dd if="$BUILD_DIR/part.img" of="$DISK" bs=512 seek=2048 conv=notrunc status=none
        rm "$BUILD_DIR/part.img"

        echo "[+] kernel.img: $BUILD_DIR/kernel.img"
        echo "[+] Pi SD image: $DISK"
        echo "[!] Flash to SD with:"
        echo "    sudo dd if=$DISK of=/dev/sdX bs=4M status=progress && sync"
        ;;
esac