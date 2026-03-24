//! Unified Platform Abstraction Layer
//!
//! This module provides a single platform implementation that discovers hardware
//! at runtime using boot information (Multiboot2, Device Tree, ACPI, or probing).
//!
//! # Architecture Support
//!
//! - **x86**: Uses Multiboot2 from GRUB, falls back to standard PC hardware probing
//! - **ARM**: Uses Device Tree if provided, falls back to CPU ID probing
//!
//! # Usage
//!
//! ```rust
//! use drivers::platform::{Platform, BootInfo};
//!
//! unsafe {
//!     Platform::init(BootInfo::Multiboot2 {
//!         magic: multiboot_magic,
//!         info_addr: multiboot_info,
//!     }).expect("Platform init failed");
//! }
//!
//! let memory = Platform::memory_regions();
//! let uart = Platform::find_device("serial0").unwrap();
//! ```

use crate::device_manager::DeviceManager;
use crate::peripheral::*;
use core::sync::atomic::{AtomicBool, Ordering};

const MAX_DEVICES: usize = 32;
const MAX_MEMORY_REGIONS: usize = 64;

// ============================================================================
// Public Types
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryType {
    Available,
    Reserved,
    Mmio,
    Kernel,
    Framebuffer,
}

#[derive(Debug, Clone, Copy)]
pub struct MemoryRegion {
    pub base: usize,
    pub size: usize,
    pub mem_type: MemoryType,
}

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub name: &'static str,
    pub compatible: &'static str,
    pub base_addr: usize,
    pub size: usize,
    pub irq: Option<u32>,
}

#[derive(Debug)]
pub enum BootInfo {
    Multiboot2 { magic: u32, info_addr: usize },
    DeviceTree { dtb_addr: usize },
    Raw,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Architecture {
    X86,
    X86_64,
    Arm,
    AArch64,
}

/// Structured memory map summary for the kernel memory manager.
#[derive(Debug, Clone, Copy)]
pub struct MemoryMap {
    /// Base of main RAM region
    pub ram_start: usize,
    /// Total size of main RAM region
    pub ram_size: usize,
    /// Base of MMIO/peripheral region (mapped as Device memory by MMU)
    pub peripheral_base: usize,
    /// Total size of MMIO/peripheral region
    pub peripheral_size: usize,
}

// ============================================================================
// Static Storage
// ============================================================================

static mut MEMORY_REGIONS: [MemoryRegion; MAX_MEMORY_REGIONS] = [MemoryRegion {
    base: 0,
    size: 0,
    mem_type: MemoryType::Reserved,
}; MAX_MEMORY_REGIONS];
static mut MEMORY_REGION_COUNT: usize = 0;

static mut DEVICES: [Option<DeviceInfo>; MAX_DEVICES] = [const { None }; MAX_DEVICES];
static mut DEVICE_COUNT: usize = 0;

static mut CMDLINE: Option<&'static str> = None;
static mut PLATFORM_NAME: &'static str = "Unknown";
static mut ARCH: Architecture = Architecture::X86;

static INITIALIZED: AtomicBool = AtomicBool::new(false);

// ============================================================================
// Platform
// ============================================================================

pub struct Platform;

impl Platform {
    pub fn name() -> &'static str {
        unsafe { PLATFORM_NAME }
    }

