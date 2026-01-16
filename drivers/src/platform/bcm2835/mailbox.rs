//! BCM2835 Mailbox Interface
//!
//! The mailbox provides a communication channel between the ARM CPU
//! and the VideoCore GPU. It's used for:
//! - Querying system configuration (memory, clocks, etc.)
//! - Configuring framebuffer
//! - Power management
//! - And more
//!
//! # Architecture
//!
//! The mailbox is a bidirectional FIFO with multiple channels.
//! Each channel serves a different purpose:
//! - Channel 0: Power management
//! - Channel 1: Framebuffer
//! - Channel 8: Property tags (most commonly used)
//!
//! # Usage
//!
//! ```no_run
//! use drivers::platform::bcm2835::mailbox::{Mailbox, Channel};
//!
//! unsafe {
//!     let mut mbox = Mailbox::new();
//!     
//!     // Prepare a request buffer (must be 16-byte aligned)
//!     let mut buffer = [0u32; 8];
//!     // ... fill buffer with property tags ...
//!     
//!     if mbox.call(Channel::Property, &buffer as *const _ as usize) {
//!         // Success! Read response from buffer
//!     }
//! }
//! ```

use core::ptr::{read_volatile, write_volatile};

/// Mailbox base address (offset from peripheral base).
const MAILBOX_OFFSET: usize = 0xB880;

/// Mailbox base address.
pub const MAILBOX_BASE: usize = super::PERIPHERAL_BASE + MAILBOX_OFFSET;

// Register offsets
const REG_READ: usize = 0x00;
const REG_STATUS: usize = 0x18;
const REG_WRITE: usize = 0x20;

// Status flags
const STATUS_EMPTY: u32 = 1 << 30;
const STATUS_FULL: u32 = 1 << 31;

// Masks
const CHANNEL_MASK: u32 = 0xF;
const DATA_MASK: u32 = 0xFFFF_FFF0;

/// Mailbox channels.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum Channel {
    /// Power management.
    Power = 0,
    /// Framebuffer.
    Framebuffer = 1,
    /// Virtual UART.
    VirtualUart = 2,
    /// VCHIQ.
    Vchiq = 3,
    /// LEDs.
    Leds = 4,
    /// Buttons.
    Buttons = 5,
    /// Touch screen.
    Touchscreen = 6,
    /// Unused.
    Unused = 7,
    /// Property tags (most commonly used).
    Property = 8,
}

impl From<Channel> for u8 {
    fn from(channel: Channel) -> u8 {
        channel as u8
    }
}

/// Property interface response codes.
pub mod response {
    /// Request successful.
    pub const SUCCESS: u32 = 0x8000_0000;
    /// Request failed.
    pub const ERROR: u32 = 0x8000_0001;
}

/// Common property tags.
pub mod tags {
    /// Get firmware revision.
    pub const GET_FIRMWARE_REVISION: u32 = 0x0000_0001;
    /// Get board model.
    pub const GET_BOARD_MODEL: u32 = 0x0001_0001;
    /// Get board revision.
    pub const GET_BOARD_REVISION: u32 = 0x0001_0002;
    /// Get board MAC address.
    pub const GET_BOARD_MAC_ADDRESS: u32 = 0x0001_0003;
    /// Get board serial.
    pub const GET_BOARD_SERIAL: u32 = 0x0001_0004;
    /// Get ARM memory.
    pub const GET_ARM_MEMORY: u32 = 0x0001_0005;
    /// Get VC memory.
    pub const GET_VC_MEMORY: u32 = 0x0001_0006;
    /// Get clocks.
    pub const GET_CLOCKS: u32 = 0x0001_0007;
    /// Get command line.
    pub const GET_COMMAND_LINE: u32 = 0x0005_0001;
    /// Get DMA channels.
    pub const GET_DMA_CHANNELS: u32 = 0x0006_0001;
    /// Get power state.
    pub const GET_POWER_STATE: u32 = 0x0002_0001;
    /// Set power state.
    pub const SET_POWER_STATE: u32 = 0x0002_8001;
    /// Allocate framebuffer.
    pub const ALLOCATE_BUFFER: u32 = 0x0004_0001;
    /// Release framebuffer.
    pub const RELEASE_BUFFER: u32 = 0x0004_8001;
    /// Get physical display size.
    pub const GET_PHYSICAL_SIZE: u32 = 0x0004_0003;
    /// Set physical display size.
    pub const SET_PHYSICAL_SIZE: u32 = 0x0004_8003;
    /// Get virtual display size.
    pub const GET_VIRTUAL_SIZE: u32 = 0x0004_0004;
    /// Set virtual display size.
    pub const SET_VIRTUAL_SIZE: u32 = 0x0004_8004;
    /// Get depth.
    pub const GET_DEPTH: u32 = 0x0004_0005;
    /// Set depth.
    pub const SET_DEPTH: u32 = 0x0004_8005;
    /// Get pixel order.
    pub const GET_PIXEL_ORDER: u32 = 0x0004_0006;
    /// Set pixel order.
    pub const SET_PIXEL_ORDER: u32 = 0x0004_8006;
    /// Get pitch.
    pub const GET_PITCH: u32 = 0x0004_0008;
}

