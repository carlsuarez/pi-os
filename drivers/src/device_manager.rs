//! Device Manager
//!
//! Central registry for all hardware devices. Devices are registered by the
//! platform during initialization and can be accessed by name or type.
//!
//! # Usage
//!
//! ```rust
//! use drivers::device_manager::{devices, Device, WithNonBlocking, WithCountingPeriodic};
//! use drivers::hal::serial::{SerialPort, NonBlockingSerial};
//!
//! // Platform registers devices during init
//! let uart = Pl011::new(0x2020_1000);
//! devices().lock().register("serial0", WithNonBlocking(uart));
//!
//! // Kernel accesses devices by name
//! if let Some(serial) = devices().lock().serial("serial0") {
//!     let mut port = serial.lock();
//!     port.write_byte(b'H');
//! }
//! ```

use crate::hal::block_device::{
    BlockDevice, DynBlockDevice, DynBlockDeviceExt, DynIdentifiableBlockDevice,
    IdentifiableBlockDevice,
};
use crate::hal::fb::FrameBuffer;
use crate::hal::interrupt::{
    DynConfigurableInterruptController, DynInterruptController, DynPriorityInterruptController,
    InterruptController,
};
use crate::hal::serial::{DynNonBlockingSerial, DynSerialPort, NonBlockingSerial, SerialPort};
use crate::hal::timer::{
    CountingTimer, DynCountingTimer, DynPeriodicTimer, DynTimer, PeriodicTimer, Timer,
};

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use core::cell::OnceCell;
use spin::Mutex;

// ============================================================
// System Timer Channel
// ============================================================

struct OnceCellChannel {
    inner: OnceCell<usize>,
}

unsafe impl Sync for OnceCellChannel {}

static SYS_TIMER_CHANNEL: OnceCellChannel = OnceCellChannel {
    inner: OnceCell::new(),
};

// ============================================================
// Capability Structs
// ============================================================

#[derive(Clone)]
pub struct TimerCapabilities {
    pub base: Arc<Mutex<dyn DynTimer>>,
    pub counting: Option<Arc<Mutex<dyn DynCountingTimer>>>,
    pub periodic: Option<Arc<Mutex<dyn DynPeriodicTimer>>>,
}

#[derive(Clone)]
pub struct SerialPortCapabilities {
    pub base: Arc<Mutex<dyn DynSerialPort>>,
    pub nonblocking: Option<Arc<Mutex<dyn DynNonBlockingSerial>>>,
}

#[derive(Clone)]
pub struct BlockDeviceCapabilities {
    pub base: Arc<dyn DynBlockDevice>,
    pub ext: Option<Arc<dyn DynBlockDeviceExt>>,
    pub id: Option<Arc<dyn DynIdentifiableBlockDevice>>,
}

#[derive(Clone)]
pub struct InterruptControllerCapabilities {
    pub base: Arc<Mutex<dyn DynInterruptController>>,
    pub priority: Option<Arc<Mutex<dyn DynPriorityInterruptController>>>,
    pub configurable: Option<Arc<Mutex<dyn DynConfigurableInterruptController>>>,
}

// ============================================================
// Device Enum
// ============================================================

pub enum Device {
    Serial(SerialPortCapabilities),
    Block(BlockDeviceCapabilities),
    FrameBuffer(Arc<Mutex<dyn FrameBuffer>>),
    Timer(TimerCapabilities),
    InterruptController(InterruptControllerCapabilities),
}

// ============================================================
// Wrapper Types for Capabilities
// ============================================================

/// Wrapper for serial port with only base capability
pub struct SerialOnly<T>(pub T);

/// Wrapper for timer with only base capability
pub struct TimerOnly<T>(pub T);

/// Wrapper for block device with only base capability
pub struct BlockOnly<T>(pub T);

/// Wrapper for interrupt controller with only base capability
pub struct InterruptControllerOnly<T>(pub T);

/// Wrapper for serial port with non-blocking capability
pub struct WithNonBlocking<T>(pub T);

/// Wrapper for timer with counting capability
pub struct WithCounting<T>(pub T);

/// Wrapper for timer with counting and periodic capability
pub struct WithCountingPeriodic<T>(pub T);

/// Wrapper for interrupt controller with priority capability
pub struct WithPriority<T>(pub T);

/// Wrapper for interrupt controller with configurable capability
pub struct WithConfigurable<T>(pub T);

/// Wrapper for interrupt controller with both priority and configurable
pub struct WithPriorityConfigurable<T>(pub T);

