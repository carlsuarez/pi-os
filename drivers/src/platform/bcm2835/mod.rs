mod emmc;
mod framebuffer;
mod gpio;
mod interrupt;
mod mailbox;
mod timer;

use super::{MemoryMap, Platform};
use crate::device_manager::{self, Device, DeviceManager, devices};
use crate::hal::framebuffer::FrameBufferConfig;
use crate::hal::gpio::PullMode;
use crate::peripheral::pl011::PL011;
use crate::platform::bcm2835::emmc::Emmc;
use crate::platform::bcm2835::framebuffer::Bcm2835Framebuffer;
use crate::platform::bcm2835::gpio::Bcm2835Gpio;
use crate::platform::bcm2835::interrupt::Bcm2835InterruptController;
use crate::platform::bcm2835::timer::{Bcm2835Timer, Channel};
use crate::{GpioController, InterruptController, SerialConfig, SerialPort, Timer};
use common::arch::arm::bcm2835::irq::IRQ_SYSTEM_TIMER_1;
use common::sync::SpinLock;

pub const PERIPHERAL_BASE: usize = 0x2000_0000;

pub struct Bcm2835Platform;

// ============================================================================
// Internal Platform State (not exposed)
// ============================================================================

/// Interrupt controller instance (private)
static INTERRUPT_CONTROLLER: SpinLock<Option<Bcm2835InterruptController>> = SpinLock::new(None);

/// System timer instance (private)
static TIMER: SpinLock<Option<Bcm2835Timer>> = SpinLock::new(None);

impl Platform for Bcm2835Platform {
    fn name() -> &'static str {
        "BCM2835 (Raspberry Pi 1/Zero)"
    }

    unsafe fn early_init() {
        // Configure GPIO pins for UART0
        let mut gpio = unsafe { Bcm2835Gpio::new() };
        gpio.set_alt_function(14, 0).ok(); // TX
        gpio.set_pull(14, PullMode::None).ok();
        gpio.set_alt_function(15, 0).ok(); // RX
        gpio.set_pull(15, PullMode::Up).ok();
    }

    fn memory_map() -> MemoryMap {
        MemoryMap {
            ram_start: 0x0000_0000,
            ram_size: 512 * 1024 * 1024,
            peripheral_base: PERIPHERAL_BASE,
            peripheral_size: 16 * 1024 * 1024,
            kernel_start: 0x8000,
        }
    }

    unsafe fn query_ram_size() -> Option<(usize, usize)> {
        unsafe { mailbox::get_arm_memory() }
    }

    unsafe fn init_devices(device_mgr: &mut DeviceManager) -> Result<(), &'static str> {
        // 1. Initialize interrupt controller
        let intc = unsafe { Bcm2835InterruptController::new() };
        *INTERRUPT_CONTROLLER.lock() = Some(intc);

        // 2. Initialize system timer
        let timer = unsafe { Bcm2835Timer::new() };
        *TIMER.lock() = Some(timer);

        // 3. Initialize and register console UART
        let mut uart = unsafe { PL011::new(0x2020_1000) };
        uart.configure(SerialConfig::new_8n1(115200))
            .map_err(|_| "Failed to configure UART")?;

        device_mgr.register("console".into(), Device::new_serial(uart));

        // Create alias for uart0
        let console_device = device_mgr.serial("console").unwrap();
        device_mgr.register("uart0".into(), Device::Serial(console_device));

        // 4. Initialize and register EMMC block device
        let mut emmc = unsafe { Emmc::new() };
        emmc.init().map_err(|_| "Failed to initialize EMMC")?;

        device_mgr.register("emmc0".into(), Device::new_block(emmc));

        // 5. Initialize framebuffer
        match unsafe { Bcm2835Framebuffer::new(FrameBufferConfig::default()) } {
            Ok(fb) => {
                device_mgr.register("fb0".into(), Device::new_framebuffer(fb));
            }
            Err(_e) => {
                // Framebuffer is optional, just log the error
                // Note: kprintln might not work here if called before console is ready
                // Consider storing this error for later logging
            }
        }

        Ok(())
    }

    fn next_pending_irq() -> Option<u32> {
        INTERRUPT_CONTROLLER
            .lock()
            .as_ref()
            .and_then(|intc| intc.next_pending())
    }

    fn enable_irq(irq: u32) {
        if let Some(ref mut intc) = *INTERRUPT_CONTROLLER.lock() {
            intc.enable(irq).ok();
        }
    }

    fn disable_irq(irq: u32) {
        if let Some(ref mut intc) = *INTERRUPT_CONTROLLER.lock() {
            intc.disable(irq).ok();
        }
    }

    fn timer_start(interval_us: u32) {
        if let Some(ref mut timer) = *TIMER.lock() {
            timer.start(Channel::Channel1, interval_us).ok();
        }
    }

    fn timer_clear() {
        if let Some(ref mut timer) = *TIMER.lock() {
            timer.clear_interrupt(Channel::Channel1).ok();
        }
    }

    fn timer_irq() -> u32 {
        IRQ_SYSTEM_TIMER_1
    }
}
