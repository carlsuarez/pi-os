#!/bin/bash
set -e

echo "[*] Starting QEMU"
qemu-system-arm \
    -M raspi0 \
    -kernel "$1" \
    -nographic \
    -D qemu.log \
    -d int,guest_errors