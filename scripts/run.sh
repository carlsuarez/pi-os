#!/bin/bash
set -e

KERNEL="kernel.elf"

echo "[*] Starting QEMU"
qemu-system-arm \
    -M raspi0 \
    -kernel "$KERNEL" \
    -nographic