/// Wrapper for block device with extended capabilities
pub struct WithBlockExt<T>(pub T);

/// Wrapper for block device with identifiable capability
pub struct WithBlockId<T>(pub T);

/// Wrapper for block device with both ext and id capabilities
pub struct WithBlockFull<T>(pub T);

// ============================================================
// Into<Device> Implementations
// ============================================================

// ---------------- Serial ----------------

impl<T: SerialPort + 'static> From<SerialOnly<T>> for Device {
    fn from(dev: SerialOnly<T>) -> Self {
        let arc = Arc::new(Mutex::new(dev.0));
        let base: Arc<Mutex<dyn DynSerialPort>> = arc.clone();

        Device::Serial(SerialPortCapabilities {
            base,
            nonblocking: None,
        })
    }
}

impl<T> From<WithNonBlocking<T>> for Device
where
    T: SerialPort + NonBlockingSerial + 'static,
{
    fn from(dev: WithNonBlocking<T>) -> Self {
        // Create one Arc and clone it for both capabilities
        let arc = Arc::new(Mutex::new(dev.0));
        let base: Arc<Mutex<dyn DynSerialPort>> = arc.clone();
        let nonblocking: Arc<Mutex<dyn DynNonBlockingSerial>> = arc;

        Device::Serial(SerialPortCapabilities {
            base,
            nonblocking: Some(nonblocking),
        })
    }
}

// ---------------- Timer ----------------

impl<T> From<TimerOnly<T>> for Device
where
    T: Timer + 'static,
    <T as Timer>::Handle: From<usize>,
{
    fn from(dev: TimerOnly<T>) -> Self {
        let arc = Arc::new(Mutex::new(dev.0));
        let base: Arc<Mutex<dyn DynTimer>> = arc.clone();

        Device::Timer(TimerCapabilities {
            base,
            counting: None,
            periodic: None,
        })
    }
}

impl<T> From<WithCounting<T>> for Device
where
    T: Timer + CountingTimer + 'static,
    <T as Timer>::Handle: From<usize>,
{
    fn from(dev: WithCounting<T>) -> Self {
        let arc = Arc::new(Mutex::new(dev.0));
        let base: Arc<Mutex<dyn DynTimer>> = arc.clone();
        let counting: Arc<Mutex<dyn DynCountingTimer>> = arc;

        Device::Timer(TimerCapabilities {
            base,
            counting: Some(counting),
            periodic: None,
        })
    }
}

impl<T> From<WithCountingPeriodic<T>> for Device
where
    T: Timer + CountingTimer + PeriodicTimer + 'static,
    <T as Timer>::Handle: From<usize>,
{
    fn from(dev: WithCountingPeriodic<T>) -> Self {
        let arc = Arc::new(Mutex::new(dev.0));
        let base: Arc<Mutex<dyn DynTimer>> = arc.clone();
        let counting: Arc<Mutex<dyn DynCountingTimer>> = arc.clone();
        let periodic: Arc<Mutex<dyn DynPeriodicTimer>> = arc;

        Device::Timer(TimerCapabilities {
            base,
            counting: Some(counting),
            periodic: Some(periodic),
        })
    }
}

// ---------------- Block ----------------

impl<T: BlockDevice + 'static> From<BlockOnly<T>> for Device {
    fn from(dev: BlockOnly<T>) -> Self {
        let arc = Arc::new(dev.0);
        let base: Arc<dyn DynBlockDevice> = arc.clone();

        Device::Block(BlockDeviceCapabilities {
            base,
            ext: None,
            id: None,
        })
    }
}

impl<T> From<WithBlockExt<T>> for Device
where
    T: BlockDevice + DynBlockDeviceExt + 'static,
{
    fn from(dev: WithBlockExt<T>) -> Self {
        let arc = Arc::new(dev.0);
        let base: Arc<dyn DynBlockDevice> = arc.clone();
        let ext: Arc<dyn DynBlockDeviceExt> = arc;

        Device::Block(BlockDeviceCapabilities {
            base,
            ext: Some(ext),
            id: None,
        })
    }
}

impl<T> From<WithBlockId<T>> for Device
where
    T: BlockDevice + DynIdentifiableBlockDevice + 'static,
{
    fn from(dev: WithBlockId<T>) -> Self {
        let arc = Arc::new(dev.0);
        let base: Arc<dyn DynBlockDevice> = arc.clone();
        let id: Arc<dyn DynIdentifiableBlockDevice> = arc;

        Device::Block(BlockDeviceCapabilities {
            base,
            ext: None,
            id: Some(id),
        })
    }
}

