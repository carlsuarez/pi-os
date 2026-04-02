#!/bin/bash
set -e

KERNEL_BIN="$1"
OUTPUT_IMG="${2:-pi-os-x86.img}"

if [ -z "$KERNEL_BIN" ] || [ ! -f "$KERNEL_BIN" ]; then
    echo "Usage: $0 <kernel_binary> [output_image]"
    exit 1
fi

echo "[+] Creating bootable USB image: $OUTPUT_IMG"

# Create a 64MB disk image
dd if=/dev/zero of="$OUTPUT_IMG" bs=1M count=64 status=progress

# Create partition table and bootable partition
parted "$OUTPUT_IMG" -s mklabel msdos
parted "$OUTPUT_IMG" -s mkpart primary ext2 1MiB 100%
parted "$OUTPUT_IMG" -s set 1 boot on

# Setup loop device
LOOP_DEV=$(sudo losetup -f --show -P "$OUTPUT_IMG")
LOOP_PART="${LOOP_DEV}p1"

echo "[+] Using loop device: $LOOP_DEV"

# Wait for partition device
sleep 1
while [ ! -e "$LOOP_PART" ]; do
    sleep 0.5
done

# Create ext2 filesystem
sudo mkfs.ext2 -L "PI-OS-BOOT" "$LOOP_PART"

# Mount partition
MOUNT_DIR=$(mktemp -d)
sudo mount "$LOOP_PART" "$MOUNT_DIR"

echo "[+] Mounted at: $MOUNT_DIR"

# Create boot directory structure
sudo mkdir -p "$MOUNT_DIR/boot/grub"

# Copy kernel
sudo cp "$KERNEL_BIN" "$MOUNT_DIR/boot/kernel.bin"

# Copy GRUB config
if [ -f "bootloader/grub/grub.cfg" ]; then
    sudo cp bootloader/grub/grub.cfg "$MOUNT_DIR/boot/grub/"
else
    echo "[!] Warning: grub.cfg not found at bootloader/grub/grub.cfg"
    echo "    Creating default config..."
    sudo tee "$MOUNT_DIR/boot/grub/grub.cfg" > /dev/null <<EOF
set timeout=3
set default=0

menuentry "pi-os x86" {
    multiboot2 /boot/kernel.bin
    boot
}
EOF
fi

# Install GRUB
echo "[+] Installing GRUB bootloader..."
sudo grub-install --target=i386-pc --boot-directory="$MOUNT_DIR/boot" "$LOOP_DEV"

# Verify kernel exists
if [ ! -f "$MOUNT_DIR/boot/kernel.bin" ]; then
    echo "[!] Error: Kernel not found after copy!"
    sudo umount "$MOUNT_DIR"
    sudo losetup -d "$LOOP_DEV"
    rmdir "$MOUNT_DIR"
    exit 1
fi

echo "[+] Contents of /boot:"
sudo ls -lh "$MOUNT_DIR/boot/"

# Sync and unmount
sync
sudo umount "$MOUNT_DIR"
sudo losetup -d "$LOOP_DEV"
rmdir "$MOUNT_DIR"

echo ""
echo "[✓] Bootable image created: $OUTPUT_IMG"