    pub fn arch() -> &'static str {
        match unsafe { ARCH } {
            Architecture::X86 => "x86",
            Architecture::X86_64 => "x86_64",
            Architecture::Arm => "arm",
            Architecture::AArch64 => "aarch64",
        }
    }

    pub fn cmdline() -> Option<&'static str> {
        unsafe { CMDLINE }
    }

    /// Initialize platform from boot information.
    ///
    /// Tries discovery methods in order:
    /// 1. Multiboot2 (x86 GRUB)
    /// 2. Device Tree (ARM)
    /// 3. ACPI (future)
    /// 4. Hardware probing (fallback)
    ///
    /// # Safety
    /// Must be called exactly once, very early in boot before any memory
    /// management is initialized.
    pub unsafe fn init(boot_info: BootInfo) -> Result<(), &'static str> {
        if INITIALIZED.swap(true, Ordering::SeqCst) {
            return Err("Platform already initialized");
        }

        unsafe {
            ARCH = detect_architecture();

            let discovered = match boot_info {
                BootInfo::Multiboot2 { magic, info_addr } => {
                    Self::discover_from_multiboot2(magic, info_addr).is_ok()
                }
                BootInfo::DeviceTree { dtb_addr } => {
                    Self::discover_from_device_tree(dtb_addr).is_ok()
                }
                BootInfo::Raw => false,
            };

            if !discovered {
                #[cfg(target_arch = "x86")]
                {
                    if Self::discover_from_acpi().is_err() {
                        Self::discover_from_probing()?;
                    }
                }

                #[cfg(target_arch = "arm")]
                {
                    Self::discover_from_probing()?;
                }
            }

            PLATFORM_NAME = Self::determine_platform_name();
        }

        Ok(())
    }

    /// All discovered memory regions.
    pub fn memory_regions() -> &'static [MemoryRegion] {
        unsafe { &MEMORY_REGIONS[..MEMORY_REGION_COUNT] }
    }

    /// Total available RAM across all Available regions.
    pub fn total_ram() -> usize {
        Self::memory_regions()
            .iter()
            .filter(|r| r.mem_type == MemoryType::Available)
            .map(|r| r.size)
            .sum()
    }

    /// Structured memory map for the kernel memory manager.
    ///
    /// Returns the largest Available region as RAM, and the spanning range
    /// of all Mmio regions as the peripheral window. Panics if the platform
    /// has not been initialized or no RAM region exists.
    pub fn memory_map() -> MemoryMap {
        let ram = Self::memory_regions()
            .iter()
            .filter(|r| r.mem_type == MemoryType::Available)
            .max_by_key(|r| r.size)
            .expect("No available RAM region found");

        // Compute the spanning range of all MMIO regions
        let periph_base = Self::memory_regions()
            .iter()
            .filter(|r| r.mem_type == MemoryType::Mmio)
            .map(|r| r.base)
            .min();

        let periph_end = Self::memory_regions()
            .iter()
            .filter(|r| r.mem_type == MemoryType::Mmio)
            .map(|r| r.base + r.size)
            .max();

        let (peripheral_base, peripheral_size) = match (periph_base, periph_end) {
            (Some(base), Some(end)) => (base, end.saturating_sub(base)),
            _ => (0, 0),
        };

        MemoryMap {
            ram_start: ram.base,
            ram_size: ram.size,
            peripheral_base,
            peripheral_size,
        }
    }

    /// Find a device by name or compatible string.
    pub fn find_device(name_or_compat: &str) -> Option<&'static DeviceInfo> {
        unsafe {
            DEVICES[..DEVICE_COUNT]
                .iter()
                .filter_map(|d| d.as_ref())
                .find(|d| d.name == name_or_compat || d.compatible.contains(name_or_compat))
        }
    }

    /// Iterator over all discovered devices.
    pub fn devices() -> impl Iterator<Item = &'static DeviceInfo> + 'static {
        unsafe { (0..DEVICE_COUNT).filter_map(|i| DEVICES[i].as_ref()) }
    }

    /// Initialize and register all platform devices with the device manager.
    ///
    /// # Safety
    /// Must be called after `Platform::init()` and after memory management
    /// is initialized (drivers may allocate).
    pub unsafe fn init_devices(device_mgr: &mut DeviceManager) -> Result<(), &'static str> {
        unsafe {
            for device in Self::devices() {
                match device.compatible {
                    // --------------------------------------------------------
                    // UART
                    // --------------------------------------------------------
                    "arm,pl011" | "arm,primecell" => {
                        let uart = arm::pl011::Pl011::new(device.base_addr);
                        device_mgr.register_serial(device.name, uart)?;
                    }

                    "16550a-uart" | "ns16550a" => {
                        let uart = if ARCH == Architecture::X86 {
                            x86::uart16550::Uart16550::new_pio(device.base_addr as u16)
                        } else {
                            x86::uart16550::Uart16550::new_mmio(device.base_addr)
                        };
                        device_mgr.register_serial(device.name, uart)?;
                    }

                    // --------------------------------------------------------
                    // Timers
                    // --------------------------------------------------------
                    "brcm,bcm2835-system-timer" => {
                        let timer = bcm2835::timer::Bcm2835Timer::new(device.base_addr);
                        device_mgr.register_timer(device.name, timer)?;
                    }

                    "arm,armv7-timer" | "arm,armv8-timer" => {
                        todo!("ARMv7/ARMv8 generic timer driver");
                    }

                    "i8254-pit" | "intel,8254" => {
                        todo!("I8254 PIT driver");
                    }

                    // --------------------------------------------------------
                    // Interrupt Controllers
                    // --------------------------------------------------------
                    "brcm,bcm2835-armctrl-ic" | "brcm,bcm2836-armctrl-ic" => {
                        let intc = bcm2835::intc::Bcm2835InterruptController::new(device.base_addr);
                        device_mgr.register_interrupt_controller(device.name, intc)?;
                    }

                    "arm,gic-400" | "arm,cortex-a15-gic" | "arm,gic-v3" => {
                        todo!("ARM GIC driver");
                    }

                    "i8259-pic" | "intel,8259" => {
                        todo!("I8259 PIC driver");
                    }

                    // --------------------------------------------------------
                    // Framebuffer
                    // --------------------------------------------------------
                    "multiboot2-fb" | "simple-framebuffer" => {
                        todo!("Simple framebuffer driver");
                    }

                    // --------------------------------------------------------
                    // Block Devices
                    // --------------------------------------------------------
                    "brcm,bcm2835-sdhost" | "brcm,bcm2711-emmc2" => {
                        let block_dev = bcm2835::emmc::Emmc::new(device.base_addr);
                        device_mgr.register_block(device.name, block_dev)?;
                    }

                    _ => {
                        #[cfg(feature = "log")]
                        log::warn!(
                            "Unknown device type: {} ({})",
                            device.name,
                            device.compatible
                        );
                    }
                }
            }
        }

        Ok(())
    }

    // ========================================================================
    // Discovery
    // ========================================================================

    unsafe fn discover_from_multiboot2(magic: u32, info_addr: usize) -> Result<(), &'static str> {
        unsafe {
            if magic != 0x36d76289 {
                return Err("Invalid Multiboot2 magic");
            }

            let total_size = *(info_addr as *const u32);
            let mut tag_addr = info_addr + 8;
            let end_addr = info_addr + total_size as usize;

            while tag_addr < end_addr {
                tag_addr = (tag_addr + 7) & !7;

                let tag_type = *(tag_addr as *const u32);
                let tag_size = *((tag_addr + 4) as *const u32);

                if tag_type == 0 {
                    break;
                }

                match tag_type {
                    1 => parse_multiboot2_cmdline(tag_addr),
                    6 => parse_multiboot2_memory_map(tag_addr)?,
                    8 => parse_multiboot2_framebuffer(tag_addr),
                    _ => {}
                }

                tag_addr += tag_size as usize;
            }

            register_standard_pc_devices()?;
        }

        Ok(())
    }

    unsafe fn discover_from_device_tree(dtb_addr: usize) -> Result<(), &'static str> {
        #[cfg(feature = "device-tree")]
        {
            use fdt::Fdt;

            let fdt = Fdt::from_ptr(dtb_addr as *const u8).map_err(|_| "Invalid device tree")?;

            for node in fdt.memory() {
                for region in node.regions() {
                    add_memory_region(MemoryRegion {
                        base: region.starting_address as usize,
                        size: region.size.unwrap_or(0),
                        mem_type: MemoryType::Available,
                    });
                }
            }

            for node in fdt.all_nodes() {
                if let Some(compatible) = node.compatible() {
                    if let Some(compat_str) = compatible.first() {
                        if let Some(reg) = node.reg() {
                            if let Some(region) = reg.next() {
                                add_device(DeviceInfo {
                                    name: node.name,
                                    compatible: compat_str,
                                    base_addr: region.starting_address as usize,
                                    size: region.size.unwrap_or(0),
                                    irq: node.interrupts().and_then(|mut i| i.next()),
                                });
                            }
                        }
                    }
                }
            }

            Ok(())
        }

        #[cfg(not(feature = "device-tree"))]
        Err("Device tree support not enabled")
    }

    unsafe fn discover_from_acpi() -> Result<(), &'static str> {
        Err("ACPI discovery not implemented")
    }

    unsafe fn discover_from_probing() -> Result<(), &'static str> {
        unsafe {
            match ARCH {
                Architecture::X86 | Architecture::X86_64 => probe_x86_hardware(),
                Architecture::Arm | Architecture::AArch64 => probe_arm_hardware(),
            }
        }
    }

    unsafe fn determine_platform_name() -> &'static str {
        unsafe {
            match ARCH {
                Architecture::X86 | Architecture::X86_64 => "Generic PC (x86)",
                Architecture::Arm | Architecture::AArch64 => {
                    if DEVICES[..DEVICE_COUNT].iter().any(|d| {
                        d.as_ref()
                            .map_or(false, |dev| dev.compatible.contains("bcm2835"))
                    }) {
                        "Broadcom BCM2835 (Raspberry Pi Zero/1)"
                    } else if DEVICES[..DEVICE_COUNT].iter().any(|d| {
                        d.as_ref()
                            .map_or(false, |dev| dev.compatible.contains("bcm2836"))
                    }) {
                        "Broadcom BCM2836 (Raspberry Pi 2)"
                    } else if DEVICES[..DEVICE_COUNT].iter().any(|d| {
                        d.as_ref()
                            .map_or(false, |dev| dev.compatible.contains("bcm2837"))
                    }) {
                        "Broadcom BCM2837 (Raspberry Pi 3)"
                    } else if DEVICES[..DEVICE_COUNT].iter().any(|d| {
                        d.as_ref()
                            .map_or(false, |dev| dev.compatible.contains("bcm2711"))
                    }) {
                        "Broadcom BCM2711 (Raspberry Pi 4)"
                    } else {
                        "ARM-based Platform"
                    }
                }
            }
        }
    }
}