impl<T> From<WithBlockFull<T>> for Device
where
    T: BlockDevice + DynBlockDeviceExt + IdentifiableBlockDevice + 'static,
{
    fn from(dev: WithBlockFull<T>) -> Self {
        let arc = Arc::new(dev.0);
        let base: Arc<dyn DynBlockDevice> = arc.clone();
        let ext: Arc<dyn DynBlockDeviceExt> = arc.clone();
        let id: Arc<dyn DynIdentifiableBlockDevice> = arc;

        Device::Block(BlockDeviceCapabilities {
            base,
            ext: Some(ext),
            id: Some(id),
        })
    }
}

// ---------------- Framebuffer ----------------

impl<T> From<T> for Device
where
    T: FrameBuffer + 'static,
{
    fn from(dev: T) -> Self {
        Device::FrameBuffer(Arc::new(Mutex::new(dev)))
    }
}

// ---------------- Interrupt Controller ----------------

impl<T: InterruptController + 'static> From<InterruptControllerOnly<T>> for Device {
    fn from(dev: InterruptControllerOnly<T>) -> Self {
        let arc = Arc::new(Mutex::new(dev.0));
        let base: Arc<Mutex<dyn DynInterruptController>> = arc.clone();

        Device::InterruptController(InterruptControllerCapabilities {
            base,
            priority: None,
            configurable: None,
        })
    }
}

impl<T> From<WithPriority<T>> for Device
where
    T: InterruptController + DynPriorityInterruptController + 'static,
{
    fn from(dev: WithPriority<T>) -> Self {
        let arc = Arc::new(Mutex::new(dev.0));
        let base: Arc<Mutex<dyn DynInterruptController>> = arc.clone();
        let priority: Arc<Mutex<dyn DynPriorityInterruptController>> = arc;

        Device::InterruptController(InterruptControllerCapabilities {
            base,
            priority: Some(priority),
            configurable: None,
        })
    }
}

impl<T> From<WithConfigurable<T>> for Device
where
    T: InterruptController + DynConfigurableInterruptController + 'static,
{
    fn from(dev: WithConfigurable<T>) -> Self {
        let arc = Arc::new(Mutex::new(dev.0));
        let base: Arc<Mutex<dyn DynInterruptController>> = arc.clone();
        let configurable: Arc<Mutex<dyn DynConfigurableInterruptController>> = arc;

        Device::InterruptController(InterruptControllerCapabilities {
            base,
            priority: None,
            configurable: Some(configurable),
        })
    }
}

impl<T> From<WithPriorityConfigurable<T>> for Device
where
    T: InterruptController
        + DynPriorityInterruptController
        + DynConfigurableInterruptController
        + 'static,
{
    fn from(dev: WithPriorityConfigurable<T>) -> Self {
        let arc = Arc::new(Mutex::new(dev.0));
        let base: Arc<Mutex<dyn DynInterruptController>> = arc.clone();
        let priority: Arc<Mutex<dyn DynPriorityInterruptController>> = arc.clone();
        let configurable: Arc<Mutex<dyn DynConfigurableInterruptController>> = arc;

        Device::InterruptController(InterruptControllerCapabilities {
            base,
            priority: Some(priority),
            configurable: Some(configurable),
        })
    }
}

// ============================================================
// Device Manager
// ============================================================

pub struct DeviceManager {
    devices: BTreeMap<String, Device>,
}

impl DeviceManager {
    pub const fn new() -> Self {
        Self {
            devices: BTreeMap::new(),
        }
    }

    /// Unified register API
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // Serial with non-blocking
    /// device_mgr.register("serial0", WithNonBlocking(uart));
    ///
    /// // Timer with counting and periodic
    /// device_mgr.register("timer", WithCountingPeriodic(timer));
    ///
    /// // Framebuffer (implements FrameBuffer)
    /// device_mgr.register("fb0", framebuffer);
    /// ```
    pub fn register<D>(&mut self, name: impl Into<String>, dev: D)
    where
        D: Into<Device>,
    {
        self.devices.insert(name.into(), dev.into());
    }

    pub fn get(&self, name: &str) -> Option<&Device> {
        self.devices.get(name)
    }

    pub fn list(&self) -> impl Iterator<Item = &String> {
        self.devices.keys()
    }

    // ========================================================
    // Accessors
    // ========================================================