/// BCM2835 Mailbox interface.
#[derive(Debug)]
pub struct Mailbox {
    base: usize,
}

impl Mailbox {
    /// Create a new mailbox interface.
    ///
    /// # Safety
    ///
    /// Mailbox registers must be properly mapped.
    pub const unsafe fn new() -> Self {
        Self { base: MAILBOX_BASE }
    }

    /// Create a mailbox with custom base address (for testing).
    ///
    /// # Safety
    ///
    /// `base` must point to valid mailbox registers.
    pub const unsafe fn with_base(base: usize) -> Self {
        Self { base }
    }

    #[inline]
    fn read_status(&self) -> u32 {
        unsafe { read_volatile((self.base + REG_STATUS) as *const u32) }
    }

    #[inline]
    fn read_data(&self) -> u32 {
        unsafe { read_volatile((self.base + REG_READ) as *const u32) }
    }

    #[inline]
    fn write_data(&mut self, value: u32) {
        unsafe { write_volatile((self.base + REG_WRITE) as *mut u32, value) }
    }

    fn wait_for_write(&self) {
        while self.read_status() & STATUS_FULL != 0 {
            core::hint::spin_loop();
        }
    }

    fn wait_for_read(&self) {
        while self.read_status() & STATUS_EMPTY != 0 {
            core::hint::spin_loop();
        }
    }

    /// Perform a mailbox call.
    ///
    /// # Arguments
    ///
    /// - `channel`: Which mailbox channel to use
    /// - `buffer_phys`: Physical address of the request buffer
    ///
    /// # Buffer Format
    ///
    /// The buffer must:
    /// - Be 16-byte aligned
    /// - Be a physical address visible to the GPU
    /// - Follow the property tag format:
    ///   ```text
    ///   [0] = Total buffer size in bytes
    ///   [1] = Request/response code (0 for request)
    ///   [2..n-1] = Tags
    ///   [n] = End tag (0)
    ///   ```
    ///
    /// # Returns
    ///
    /// `true` if the GPU responded successfully, `false` otherwise.
    ///
    /// # Safety
    ///
    /// - Buffer must be valid and properly formatted
    /// - Buffer must remain valid until call completes
    /// - Not synchronized for multicore use
    pub unsafe fn call(&mut self, channel: Channel, buffer_phys: usize) -> bool {
        // Verify alignment
        debug_assert_eq!(buffer_phys & 0xF, 0, "Buffer must be 16-byte aligned");

        // Combine buffer address with channel number
        let msg = (buffer_phys & DATA_MASK as usize) | (channel as usize & CHANNEL_MASK as usize);

        // Wait for mailbox to be ready
        self.wait_for_write();

        // Send request
        self.write_data(msg as u32);

        // Wait for response
        loop {
            self.wait_for_read();
            let resp = self.read_data();

            // Check if response is for our channel
            if (resp & CHANNEL_MASK) == channel as u32 {
                let resp_addr = (resp & DATA_MASK) as usize;

                // Verify response address matches request
                if resp_addr != (buffer_phys & DATA_MASK as usize) {
                    return false;
                }

                // Check response code (second word of buffer)
                let response_code = unsafe { read_volatile((buffer_phys + 4) as *const u32) };

                return response_code == response::SUCCESS;
            }
        }
    }

    /// Perform a mailbox call with a mutable buffer.
    ///
    /// This is a convenience wrapper that:
    /// 1. Takes a mutable buffer slice
    /// 2. Verifies alignment
    /// 3. Calls the mailbox
    /// 4. Returns success/failure
    ///
    /// # Safety
    ///
    /// - Buffer must be properly formatted
    /// - Physical address must equal virtual address (identity mapping)
    pub unsafe fn call_with_buffer(
        &mut self,
        channel: Channel,
        buffer: &mut [u32],
    ) -> Result<(), MailboxError> {
        let buffer_ptr = buffer.as_ptr() as usize;

        // Check alignment
        if buffer_ptr & 0xF != 0 {
            return Err(MailboxError::UnalignedBuffer);
        }

        // Call mailbox (assuming identity mapping)
        if unsafe { self.call(channel, buffer_ptr) } {
            Ok(())
        } else {
            Err(MailboxError::CallFailed)
        }
    }
}

// SAFETY: Mailbox wraps memory-mapped hardware.
// Access is synchronized externally.
unsafe impl Send for Mailbox {}
unsafe impl Sync for Mailbox {}

