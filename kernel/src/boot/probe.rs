//! Hardware probing — fallback when no boot-protocol info is available.
//!
//! Also contains `register_standard_pc_devices`, called by the
//! Multiboot2 path to unconditionally add the fixed-address ISA
//! devices (serial, PIT, PIC, VGA text) that are always present on a
//! PC regardless of what GRUB reported.

use drivers::platform::{Architecture, DeviceInfo, PlatformBuilder};

// x86

/// Probe standard x86/PC hardware.
///
/// # Safety
/// Must only be called on an actual x86 machine.
pub unsafe fn x86() -> Result<(), &'static str> {
    register_standard_pc_devices();
    Ok(())
}

/// Add the fixed-address ISA devices that every PC has.
///
/// Called both from the probing path and from the Multiboot2 path (which
/// may not enumerate these devices in its tags).
pub fn register_standard_pc_devices() {
    PlatformBuilder::add_device(DeviceInfo {
        name: "serial0",
        compatible: "16550a-uart",
        base_addr: 0x3F8,
        size: 8,
        irq: Some(4),
    });

    PlatformBuilder::add_device(DeviceInfo {
        name: "timer",
        compatible: "i8254-pit",
        base_addr: 0x40,
        size: 4,
        irq: Some(0),
    });

    PlatformBuilder::add_device(DeviceInfo {
        name: "pic",
        compatible: "i8259-pic",
        base_addr: 0x20,
        size: 2,
        irq: None,
    });

    PlatformBuilder::add_device(DeviceInfo {
        name: "vga",
        compatible: "vga-text",
        base_addr: 0xB8000,
        size: 0x8000,
        irq: None,
    });

    // Conservative fallback memory map (Multiboot2 overrides this with
    // precise values when available).
    PlatformBuilder::add_ram_region(
        0x0010_0000,       // start at 1 MB — skip low memory / BIOS area
        127 * 1024 * 1024, // assume 128 MB total
    );

    // ISA hole + VGA frame buffer: mark as MMIO so the allocator never
    // touches this range.
    PlatformBuilder::add_mmio_region(0x000A_0000, 0x0006_0000);
}

// ARM

/// Probe ARM hardware by reading the CPU ID register.
///
/// # Safety
/// Must only be called on an ARM CPU with CP15 access.
pub unsafe fn arm() -> Result<(), &'static str> {
    let cpu_id = unsafe { read_arm_cpu_id() };
    match cpu_id {
        0xB760 => bcm2835(), // ARM1176JZF-S → Pi Zero / Pi 1
        0xC070 => bcm2836(), // Cortex-A7    → Pi 2
        0xD030 => bcm2837(), // Cortex-A53   → Pi 3
        _ => Err("unknown ARM CPU"),
    }
}

#[cfg(target_arch = "arm")]
unsafe fn read_arm_cpu_id() -> u32 {
    let id: u32;
    unsafe {
        core::arch::asm!("mrc p15, 0, {}, c0, c0, 0", out(reg) id);
    }
    (id >> 4) & 0xFFF // Primary Part Number in MIDR[15:4]
}

#[cfg(not(target_arch = "arm"))]
unsafe fn read_arm_cpu_id() -> u32 {
    0
}

fn bcm2835() -> Result<(), &'static str> {
    PlatformBuilder::add_device(DeviceInfo {
        name: "uart0",
        compatible: "arm,pl011",
        base_addr: 0x2020_1000,
        size: 0x1000,
        irq: Some(57),
    });
    PlatformBuilder::add_device(DeviceInfo {
        name: "timer",
        compatible: "brcm,bcm2835-system-timer",
        base_addr: 0x2000_3000,
        size: 0x1000,
        irq: Some(1),
    });
    PlatformBuilder::add_device(DeviceInfo {
        name: "intc",
        compatible: "brcm,bcm2835-armctrl-ic",
        base_addr: 0x2000_B200,
        size: 0x200,
        irq: None,
    });
    PlatformBuilder::add_ram_region(0x0000_0000, 512 * 1024 * 1024);
    PlatformBuilder::add_mmio_region(0x2000_0000, 0x0100_0000);
    Ok(())
}

fn bcm2836() -> Result<(), &'static str> {
    PlatformBuilder::add_device(DeviceInfo {
        name: "uart0",
        compatible: "arm,pl011",
        base_addr: 0x3F20_1000,
        size: 0x1000,
        irq: Some(57),
    });
    PlatformBuilder::add_device(DeviceInfo {
        name: "timer",
        compatible: "arm,armv7-timer",
        base_addr: 0,
        size: 0,
        irq: Some(30),
    });
    PlatformBuilder::add_device(DeviceInfo {
        name: "intc",
        compatible: "brcm,bcm2835-armctrl-ic",
        base_addr: 0x3F00_B200,
        size: 0x200,
        irq: None,
    });
    PlatformBuilder::add_ram_region(0x0000_0000, 1024 * 1024 * 1024);
    PlatformBuilder::add_mmio_region(0x3F00_0000, 0x0100_0000);
    Ok(())
}

fn bcm2837() -> Result<(), &'static str> {
    PlatformBuilder::add_device(DeviceInfo {
        name: "uart0",
        compatible: "arm,pl011",
        base_addr: 0x3F20_1000,
        size: 0x1000,
        irq: Some(57),
    });
    PlatformBuilder::add_device(DeviceInfo {
        name: "timer",
        compatible: "arm,armv8-timer",
        base_addr: 0,
        size: 0,
        irq: Some(30),
    });
    PlatformBuilder::add_device(DeviceInfo {
        name: "intc",
        compatible: "brcm,bcm2835-armctrl-ic",
        base_addr: 0x3F00_B200,
        size: 0x200,
        irq: None,
    });
    PlatformBuilder::add_ram_region(0x0000_0000, 1024 * 1024 * 1024);
    PlatformBuilder::add_mmio_region(0x3F00_0000, 0x0100_0000); // same window as BCM2836
    Ok(())
}
