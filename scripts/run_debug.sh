#!/bin/bash
set -e

KERNEL_ELF="$WORKSPACE_ROOT/kernel_debug.elf"

echo "[*] Starting QEMU in debug mode (waiting for GDB on port 1234)..."

qemu-system-arm \
    -M raspi0 \
    -kernel "$KERNEL_ELF" \
    -nographic \
    -S \
    -s