#!/bin/bash
set -e

WORKSPACE_ROOT="$(pwd)"
BUILD_DIR="$WORKSPACE_ROOT/build"
KERNEL_ELF="$BUILD_DIR/kernel_debug.elf"
LINKER_SCRIPT="$WORKSPACE_ROOT/kernel/linker.ld"
ASM_FILES=$(find "$WORKSPACE_ROOT/kernel" -name '*.S')
RUST_TARGET_JSON="$WORKSPACE_ROOT/targets/armv6-none.json"

mkdir -p "$BUILD_DIR"

echo "[*] Assembling .S files..."
ASM_OBJS=()
for f in $ASM_FILES; do
    OBJ="$BUILD_DIR/$(basename ${f%.S}.o)"
    arm-none-eabi-gcc \
        -c -g \
        -mcpu=arm1176jzf-s \
        -mfloat-abi=hard \
        -mfpu=vfp \
        -ffreestanding \
        -nostdlib \
        "$f" \
        -o "$OBJ"
    ASM_OBJS+=("$OBJ")
done

echo "[*] Building Rust kernel (debug)..."
cargo +nightly rustc \
    -Z build-std=core,alloc,compiler_builtins \
    -Z build-std-features=compiler-builtins-mem \
    -Z json-target-spec \
    -p kernel \
    --target "$RUST_TARGET_JSON" \
    -- \
    -C debuginfo=2 \
    -C link-arg=-T"$LINKER_SCRIPT" \
    -C link-arg=--gc-sections \
    ${ASM_OBJS[@]/#/-C link-arg=}

RUST_ELF="target/armv6-none/debug/kernel"
cp "$RUST_ELF" "$KERNEL_ELF"

echo "[*] Creating FAT32 test image..."
IMG_FILE="$BUILD_DIR/rootfs.img"
IMG_SIZE=64M
TMP_MOUNT="$BUILD_DIR/tmp_mount"

dd if=/dev/zero of="$IMG_FILE" bs=1M count=64
mkfs.fat -F32 -n ROOTFS "$IMG_FILE"

mkdir -p "$TMP_MOUNT"
sudo mount -o loop "$IMG_FILE" "$TMP_MOUNT"

# create a test file
echo "Hello from QEMU FAT32 (debug)!" | sudo tee "$TMP_MOUNT/test.txt" > /dev/null

sudo umount "$TMP_MOUNT"
rmdir "$TMP_MOUNT"

echo "[*] Debug build complete: $KERNEL_ELF"
echo "[*] FAT32 image created: $IMG_FILE"