use alloc::boxed::Box;
use alloc::sync::Arc;
use common::sync::SpinLock;
use core::cell::OnceCell;
use drivers::{
    device_manager::Device,
    hal::{
        console::DynConsoleOutput, interrupt::DynInterruptController, serial::DynSerialPort,
        timer::DynTimer,
    },
};

struct DeviceManagerCell {
    inner: OnceCell<SpinLock<drivers::device_manager::DeviceManager>>,
}
unsafe impl Sync for DeviceManagerCell {}
unsafe impl Send for DeviceManagerCell {}

static DEVICE_MANAGER: DeviceManagerCell = DeviceManagerCell {
    inner: OnceCell::new(),
};

// Console output
// Holds a platform-specific text-output driver (VGA on x86, None on ARM).
// When Some, console_write() uses it directly and skips the serial fallback.
// When None, console_write() falls back to the serial device from the device
// manager — which is exactly what ARM does today, unchanged.

struct ConsoleCell {
    inner: OnceCell<SpinLock<Box<dyn DynConsoleOutput>>>,
}
unsafe impl Sync for ConsoleCell {}
unsafe impl Send for ConsoleCell {}

static CONSOLE_OUTPUT: ConsoleCell = ConsoleCell {
    inner: OnceCell::new(),
};

pub unsafe fn init_platform(boot_info: drivers::platform::BootInfo) {
    DEVICE_MANAGER
        .inner
        .set(SpinLock::new(drivers::device_manager::DeviceManager::new()))
        .ok()
        .expect("DeviceManager already initialized");

    unsafe {
        drivers::platform::Platform::init(boot_info).expect("Platform initialization failed");
    }
}

pub unsafe fn init_devices() {
    unsafe {
        if let Err(e) = drivers::platform::Platform::init_devices(
            &mut *DEVICE_MANAGER.inner.get().unwrap().lock(),
        ) {
            panic!("{}", e);
        }
    }

    #[cfg(target_arch = "x86")]
    {
        use drivers::peripheral::x86::vga_text::VgaText;
        let vga: Box<dyn DynConsoleOutput> = Box::new(unsafe { VgaText::new() });
        CONSOLE_OUTPUT
            .inner
            .set(SpinLock::new(vga))
            .ok()
            .expect("ConsoleOutput already initialized");
    }
}

pub fn device_manager() -> &'static SpinLock<drivers::device_manager::DeviceManager> {
    DEVICE_MANAGER
        .inner
        .get()
        .expect("DeviceManager not initialized")
}

pub fn serial_console() -> Option<Arc<SpinLock<dyn DynSerialPort>>> {
    device_manager().lock().serial_console()
}

pub fn system_timer() -> Option<Arc<SpinLock<dyn DynTimer>>> {
    device_manager().lock().system_timer()
}

pub fn irq_controller() -> Option<Arc<SpinLock<dyn DynInterruptController>>> {
    device_manager().lock().irq_controller()
}

pub fn print_devices() {
    let dm = device_manager().lock();
    crate::kprintln!("Registered Devices ({} total):", dm.count());
    for name in dm.list() {
        let dev_type = match dm.get(name.as_str()).unwrap() {
            Device::Serial(_) => "Serial",
            Device::Block(_) => "Block",
            Device::FrameBuffer(_) => "FrameBuffer",
            Device::Timer(_) => "Timer",
            Device::InterruptController(_) => "InterruptController",
        };
        crate::kprintln!("  {} ({})", name, dev_type);
    }
}

// console_write()
//
// Priority:
//   1. CONSOLE_OUTPUT (VGA on x86)          — set by init() on x86 only
//   2. device_manager serial console        — always available as fallback

#[inline(always)]
pub fn console_write(s: &str) {
    // Platform text console (VGA on x86)
    if let Some(output) = CONSOLE_OUTPUT.inner.get() {
        output.lock().write_str(s);
        return;
    }

    // Serial fallback
    if let Some(serial) = serial_console() {
        let _ = serial.lock().write(s.as_bytes());
    }
}

/// Print to console without newline
#[macro_export]
macro_rules! kprint {
    ($($arg:tt)*) => {{
        use alloc::format;
        let s = format!($($arg)*);
        $crate::subsystems::console_write(&s);
    }};
}

#[macro_export]
macro_rules! kprintln {
    () => { $crate::kprint!("\n") };
    ($($arg:tt)*) => {{
        $crate::kprint!($($arg)*);
        $crate::kprint!("\n");
    }};
}
