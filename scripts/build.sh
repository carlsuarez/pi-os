#!/bin/bash
set -e

WORKSPACE_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BUILD_DIR="$WORKSPACE_ROOT/build"

# Architecture selection
ARCH="arm"      # default
TARGET="qemu"   # default
DEBUG=0

for arg in "$@"; do
    [[ "$arg" == "--debug"        ]] && DEBUG=1
    [[ "$arg" == "--target=uboot" ]] && TARGET="uboot"
    [[ "$arg" == "--target=pi"    ]] && TARGET="pi"
    [[ "$arg" == "--target=qemu"  ]] && TARGET="qemu"
    [[ "$arg" == "--target=x86"   ]] && TARGET="x86"
    [[ "$arg" == "--arch=arm"     ]] && ARCH="arm"
    [[ "$arg" == "--arch=x86"     ]] && ARCH="x86"
done

# Auto-detect architecture from target if not explicitly set
[[ "$TARGET" == "x86" ]] && ARCH="x86"

echo "[*] Architecture: $ARCH"
echo "[*] Target: $TARGET"

# Architecture-specific configuration
if [[ "$ARCH" == "arm" ]]; then
    RUST_TARGET_JSON="$WORKSPACE_ROOT/targets/armv6-none.json"
    ASM_ASSEMBLER="arm-none-eabi-gcc"
    # Note: no VFP flags here — VFP is not enabled in early boot
    ASM_FLAGS="-mcpu=arm1176jzf-s -mfloat-abi=soft -ffreestanding -nostdlib"
    LINKER_SCRIPT="$WORKSPACE_ROOT/kernel/linker-arm.ld"

    case "$TARGET" in
        uboot) KERNEL_LOAD_ADDR="0x40008000" ;;
        pi)    KERNEL_LOAD_ADDR="0x8000"     ;;
        qemu)  KERNEL_LOAD_ADDR="0x8000"     ;;
        *)
            echo "[!] Error: Unknown ARM target '$TARGET'"
            exit 1
            ;;
    esac

    if [[ $DEBUG -eq 1 ]]; then
        RUST_OUT_DIR="target/armv6-none/debug"
        KERNEL_ELF="$BUILD_DIR/kernel-${TARGET}_debug.elf"
    else
        RUST_OUT_DIR="target/armv6-none/release"
        KERNEL_ELF="$BUILD_DIR/kernel-${TARGET}.elf"
    fi

elif [[ "$ARCH" == "x86" ]]; then
    RUST_TARGET_JSON="$WORKSPACE_ROOT/targets/x86-none.json"
    ASM_ASSEMBLER="gcc"
    ASM_FLAGS="-m32 -ffreestanding -nostdlib"
    LINKER_SCRIPT="$WORKSPACE_ROOT/kernel/linker-x86.ld"

    if [[ $DEBUG -eq 1 ]]; then
        RUST_OUT_DIR="target/x86-none/debug"
        KERNEL_ELF="$BUILD_DIR/kernel-x86_debug.elf"
    else
        RUST_OUT_DIR="target/x86-none/release"
        KERNEL_ELF="$BUILD_DIR/kernel-x86.elf"
    fi

else
    echo "[!] Error: Unknown architecture '$ARCH'"
    exit 1
fi

# Build mode configuration
if [[ $DEBUG -eq 1 ]]; then
    CARGO_PROFILE=""
    CARGO_FLAGS="-C debuginfo=2"
    echo "[*] Build mode: debug"
else
    CARGO_PROFILE="--release"
    CARGO_FLAGS=""
    echo "[*] Build mode: release"
fi

mkdir -p "$BUILD_DIR"

# Assemble architecture-specific .S files
echo "[*] Assembling .S files..."
ASM_OBJS=()

ARCH_ASM_DIR="$WORKSPACE_ROOT/kernel/src/arch/$ARCH"
if [[ -d "$ARCH_ASM_DIR" ]]; then
    while IFS= read -r -d '' f; do
        OBJ="$BUILD_DIR/$(basename "${f%.S}.o")"
        $ASM_ASSEMBLER -c $ASM_FLAGS ${DEBUG:+-g} "$f" -o "$OBJ"
        ASM_OBJS+=("$OBJ")
        echo "    Assembled: $(basename "$f")"
    done < <(find "$ARCH_ASM_DIR" -name '*.S' -print0)
fi

# Build Rust kernel
echo "[*] Building Rust kernel..."

LINK_ARGS=(
    -C link-arg=-T"$LINKER_SCRIPT"
    -C link-arg=--gc-sections
)

# Pass load address to linker script for ARM targets
if [[ "$ARCH" == "arm" ]]; then
    LINK_ARGS+=(-C link-arg=--defsym=KERNEL_LOAD_ADDR="$KERNEL_LOAD_ADDR")
fi

# Add assembled objects to link
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

# Verify the binary
if [[ "$ARCH" == "arm" ]]; then
    if command -v arm-none-eabi-readelf &> /dev/null; then
        echo "[+] Entry point and architecture:"
        arm-none-eabi-readelf -h "$KERNEL_ELF" | grep -E "Machine|Class|Entry"
    fi
fi

if [[ "$ARCH" == "x86" ]]; then
    if command -v grub-file &> /dev/null; then
        if grub-file --is-x86-multiboot2 "$KERNEL_ELF"; then
            echo "[✓] Valid Multiboot2 kernel"
        else
            echo "[!] Warning: Kernel may not be a valid Multiboot2 binary"
        fi
    fi

    if command -v readelf &> /dev/null; then
        echo "[+] Architecture info:"
        readelf -h "$KERNEL_ELF" | grep -E "Machine|Class"
    fi
fi