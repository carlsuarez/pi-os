use crate::hal::block_device::BlockDevice;
use crate::hal::framebuffer::FrameBuffer;
use crate::hal::serial::SerialPort;
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use common::sync::SpinLock;

/// Device types that can be managed
pub enum Device {
    Serial(Arc<SpinLock<Box<dyn SerialPort + Send>>>),
    Block(Arc<dyn BlockDevice>),
    FrameBuffer(Arc<SpinLock<Box<dyn FrameBuffer>>>),
}

impl Device {
    /// Create a serial device from any SerialPort implementation
    pub fn new_serial<T: SerialPort + Send + 'static>(serial: T) -> Self {
        Device::Serial(Arc::new(SpinLock::new(Box::new(serial))))
    }

    /// Create a block device from any BlockDevice implementation
    pub fn new_block<T: BlockDevice + 'static>(block: T) -> Self {
        Device::Block(Arc::new(block))
    }

    /// Create a framebuffer device from any FrameBuffer implementation
    pub fn new_framebuffer<T: FrameBuffer + 'static>(fb: T) -> Self {
        Device::FrameBuffer(Arc::new(SpinLock::new(Box::new(fb))))
    }
}

pub struct DeviceManager {
    devices: BTreeMap<String, Device>,
}

impl DeviceManager {
    pub const fn new() -> Self {
        Self {
            devices: BTreeMap::new(),
        }
    }

    pub fn register(&mut self, name: String, device: Device) {
        self.devices.insert(name, device);
    }

    pub fn get(&self, name: &str) -> Option<&Device> {
        self.devices.get(name)
    }

    pub fn list(&self) -> impl Iterator<Item = &String> {
        self.devices.keys()
    }

    pub fn serial(&self, name: &str) -> Option<Arc<SpinLock<Box<dyn SerialPort + Send>>>> {
        match self.get(name)? {
            Device::Serial(serial) => Some(serial.clone()),
            _ => None,
        }
    }

    pub fn block(&self, name: &str) -> Option<Arc<dyn BlockDevice>> {
        match self.get(name)? {
            Device::Block(block) => Some(block.clone()),
            _ => None,
        }
    }

    pub fn framebuffer(&self, name: &str) -> Option<Arc<SpinLock<Box<dyn FrameBuffer>>>> {
        match self.get(name)? {
            Device::FrameBuffer(fb) => Some(fb.clone()),
            _ => None,
        }
    }

    pub fn console(&self) -> Option<Arc<SpinLock<Box<dyn SerialPort + Send>>>> {
        self.serial("console")
    }
}

static DEVICE_MANAGER: SpinLock<DeviceManager> = SpinLock::new(DeviceManager::new());

pub fn devices() -> &'static SpinLock<DeviceManager> {
    &DEVICE_MANAGER
}
