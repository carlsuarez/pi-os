pub mod boot_sinks;
pub mod log_sinks;

use crate::subsystems::boot_sinks::BootSink;
use alloc::format;
use alloc::sync::Arc;
use alloc::{boxed::Box, string::String};
use core::cell::OnceCell;
use drivers::peripheral::x86::mb2fb::Mb2Fb;
use drivers::{
    device_manager::Device,
    hal::{
        console::DynConsoleOutput, interrupt::DynInterruptController, serial::DynSerialPort,
        timer::DynTimer,
    },
    peripheral::x86::mb2fb::MB2_FB_TAG,
};
use spin::Mutex;

struct DeviceManagerCell {
    inner: OnceCell<Mutex<drivers::device_manager::DeviceManager>>,
}
unsafe impl Sync for DeviceManagerCell {}
unsafe impl Send for DeviceManagerCell {}

static DEVICE_MANAGER: DeviceManagerCell = DeviceManagerCell {
    inner: OnceCell::new(),
};

pub unsafe fn init_devices() {
    DEVICE_MANAGER
        .inner
        .set(Mutex::new(drivers::device_manager::DeviceManager::new()))
        .ok()
        .expect("DeviceManager already initialized");

    unsafe {
        drivers::platform::Platform::init_devices(&mut *DEVICE_MANAGER.inner.get().unwrap().lock())
            .expect("Failed to initialize platform devices");
    }
}

pub fn device_manager() -> &'static Mutex<drivers::device_manager::DeviceManager> {
    DEVICE_MANAGER
        .inner
        .get()
        .expect("DeviceManager not initialized")
}

pub fn serial_console() -> Option<Arc<Mutex<dyn DynSerialPort>>> {
    device_manager().lock().serial_console()
}

pub fn system_timer() -> Option<Arc<Mutex<dyn DynTimer>>> {
    device_manager().lock().system_timer()
}

pub fn irq_controller() -> Option<Arc<Mutex<dyn DynInterruptController>>> {
    device_manager().lock().irq_controller()
}

pub fn print_devices() {
    let dm = device_manager().lock();
    log::info!("Registered Devices ({} total):\n", dm.count());
    for name in dm.list() {
        let dev_type = match dm.get(name.as_str()).unwrap() {
            Device::Serial(_) => "Serial",
            Device::Block(_) => "Block",
            Device::FrameBuffer(_) => "FrameBuffer",
            Device::Timer(_) => "Timer",
            Device::InterruptController(_) => "InterruptController",
        };
        log::info!("  {} ({})\n", name, dev_type);
    }
}

cfg_if::cfg_if! {
    if #[cfg(target_arch = "x86")] {
        use boot_sinks::x86::X86BootSink;
        pub type BootSinkImpl = X86BootSink;
        static BOOT_SINK: X86BootSink = X86BootSink;
    } else if #[cfg(target_arch = "arm")] {
        use boot_sinks::arm::ArmBootSink;
        pub type BootSinkImpl = ArmBootSink;
        static BOOT_SINK: ArmBootSink = ArmBootSink;
    } else {
        use boot_sinks::null::NullSink;
        pub type BootSinkImpl = NullSink;
        static NULL_SINK: NullSink = NullSink;
    }
}

pub fn boot_console() -> &'static impl BootSink {
    cfg_if::cfg_if! {
        if #[cfg(target_arch = "x86")] {
            &BOOT_SINK
        } else if #[cfg(target_arch = "arm")] {
            &BOOT_SINK
        } else {
            &NULL_SINK
        }
    }
}

// For future. Doesn't work in QEMU
pub fn enable_graphical_framebuffer() -> Result<(), String> {
    let tag = MB2_FB_TAG
        .get()
        .ok_or("No graphical framebuffer available")?;
    let fb =
        unsafe { Mb2Fb::new(*tag) }.map_err(|e| format!("Framebuffer init failed: {:?}", e))?;
    crate::subsystems::device_manager()
        .lock()
        .register_framebuffer("framebuffer", fb)?;
    Ok(())
}