/// Mailbox errors.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MailboxError {
    /// Buffer is not 16-byte aligned.
    UnalignedBuffer,
    /// Mailbox call failed (GPU returned error).
    CallFailed,
    /// Invalid response.
    InvalidResponse,
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Query ARM memory size using the mailbox.
///
/// Returns `(base, size)` in bytes.
///
/// # Safety
///
/// - Mailbox must be accessible
/// - Identity mapping required (physical == virtual)
pub unsafe fn get_arm_memory() -> Option<(usize, usize)> {
    #[repr(C, align(16))]
    struct ArmMemoryRequest {
        size: u32,
        code: u32,
        tag: u32,
        val_buf_size: u32,
        val_len: u32,
        base: u32,
        length: u32,
        end: u32,
    }

    static mut REQ: ArmMemoryRequest = ArmMemoryRequest {
        size: core::mem::size_of::<ArmMemoryRequest>() as u32,
        code: 0,
        tag: tags::GET_ARM_MEMORY,
        val_buf_size: 8,
        val_len: 0,
        base: 0,
        length: 0,
        end: 0,
    };

    let mut mailbox = unsafe { Mailbox::new() };
    let req_phys = &raw const REQ as usize;

    if unsafe { mailbox.call(Channel::Property, req_phys) } {
        let base = unsafe { read_volatile(core::ptr::addr_of!(REQ.base)) } as usize;
        let size = unsafe { read_volatile(core::ptr::addr_of!(REQ.length)) } as usize;
        Some((base, size))
    } else {
        None
    }
}

/// Query VideoCore (GPU) memory size using the mailbox.
///
/// Returns `(base, size)` in bytes.
///
/// # Safety
///
/// - Mailbox must be accessible
/// - Identity mapping required
pub unsafe fn get_vc_memory() -> Option<(usize, usize)> {
    #[repr(C, align(16))]
    struct VcMemoryRequest {
        size: u32,
        code: u32,
        tag: u32,
        val_buf_size: u32,
        val_len: u32,
        base: u32,
        length: u32,
        end: u32,
    }

    static mut REQ: VcMemoryRequest = VcMemoryRequest {
        size: core::mem::size_of::<VcMemoryRequest>() as u32,
        code: 0,
        tag: tags::GET_VC_MEMORY,
        val_buf_size: 8,
        val_len: 0,
        base: 0,
        length: 0,
        end: 0,
    };

    let mut mailbox = unsafe { Mailbox::new() };
    let req_phys = &raw const REQ as usize;

    if unsafe { mailbox.call(Channel::Property, req_phys) } {
        let base = unsafe { read_volatile(core::ptr::addr_of!(REQ.base)) } as usize;
        let size = unsafe { read_volatile(core::ptr::addr_of!(REQ.length)) } as usize;
        Some((base, size))
    } else {
        None
    }
}

/// Query the firmware revision.
///
/// # Safety
///
/// - Mailbox must be accessible
/// - Identity mapping required
pub unsafe fn get_firmware_revision() -> Option<u32> {
    #[repr(C, align(16))]
    struct FirmwareRequest {
        size: u32,
        code: u32,
        tag: u32,
        val_buf_size: u32,
        val_len: u32,
        revision: u32,
        end: u32,
    }

    static mut REQ: FirmwareRequest = FirmwareRequest {
        size: core::mem::size_of::<FirmwareRequest>() as u32,
        code: 0,
        tag: tags::GET_FIRMWARE_REVISION,
        val_buf_size: 4,
        val_len: 0,
        revision: 0,
        end: 0,
    };

    let mut mailbox = unsafe { Mailbox::new() };
    let req_phys = &raw const REQ as usize;

    if unsafe { mailbox.call(Channel::Property, req_phys) } {
        Some(unsafe { read_volatile(core::ptr::addr_of!(REQ.revision)) })
    } else {
        None
    }
}

/// Query the board serial number.
///
/// # Safety
///
/// - Mailbox must be accessible
/// - Identity mapping required
pub unsafe fn get_board_serial() -> Option<u64> {
    #[repr(C, align(16))]
    struct SerialRequest {
        size: u32,
        code: u32,
        tag: u32,
        val_buf_size: u32,
        val_len: u32,
        serial_low: u32,
        serial_high: u32,
        end: u32,
    }

    static mut REQ: SerialRequest = SerialRequest {
        size: core::mem::size_of::<SerialRequest>() as u32,
        code: 0,
        tag: tags::GET_BOARD_SERIAL,
        val_buf_size: 8,
        val_len: 0,
        serial_low: 0,
        serial_high: 0,
        end: 0,
    };

    let mut mailbox = unsafe { Mailbox::new() };
    let req_phys = &raw const REQ as usize;

    if unsafe { mailbox.call(Channel::Property, req_phys) } {
        let low = unsafe { read_volatile(core::ptr::addr_of!(REQ.serial_low)) } as u64;
        let high = unsafe { read_volatile(core::ptr::addr_of!(REQ.serial_high)) } as u64;
        Some((high << 32) | low)
    } else {
        None
    }
}
