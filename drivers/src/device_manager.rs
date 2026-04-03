//! Device Manager
//!
//! Central registry for all hardware devices. Devices are registered by the
//! platform during initialization and can be accessed by name or type.
//!
//! # Usage
//!
//! ```rust
//! use drivers::device_manager::{devices, Device};
//!
//! // Platform registers devices during init
//! devices().lock().register("serial0", Device::new_serial(uart));
//!
//! // Kernel accesses devices by name
//! if let Some(serial) = devices().lock().serial("serial0") {
//!     let mut port = serial.lock();
//!     port.write_byte(b'H');
//! }
//!
//! // Or get the serial console (default serial)
//! if let Some(console) = devices().lock().serial_console() {
//!     console.lock().write_str("Hello, world!\n");
//! }
//! ```

use crate::hal::block_device::{BlockDevice, DynBlockDevice};
use crate::hal::fb::FrameBuffer;
use crate::hal::interrupt::{DynInterruptController, InterruptController};
use crate::hal::serial::DynSerialPort;
use crate::hal::timer::DynTimer;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use core::cell::OnceCell;
use spin::Mutex;

struct OnceCellChannel {
    inner: OnceCell<usize>,
}

unsafe impl Sync for OnceCellChannel {}

static SYS_TIMER_CHANNEL: OnceCellChannel = OnceCellChannel {
    inner: OnceCell::new(),
};

/// Device types that can be managed
pub enum Device {
    Serial(Arc<Mutex<dyn DynSerialPort>>),
    Block(Arc<dyn DynBlockDevice>),
    FrameBuffer(Arc<Mutex<dyn FrameBuffer>>),
    Timer(Arc<Mutex<dyn DynTimer>>),
    InterruptController(Arc<Mutex<dyn DynInterruptController>>),
}

impl Device {
    /// Create a serial device from any DynSerialPort implementation
    pub fn new_serial<T: DynSerialPort + 'static>(serial: T) -> Self {
        Device::Serial(Arc::new(Mutex::new(serial)))
    }

    /// Create a block device from any BlockDevice implementation
    pub fn new_block<T: DynBlockDevice + 'static>(block: T) -> Self {
        Device::Block(Arc::new(block))
    }

    /// Create a framebuffer device from any FrameBuffer implementation
    pub fn new_framebuffer<T: FrameBuffer + 'static>(fb: T) -> Self {
        Device::FrameBuffer(Arc::new(Mutex::new(fb)))
    }

    /// Create a timer device from any Timer implementation
    pub fn new_timer<T: DynTimer + 'static>(timer: T) -> Self {
        Device::Timer(Arc::new(Mutex::new(timer)))
    }

    /// Create an interrupt controller from any InterruptController implementation
    pub fn new_interrupt_controller<T: DynInterruptController + 'static>(intc: T) -> Self {
        Device::InterruptController(Arc::new(Mutex::new(intc)))
    }
}

/// Device Manager - Central registry for all hardware devices
pub struct DeviceManager {
    devices: BTreeMap<String, Device>,
}

impl DeviceManager {
    pub const fn new() -> Self {
        Self {
            devices: BTreeMap::new(),
        }
    }

    /// Register a device with a name
    pub fn register(&mut self, name: String, device: Device) {
        self.devices.insert(name, device);
    }

    /// Get a device by name
    pub fn get(&self, name: &str) -> Option<&Device> {
        self.devices.get(name)
    }

    /// List all device names
    pub fn list(&self) -> impl Iterator<Item = &String> {
        self.devices.keys()
    }

    // ========================================================================
    // Type-Specific Accessors
    // ========================================================================

    /// Get a serial port by name
    pub fn serial(&self, name: &str) -> Option<Arc<Mutex<dyn DynSerialPort>>> {
        match self.get(name)? {
            Device::Serial(serial) => Some(Arc::clone(serial)),
            _ => None,
        }
    }

    /// Get a block device by name
    pub fn block(&self, name: &str) -> Option<Arc<dyn DynBlockDevice>> {
        match self.get(name)? {
            Device::Block(block) => Some(Arc::clone(block)),
            _ => None,
        }
    }

    /// Get a framebuffer by name
    pub fn framebuffer(&self, name: &str) -> Option<Arc<Mutex<dyn FrameBuffer>>> {
        match self.get(name)? {
            Device::FrameBuffer(fb) => Some(Arc::clone(fb)),
            _ => None,
        }
    }

    /// Get a timer by name
    pub fn timer(&self, name: &str) -> Option<Arc<Mutex<dyn DynTimer>>> {
        match self.get(name)? {
            Device::Timer(timer) => Some(Arc::clone(timer)),
            _ => None,
        }
    }

    /// Get an interrupt controller by name
    pub fn interrupt_controller(
        &self,
        name: &str,
    ) -> Option<Arc<Mutex<dyn DynInterruptController>>> {
        match self.get(name)? {
            Device::InterruptController(intc) => Some(Arc::clone(intc)),
            _ => None,
        }
    }

    // ========================================================================
    // Convenience Accessors (Common Use Cases)
    // ========================================================================

