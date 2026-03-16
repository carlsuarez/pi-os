#!/bin/bash
set -e
WORKSPACE_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BUILD_DIR="$WORKSPACE_ROOT/build"
RUST_TARGET_JSON="$WORKSPACE_ROOT/targets/armv6-none.json"
ASM_FLAGS="-mcpu=arm1176jzf-s -mfloat-abi=hard -mfpu=vfp -ffreestanding -nostdlib"

TARGET="qemu"   # default
DEBUG=0

for arg in "$@"; do
    [[ "$arg" == "--debug"  ]] && DEBUG=1
    [[ "$arg" == "--target=uboot" ]] && TARGET="uboot"
    [[ "$arg" == "--target=pi"    ]] && TARGET="pi"
    [[ "$arg" == "--target=qemu"  ]] && TARGET="qemu"
done

# Each target gets its own linker script so load addresses stay separate
case "$TARGET" in
    uboot) LINKER_SCRIPT="$WORKSPACE_ROOT/kernel/linker-uboot.ld" ;;
    pi)    LINKER_SCRIPT="$WORKSPACE_ROOT/kernel/linker-pi.ld"    ;;
    qemu)  LINKER_SCRIPT="$WORKSPACE_ROOT/kernel/linker-pi.ld"  ;;
esac

echo "[*] Target: $TARGET"

if [[ $DEBUG -eq 1 ]]; then
    CARGO_PROFILE=""
    CARGO_FLAGS="-C debuginfo=2"
    RUST_OUT_DIR="target/armv6-none/debug"
    KERNEL_ELF="$BUILD_DIR/kernel-${TARGET}_debug.elf"
    echo "[*] Build mode: debug"
else
    CARGO_PROFILE="--release"
    CARGO_FLAGS=""
    RUST_OUT_DIR="target/armv6-none/release"
    KERNEL_ELF="$BUILD_DIR/kernel-${TARGET}.elf"
    echo "[*] Build mode: release"
fi

mkdir -p "$BUILD_DIR"

echo "[*] Assembling .S files..."
ASM_OBJS=()
while IFS= read -r -d '' f; do
    OBJ="$BUILD_DIR/$(basename "${f%.S}.o")"
    arm-none-eabi-gcc -c $ASM_FLAGS ${DEBUG:+-g} "$f" -o "$OBJ"
    ASM_OBJS+=("$OBJ")
done < <(find "$WORKSPACE_ROOT/kernel" -name '*.S' -print0)

echo "[*] Building Rust kernel..."
LINK_ARGS=(
    -C link-arg=-T"$LINKER_SCRIPT"
    -C link-arg=--gc-sections
)
for obj in "${ASM_OBJS[@]}"; do
    LINK_ARGS+=(-C link-arg="$obj")
done

cargo +nightly rustc $CARGO_PROFILE \
    -Z build-std=core,alloc,compiler_builtins \
    -Z build-std-features=compiler-builtins-mem \
    -Z json-target-spec \
    -p kernel \
    --target "$RUST_TARGET_JSON" \
    -- \
    $CARGO_FLAGS \
    "${LINK_ARGS[@]}"

cp "$RUST_OUT_DIR/kernel" "$KERNEL_ELF"
echo "[+] Kernel ELF: $KERNEL_ELF"