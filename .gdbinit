# Connect to QEMU
target remote localhost:1234

file kernel.elf
set architecture arm
set disassembly-flavor intel
set pagination off
set arm fallback-mode thumb
set arm force-mode arm

echo \n[GDB] Connected to QEMU. Type 'continue' to start execution.\n