    pub fn serial(&self, name: &str) -> Option<Arc<Mutex<dyn DynSerialPort>>> {
        match self.get(name)? {
            Device::Serial(s) => Some(s.base.clone()),
            _ => None,
        }
    }

    pub fn serial_nonblocking(&self, name: &str) -> Option<Arc<Mutex<dyn DynNonBlockingSerial>>> {
        match self.get(name)? {
            Device::Serial(s) => s.nonblocking.clone(),
            _ => None,
        }
    }

    pub fn block(&self, name: &str) -> Option<Arc<dyn DynBlockDevice>> {
        match self.get(name)? {
            Device::Block(b) => Some(b.base.clone()),
            _ => None,
        }
    }

    pub fn framebuffer(&self, name: &str) -> Option<Arc<Mutex<dyn FrameBuffer>>> {
        match self.get(name)? {
            Device::FrameBuffer(fb) => Some((*fb).clone()),
            _ => None,
        }
    }

    pub fn timer(&self, name: &str) -> Option<Arc<Mutex<dyn DynTimer>>> {
        match self.get(name)? {
            Device::Timer(t) => Some(t.base.clone()),
            _ => None,
        }
    }

    pub fn timer_counting(&self, name: &str) -> Option<Arc<Mutex<dyn DynCountingTimer>>> {
        match self.get(name)? {
            Device::Timer(t) => t.counting.clone(),
            _ => None,
        }
    }

    pub fn timer_periodic(&self, name: &str) -> Option<Arc<Mutex<dyn DynPeriodicTimer>>> {
        match self.get(name)? {
            Device::Timer(t) => t.periodic.clone(),
            _ => None,
        }
    }

    pub fn interrupt_controller(
        &self,
        name: &str,
    ) -> Option<Arc<Mutex<dyn DynInterruptController>>> {
        match self.get(name)? {
            Device::InterruptController(i) => Some(i.base.clone()),
            _ => None,
        }
    }

    // ========================================================
    // Convenience Accessors
    // ========================================================

    pub fn serial_console(&self) -> Option<Arc<Mutex<dyn DynSerialPort>>> {
        self.serial("console")
            .or_else(|| self.serial("serial0"))
            .or_else(|| {
                self.devices.values().find_map(|d| {
                    if let Device::Serial(s) = d {
                        Some(s.base.clone())
                    } else {
                        None
                    }
                })
            })
    }

    pub fn system_timer(&self) -> Option<Arc<Mutex<dyn DynTimer>>> {
        self.timer("system_timer")
            .or_else(|| self.timer("timer"))
            .or_else(|| {
                self.devices.values().find_map(|d| {
                    if let Device::Timer(t) = d {
                        Some(t.base.clone())
                    } else {
                        None
                    }
                })
            })
    }

    pub fn sys_timer_channel() -> Option<usize> {
        SYS_TIMER_CHANNEL.inner.get().copied()
    }

    /// Get the system timer with all its capabilities
    ///
    /// Returns the full TimerCapabilities so the user can check
    /// what features are available and use them directly.
    pub fn system_timer_caps(&self) -> Option<TimerCapabilities> {
        self.timer_caps("system_timer")
            .or_else(|| self.timer_caps("timer"))
            .or_else(|| {
                self.devices.values().find_map(|d| {
                    if let Device::Timer(t) = d {
                        Some(t.clone())
                    } else {
                        None
                    }
                })
            })
    }

    /// Get timer capabilities by name
    pub fn timer_caps(&self, name: &str) -> Option<TimerCapabilities> {
        match self.get(name)? {
            Device::Timer(t) => Some(t.clone()),
            _ => None,
        }
    }

    pub fn irq_controller(&self) -> Option<Arc<Mutex<dyn DynInterruptController>>> {
        self.interrupt_controller("intc")
            .or_else(|| self.interrupt_controller("pic"))
            .or_else(|| self.interrupt_controller("gic"))
            .or_else(|| {
                self.devices.values().find_map(|d| {
                    if let Device::InterruptController(i) = d {
                        Some(i.base.clone())
                    } else {
                        None
                    }
                })
            })
    }

    // ========================================================
    // Introspection
    // ========================================================

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

    pub fn is_empty(&self) -> bool {
        self.devices.is_empty()
    }

    pub fn count(&self) -> usize {
        self.devices.len()
    }
}

// Safety
unsafe impl Send for DeviceManager {}
unsafe impl Sync for DeviceManager {}