    /// Get the console (default serial port)
    ///
    /// Tries in order: "console", "serial0", first serial device
    pub fn serial_console(&self) -> Option<Arc<Mutex<dyn DynSerialPort>>> {
        if let Some(console) = self.serial("console") {
            return Some(console);
        }

        if let Some(serial0) = self.serial("serial0") {
            return Some(serial0);
        }

        for (_name, device) in &self.devices {
            if let Device::Serial(serial) = device {
                return Some(serial.clone());
            }
        }

        None
    }

    /// Get the system timer (default timer)
    ///
    /// Tries in order: "system_timer", "timer", first timer device
    pub fn system_timer(&self) -> Option<Arc<Mutex<dyn DynTimer>>> {
        self.timer("system_timer")
            .or_else(|| self.timer("timer"))
            .or_else(|| {
                for (_name, device) in &self.devices {
                    if let Device::Timer(timer) = device {
                        return Some(timer.clone());
                    }
                }
                None
            })
    }

    /// Get the system timer channel if set
    pub fn sys_timer_channel() -> Option<usize> {
        SYS_TIMER_CHANNEL.inner.get().copied()
    }

    /// Get the interrupt controller (default)
    ///
    /// Tries in order: "intc", "pic", "gic", first interrupt controller
    pub fn irq_controller(&self) -> Option<Arc<Mutex<dyn DynInterruptController>>> {
        self.interrupt_controller("intc")
            .or_else(|| self.interrupt_controller("pic"))
            .or_else(|| self.interrupt_controller("gic"))
            .or_else(|| {
                for (_name, device) in &self.devices {
                    if let Device::InterruptController(intc) = device {
                        return Some(intc.clone());
                    }
                }
                None
            })
    }

    // ========================================================================
    // Registration Helpers for Platform
    // ========================================================================

    /// Register a serial port (helper for platform)
    pub fn register_serial<T: DynSerialPort + 'static>(
        &mut self,
        name: impl Into<String>,
        serial: T,
    ) -> Result<(), &'static str> {
        self.register(name.into(), Device::new_serial(serial));
        Ok(())
    }

    /// Register a block device (helper for platform)
    pub fn register_block<T: DynBlockDevice + 'static>(
        &mut self,
        name: impl Into<String>,
        block: T,
    ) -> Result<(), &'static str> {
        self.register(name.into(), Device::new_block(block));
        Ok(())
    }

    /// Register a framebuffer (helper for platform)
    pub fn register_framebuffer<T: FrameBuffer + 'static>(
        &mut self,
        name: impl Into<String>,
        fb: T,
    ) -> Result<(), &'static str> {
        self.register(name.into(), Device::new_framebuffer(fb));
        Ok(())
    }

    /// Register a timer (helper for platform)
    pub fn register_timer<T: DynTimer + 'static>(
        &mut self,
        name: impl Into<String>,
        timer: T,
        sys_channel: Option<usize>,
    ) -> Result<(), &'static str> {
        if let Some(channel) = sys_channel {
            SYS_TIMER_CHANNEL
                .inner
                .set(channel)
                .map_err(|_| "System timer channel already set")?;
        }
        self.register(name.into(), Device::new_timer(timer));
        Ok(())
    }

    /// Register an interrupt controller (helper for platform)
    pub fn register_interrupt_controller<T: DynInterruptController + 'static>(
        &mut self,
        name: impl Into<String>,
        intc: T,
    ) -> Result<(), &'static str> {
        self.register(name.into(), Device::new_interrupt_controller(intc));
        Ok(())
    }

    // ========================================================================
    // Device Counting / Introspection
    // ========================================================================

    /// Count devices of a specific type
    pub fn count_serial(&self) -> usize {
        self.devices
            .values()
            .filter(|d| matches!(d, Device::Serial(_)))
            .count()
    }

    pub fn count_block(&self) -> usize {
        self.devices
            .values()
            .filter(|d| matches!(d, Device::Block(_)))
            .count()
    }

    pub fn count_timer(&self) -> usize {
        self.devices
            .values()
            .filter(|d| matches!(d, Device::Timer(_)))
            .count()
    }

    /// Check if any devices are registered
    pub fn is_empty(&self) -> bool {
        self.devices.is_empty()
    }

    /// Get total device count
    pub fn count(&self) -> usize {
        self.devices.len()
    }

    // ======================================================================
    // Device Specific Iterators
    // ======================================================================

    /// Iterate over all serial devices
    pub fn serial_devices(&self) -> impl Iterator<Item = Arc<Mutex<dyn DynSerialPort>>> + '_ {
        self.devices.values().filter_map(|d| {
            if let Device::Serial(serial) = d {
                Some(serial.clone())
            } else {
                None
            }
        })
    }
}

/// # Safety
///
/// This type is marked as `Send` to allow it to be safely shared across thread boundaries.
/// In practice, `DeviceManager` is accessed through a singleton instance that is guarded
/// by a lock, ensuring thread-safe access and preventing data races.
unsafe impl Send for DeviceManager {}
unsafe impl Sync for DeviceManager {}