// ============================================================================
// Helpers
// ============================================================================

fn detect_architecture() -> Architecture {
    #[cfg(all(target_arch = "x86", target_pointer_width = "32"))]
    return Architecture::X86;

    #[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
    return Architecture::X86_64;

    #[cfg(all(target_arch = "arm", target_pointer_width = "32"))]
    return Architecture::Arm;

    #[cfg(all(target_arch = "aarch64", target_pointer_width = "64"))]
    return Architecture::AArch64;
}

unsafe fn add_memory_region(region: MemoryRegion) {
    unsafe {
        if MEMORY_REGION_COUNT < MAX_MEMORY_REGIONS {
            MEMORY_REGIONS[MEMORY_REGION_COUNT] = region;
            MEMORY_REGION_COUNT += 1;
        }
    }
}

unsafe fn add_device(device: DeviceInfo) {
    unsafe {
        if DEVICE_COUNT < MAX_DEVICES {
            DEVICES[DEVICE_COUNT] = Some(device);
            DEVICE_COUNT += 1;
        }
    }
}

// ============================================================================
// Multiboot2 Parsing
// ============================================================================

unsafe fn parse_multiboot2_cmdline(tag_addr: usize) {
    unsafe {
        let string_ptr = (tag_addr + 8) as *const u8;
        let mut len = 0;
        while *string_ptr.add(len) != 0 {
            len += 1;
        }
        if let Ok(s) = core::str::from_utf8(core::slice::from_raw_parts(string_ptr, len)) {
            CMDLINE = Some(s);
        }
    }
}

