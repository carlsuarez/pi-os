use crate::hal::serial::{DynSerialPort, SerialConfig, SerialError, SerialPort};

pub struct Uart16550 {
    base_addr: u16,
    is_mmio: bool,
}

impl Uart16550 {
    /// Create a new UART16550 instance using MMIO.
    pub fn new_mmio(base_addr: usize) -> Self {
        Self {
            base_addr: base_addr as u16,
            is_mmio: true,
        }
    }

    /// Create a new UART16550 instance using PIO.
    pub fn new_pio(base_addr: u16) -> Self {
        Self {
            base_addr,
            is_mmio: false,
        }
    }
}

impl SerialPort for Uart16550 {
    type Error = SerialError;

    fn configure(&mut self, _config: SerialConfig) -> Result<(), Self::Error> {
        todo!()
    }

    fn write_byte(&mut self, _byte: u8) -> Result<(), Self::Error> {
        todo!()
    }

    fn read_byte(&mut self) -> Result<u8, Self::Error> {
        todo!()
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        todo!()
    }

    fn is_busy(&self) -> bool {
        todo!()
    }
}

impl DynSerialPort for Uart16550 {
    fn configure(&mut self, config: SerialConfig) -> Result<(), SerialError> {
        SerialPort::configure(self, config)
    }

    fn write_byte(&mut self, byte: u8) -> Result<(), SerialError> {
        SerialPort::write_byte(self, byte)
    }

    fn read_byte(&mut self) -> Result<u8, SerialError> {
        SerialPort::read_byte(self)
    }

    fn flush(&mut self) -> Result<(), SerialError> {
        SerialPort::flush(self)
    }

    fn is_busy(&self) -> bool {
        SerialPort::is_busy(self)
    }
}
