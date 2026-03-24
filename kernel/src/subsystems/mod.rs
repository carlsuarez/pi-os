use alloc::sync::Arc;
use common::sync::SpinLock;
use core::cell::OnceCell;
use drivers::{
    device_manager::Device,
    hal::{interrupt::DynInterruptController, serial::DynSerialPort, timer::DynTimer},
};

struct DeviceManagerCell {
    inner: OnceCell<SpinLock<drivers::device_manager::DeviceManager>>,
}

unsafe impl Sync for DeviceManagerCell {}
unsafe impl Send for DeviceManagerCell {}

static DEVICE_MANAGER: DeviceManagerCell = DeviceManagerCell {
    inner: OnceCell::new(),
};

pub unsafe fn init(boot_info: drivers::platform::BootInfo) {
    DEVICE_MANAGER
        .inner
        .set(SpinLock::new(drivers::device_manager::DeviceManager::new()))
        .ok()
        .expect("DeviceManager already initialized");

    unsafe {
        drivers::platform::Platform::init(boot_info).expect("Platform initialization failed");
        drivers::platform::Platform::init_devices(&mut *DEVICE_MANAGER.inner.get().unwrap().lock());
    }
}

pub fn device_manager() -> &'static SpinLock<drivers::device_manager::DeviceManager> {
    DEVICE_MANAGER
        .inner
        .get()
        .expect("DeviceManager not initialized")
}

pub fn console() -> Option<Arc<SpinLock<dyn DynSerialPort>>> {
    device_manager().lock().console()
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

#[inline(always)]
pub fn console_write(s: &str) {
    if let Some(console) = console() {
        console.lock().write(s.as_bytes());
    }
}

/// Print to console without newline
#[macro_export]
macro_rules! kprint {
    ($($arg:tt)*) => {{
        use alloc::format;
        let s = format!($($arg)*);
        let _ = $crate::subsystems::console_write(&s);
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