unsafe fn parse_multiboot2_memory_map(tag_addr: usize) -> Result<(), &'static str> {
    unsafe {
        let entry_size = *((tag_addr + 8) as *const u32);
        let entry_version = *((tag_addr + 12) as *const u32);

        if entry_version != 0 {
            return Err("Unsupported memory map version");
        }

        let tag_size = *((tag_addr + 4) as *const u32);
        let mut entry = tag_addr + 16;
        let entries_end = tag_addr + tag_size as usize;

        while entry < entries_end {
            let base = *(entry as *const u64);
            let length = *((entry + 8) as *const u64);
            let entry_type = *((entry + 16) as *const u32);

            let mem_type = match entry_type {
                1 => MemoryType::Available,
                3 => MemoryType::Mmio,
                _ => MemoryType::Reserved,
            };

            add_memory_region(MemoryRegion {
                base: base as usize,
                size: length as usize,
                mem_type,
            });

            entry += entry_size as usize;
        }
    }

    Ok(())
}

unsafe fn parse_multiboot2_framebuffer(tag_addr: usize) {
    unsafe {
        let fb_addr = *((tag_addr + 8) as *const u64);
        let fb_pitch = *((tag_addr + 16) as *const u32);
        let fb_height = *((tag_addr + 24) as *const u32);

        add_device(DeviceInfo {
            name: "framebuffer",
            compatible: "multiboot2-fb",
            base_addr: fb_addr as usize,
            size: (fb_pitch * fb_height) as usize,
            irq: None,
        });
    }
}

