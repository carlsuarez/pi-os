#!/bin/bash
set -e

WORKSPACE_ROOT="$(pwd)"
BUILD_DIR="$WORKSPACE_ROOT/build"
KERNEL_ELF="$WORKSPACE_ROOT/kernel.elf"
LINKER_SCRIPT="$WORKSPACE_ROOT/kernel/linker.ld"
ASM_FILES=$(find "$WORKSPACE_ROOT/kernel" -name '*.S')
RUST_TARGET_JSON="$WORKSPACE_ROOT/targets/armv6-none.json"

mkdir -p "$BUILD_DIR"

echo "[*] Assembling .S files..."
ASM_OBJS=()
for f in $ASM_FILES; do
    OBJ="$BUILD_DIR/$(basename ${f%.S}.o)"
    arm-none-eabi-gcc \
    -c \
    -g \
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
cargo rustc \
    --target "$RUST_TARGET_JSON" \
    -Z build-std=core,compiler_builtins \
    -Z build-std-features=compiler-builtins-mem \
    -p kernel \
    -- \
    -C debuginfo=2 \
    -C link-arg=-T"$LINKER_SCRIPT" \
    -C link-arg=--gc-sections \
    ${ASM_OBJS[@]/#/-C link-arg=}

RUST_ELF="target/armv6-none/debug/kernel"
cp "$RUST_ELF" "$KERNEL_ELF"

echo "[*] Debug build complete"
echo "[*] Starting QEMU in debug mode (waiting for GDB on port 1234)..."

qemu-system-arm \
    -M raspi0 \
    -kernel "$KERNEL_ELF" \
    -nographic \
    -S \
    -s