// ============================================================================
// x86 Probing
// ============================================================================

unsafe fn probe_x86_hardware() -> Result<(), &'static str> {
    unsafe { register_standard_pc_devices() }
}

unsafe fn register_standard_pc_devices() -> Result<(), &'static str> {
    unsafe {
        add_device(DeviceInfo {
            name: "serial0",
            compatible: "16550a-uart",
            base_addr: 0x3F8,
            size: 8,
            irq: Some(4),
        });

        add_device(DeviceInfo {
            name: "timer",
            compatible: "i8254-pit",
            base_addr: 0x40,
            size: 4,
            irq: Some(0),
        });

        add_device(DeviceInfo {
            name: "pic",
            compatible: "i8259-pic",
            base_addr: 0x20,
            size: 2,
            irq: None,
        });

        add_device(DeviceInfo {
            name: "vga",
            compatible: "vga-text",
            base_addr: 0xB8000,
            size: 0x8000,
            irq: None,
        });

        // Standard PC memory map: first 640KB available, then reserved
        // up to 1MB, then extended memory.  Multiboot2 overrides this
        // with precise values from the bootloader; this is only the
        // probing fallback.
        add_memory_region(MemoryRegion {
            base: 0x0010_0000,       // 1MB — skip low memory/BIOS area
            size: 127 * 1024 * 1024, // assume 128MB total, conservative
            mem_type: MemoryType::Available,
        });

        // ISA hole and VGA: mark as MMIO so the allocator never touches it
        add_memory_region(MemoryRegion {
            base: 0x000A_0000,
            size: 0x0006_0000, // 0xA0000 – 0x100000
            mem_type: MemoryType::Mmio,
        });
    }

    Ok(())
}

// ============================================================================
// ARM Probing
// ============================================================================

unsafe fn probe_arm_hardware() -> Result<(), &'static str> {
    unsafe {
        let cpu_id = read_arm_cpu_id();
        match cpu_id {
            0xB760 => probe_bcm2835(), // ARM1176JZF-S (Pi Zero / Pi 1)
            0xC070 => probe_bcm2836(), // Cortex-A7    (Pi 2)
            0xD030 => probe_bcm2837(), // Cortex-A53   (Pi 3)
            _ => Err("Unknown ARM CPU"),
        }
    }
}

#[cfg(target_arch = "arm")]
unsafe fn read_arm_cpu_id() -> u32 {
    unsafe {
        let id: u32;
        core::arch::asm!("mrc p15, 0, {}, c0, c0, 0", out(reg) id);
        // Primary Part Number lives in [15:4] of MIDR
        (id >> 4) & 0xFFF
    }
}

#[cfg(not(target_arch = "arm"))]
unsafe fn read_arm_cpu_id() -> u32 {
    0
}

unsafe fn probe_bcm2835() -> Result<(), &'static str> {
    unsafe {
        add_device(DeviceInfo {
            name: "uart0",
            compatible: "arm,pl011",
            base_addr: 0x2020_1000,
            size: 0x1000,
            irq: Some(57),
        });

        add_device(DeviceInfo {
            name: "timer",
            compatible: "brcm,bcm2835-system-timer",
            base_addr: 0x2000_3000,
            size: 0x1000,
            irq: Some(1),
        });

        add_device(DeviceInfo {
            name: "intc",
            compatible: "brcm,bcm2835-armctrl-ic",
            base_addr: 0x2000_B200,
            size: 0x200,
            irq: None,
        });

        // RAM: full 512MB starting at 0x0.
        // setup_memory_management trims this to _free_memory_start before
        // initializing the heap and page allocator.
        add_memory_region(MemoryRegion {
            base: 0x0000_0000,
            size: 512 * 1024 * 1024,
            mem_type: MemoryType::Available,
        });

        // BCM2835 peripheral window: 0x20000000 – 0x21000000 (16MB).
        // Mapped as Device memory by the MMU; never handed to the allocator.
        add_memory_region(MemoryRegion {
            base: 0x2000_0000,
            size: 0x0100_0000,
            mem_type: MemoryType::Mmio,
        });
    }

    Ok(())
}

unsafe fn probe_bcm2836() -> Result<(), &'static str> {
    unsafe {
        add_device(DeviceInfo {
            name: "uart0",
            compatible: "arm,pl011",
            base_addr: 0x3F20_1000,
            size: 0x1000,
            irq: Some(57),
        });

        add_device(DeviceInfo {
            name: "timer",
            compatible: "arm,armv7-timer",
            base_addr: 0,
            size: 0,
            irq: Some(30),
        });

        add_device(DeviceInfo {
            name: "intc",
            compatible: "brcm,bcm2835-armctrl-ic",
            base_addr: 0x3F00_B200,
            size: 0x200,
            irq: None,
        });

        add_memory_region(MemoryRegion {
            base: 0x0000_0000,
            size: 1024 * 1024 * 1024,
            mem_type: MemoryType::Available,
        });

        // BCM2836 peripheral window: 0x3F000000 – 0x40000000 (16MB)
        add_memory_region(MemoryRegion {
            base: 0x3F00_0000,
            size: 0x0100_0000,
            mem_type: MemoryType::Mmio,
        });
    }

    Ok(())
}

unsafe fn probe_bcm2837() -> Result<(), &'static str> {
    unsafe {
        add_device(DeviceInfo {
            name: "uart0",
            compatible: "arm,pl011",
            base_addr: 0x3F20_1000,
            size: 0x1000,
            irq: Some(57),
        });

        add_device(DeviceInfo {
            name: "timer",
            compatible: "arm,armv8-timer",
            base_addr: 0,
            size: 0,
            irq: Some(30),
        });

        add_device(DeviceInfo {
            name: "intc",
            compatible: "brcm,bcm2835-armctrl-ic",
            base_addr: 0x3F00_B200,
            size: 0x200,
            irq: None,
        });

        add_memory_region(MemoryRegion {
            base: 0x0000_0000,
            size: 1024 * 1024 * 1024,
            mem_type: MemoryType::Available,
        });

        // BCM2837 shares the same peripheral window as BCM2836
        add_memory_region(MemoryRegion {
            base: 0x3F00_0000,
            size: 0x0100_0000,
            mem_type: MemoryType::Mmio,
        });
    }

    Ok(())
}
